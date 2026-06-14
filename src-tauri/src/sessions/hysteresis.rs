use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Instant;

use super::agents;

/// Inclusive line range marking the on-screen input box for an agent pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputRegion {
    pub start_line: usize,
    pub end_line: usize,
}

/// Tracks the most-recent hash of each pane's "body" (= content minus input
/// region and minus periodic footer noise) so the state detector can tell
/// whether anything new was drawn between polling cycles.
///
/// Used only as a SHORT-TERM ASSIST for the `at_prompt → Idle` path: if the
/// body just changed it almost certainly means the agent is still streaming
/// output, even though the spinner glyph happened to be missed by this
/// capture. See `detect_claude_status` priority #5b.
///
/// The tracker is intentionally per-pane in-memory state with no persistence
/// — restart-time recovery is unnecessary because the tracker's only job is
/// to bridge a few seconds of polling gaps.
#[derive(Default)]
pub struct PaneHysteresis {
    /// pane_id → (last_body_hash, last observed change time).
    /// `last_changed` is `None` until a real change is observed — the seed
    /// observation must NOT count as a change, otherwise the second cycle
    /// (~2 s later) would report `Some(seed_time)` whose elapsed time is
    /// still within CHANGE_TTL and the at_prompt path would flash Running
    /// for one cycle on every newly-seen pane.
    entries: HashMap<String, (u64, Option<Instant>)>,
}

impl PaneHysteresis {
    /// Observe all panes in one shot. The held lock spans only the in-memory
    /// hash computation across `panes` (no DB I/O, no tmux I/O).
    ///
    /// Returned map contains an entry only for panes whose body is
    /// hash-eligible AND have produced at least one observed hash change
    /// since startup. First-seen panes seed the tracker but are omitted
    /// from the result, so a fresh app start or a pane switch does not
    /// flash 3 s of false Running for every agent pane. Panes whose
    /// hashable body could not be extracted (input region unlocatable,
    /// unknown agent) are also omitted.
    pub fn observe_batch<'a>(
        &mut self,
        panes: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
    ) -> HashMap<String, Instant> {
        let now = Instant::now();
        let mut out = HashMap::new();
        for (pane_id, agent_type, content) in panes {
            let Some(h) = hash_body_region(agent_type, content) else {
                continue;
            };
            match self.entries.get_mut(pane_id) {
                Some((prev_hash, last_changed)) => {
                    if *prev_hash != h {
                        *prev_hash = h;
                        *last_changed = Some(now);
                    }
                    if let Some(t) = *last_changed {
                        out.insert(pane_id.to_string(), t);
                    }
                }
                None => {
                    self.entries.insert(pane_id.to_string(), (h, None));
                }
            }
        }
        out
    }

    /// Drop tracker entries for panes that no longer exist. Accepting borrowed
    /// ids avoids cloning every pane id every cycle just to feed the call.
    /// Bails before allocating the live-id set when the tracker is empty —
    /// the common case at startup and whenever there are no Claude panes.
    pub fn retain<'a>(&mut self, live: impl IntoIterator<Item = &'a str>) {
        if self.entries.is_empty() {
            return;
        }
        let live: HashSet<&str> = live.into_iter().collect();
        self.entries.retain(|k, _| live.contains(k.as_str()));
    }

    #[cfg(test)]
    pub fn entry(&self, pane_id: &str) -> Option<(u64, Option<Instant>)> {
        self.entries.get(pane_id).copied()
    }
}

/// Hash the body region of a pane's captured content, returning `None` when
/// the agent-specific extractor cannot isolate the conversation body (and
/// therefore hash assist must be disabled). Lines are hashed individually
/// with an explicit separator so a `join("\n")` allocation is avoided and
/// `["ab", "c"]` doesn't collide with `["a", "bc"]`.
pub fn hash_body_region(agent_type: &str, content: &str) -> Option<u64> {
    let lines = agents::collect_hashable_body(agent_type, content)?;
    let mut hasher = DefaultHasher::new();
    for line in &lines {
        line.hash(&mut hasher);
        hasher.write_u8(b'\n');
    }
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_body_region_returns_none_for_unknown_agent() {
        assert!(hash_body_region("mystery-agent", "anything").is_none());
    }

    #[test]
    fn observe_batch_first_seen_omitted_from_result() {
        let mut h = PaneHysteresis::default();
        let content = sample_claude_content();
        let out = h.observe_batch([("%1", "claude-code", content.as_str())]);
        assert!(
            out.is_empty(),
            "first observation must not yet report a change"
        );
        assert!(h.entry("%1").is_some(), "but tracker should remember it");
    }

    #[test]
    fn observe_batch_second_cycle_unchanged_still_omitted() {
        // Regression: a previous version stored last_changed=seed_time on
        // the first observation, so the second-cycle unchanged observation
        // returned Some(seed_time). Elapsed time of 2 s (the polling
        // cadence) was still within CHANGE_TTL=3 s, so every newly-seen
        // pane flashed Running for one cycle. Verify that doesn't happen.
        let mut h = PaneHysteresis::default();
        let content = sample_claude_content();
        h.observe_batch([("%1", "claude-code", content.as_str())]);
        let out = h.observe_batch([("%1", "claude-code", content.as_str())]);
        assert!(
            out.is_empty(),
            "unchanged second cycle must not report a change time"
        );
        let (_hash, last_changed) = h.entry("%1").expect("still tracked");
        assert_eq!(
            last_changed, None,
            "last_changed stays None until a real diff is observed"
        );
    }

    #[test]
    fn observe_batch_changed_after_seed_reports_change_time() {
        let mut h = PaneHysteresis::default();
        let v1 = sample_claude_content();
        h.observe_batch([("%1", "claude-code", v1.as_str())]); // seed
        std::thread::sleep(std::time::Duration::from_millis(2));
        let v2 = sample_claude_content().replace("history B", "history B+");
        let before = Instant::now();
        let out = h.observe_batch([("%1", "claude-code", v2.as_str())]);
        let observed = *out.get("%1").expect("change reported");
        assert!(observed >= before, "change time must be from this cycle");
    }

    #[test]
    fn observe_batch_unchanged_after_real_change_preserves_change_time() {
        let mut h = PaneHysteresis::default();
        let v1 = sample_claude_content();
        h.observe_batch([("%1", "claude-code", v1.as_str())]); // seed
        let v2 = sample_claude_content().replace("history B", "history B+");
        let out = h.observe_batch([("%1", "claude-code", v2.as_str())]);
        let t_change = *out.get("%1").unwrap();
        // Now a static cycle: same content as v2. Should still report
        // t_change (so the at_prompt branch keeps Running for CHANGE_TTL).
        let out2 = h.observe_batch([("%1", "claude-code", v2.as_str())]);
        assert_eq!(out2.get("%1"), Some(&t_change));
    }

    #[test]
    fn observe_batch_footer_change_does_not_advance_timestamp() {
        let mut h = PaneHysteresis::default();
        let v1 = sample_claude_content();
        h.observe_batch([("%1", "claude-code", v1.as_str())]);
        let v2 = v1.replace(
            "Context left until auto-compact: 8%",
            "Context left until auto-compact: 9%",
        );
        assert_ne!(v1, v2, "fixture must actually change");
        let out = h.observe_batch([("%1", "claude-code", v2.as_str())]);
        assert!(
            out.is_empty(),
            "footer-only change must not surface a change time"
        );
    }

    #[test]
    fn observe_batch_unknown_agent_skips_pane() {
        let mut h = PaneHysteresis::default();
        let out = h.observe_batch([("%1", "mystery-agent", "any content")]);
        assert!(out.is_empty());
        assert!(
            h.entry("%1").is_none(),
            "unknown agents must not seed the tracker"
        );
    }

    #[test]
    fn retain_drops_missing_entries() {
        let mut h = PaneHysteresis::default();
        let c = sample_claude_content();
        h.observe_batch([("%1", "claude-code", c.as_str())]);
        h.observe_batch([("%1", "claude-code", c.as_str())]); // promote past first-seen
        assert!(h.entry("%1").is_some());
        h.retain(["%2"]);
        assert!(h.entry("%1").is_none());
    }

    fn sample_claude_content() -> String {
        // Minimal Claude-shape capture using ONLY built-in TUI patterns —
        // no custom statusline content, so the tests don't accidentally
        // pin behavior to a specific statusline format.
        [
            "history A",
            "history B",
            "history C",
            "history D",
            "history E",
            "Context left until auto-compact: 8%",
            "⏸ plan mode on (shift+tab to cycle)",
            "│ ❯                 │",
            "│                   │",
            "│                   │",
        ]
        .join("\n")
    }
}
