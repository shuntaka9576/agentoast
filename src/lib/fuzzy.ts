export interface FuzzyMatch {
  score: number;
  positions: number[]; // 0-indexed positions into target.toLowerCase()
}

// Fuzzy match. Lower score is better. Returns null when query is not a
// subsequence of target. Empty/whitespace-only queries match with score 0.
//
// Whitespace in the query splits it into independent tokens, fzf-style:
// every token must match somewhere in the target. Tokens may overlap and
// appear in any order; positions are merged for highlighting.
export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
  const tokens = query
    .toLowerCase()
    .split(/\s+/)
    .filter((t) => t !== "");
  if (tokens.length === 0) return { score: 0, positions: [] };
  const t = target.toLowerCase();

  if (tokens.length === 1) return matchToken(tokens[0], t);

  const positions = new Set<number>();
  let total = 0;
  for (const token of tokens) {
    const m = matchToken(token, t);
    if (!m) return null;
    total += m.score;
    for (const p of m.positions) positions.add(p);
  }
  return { score: total, positions: [...positions].sort((a, b) => a - b) };
}

export function fuzzyScore(query: string, target: string): number | null {
  return fuzzyMatch(query, target)?.score ?? null;
}

// Per-field highlight helper: returns positions of any tokens that hit
// `target`. Unlike `fuzzyMatch`, tokens that don't match are silently
// skipped instead of failing the whole match — needed when the AND-style
// combined-target match passes but a single field doesn't see every token
// (e.g. query "agent main" against just `repoName = "agentoast"`).
export function findMatchPositions(query: string, target: string): number[] {
  const tokens = query
    .toLowerCase()
    .split(/\s+/)
    .filter((t) => t !== "");
  if (tokens.length === 0) return [];
  const t = target.toLowerCase();
  const positions = new Set<number>();
  for (const token of tokens) {
    const m = matchToken(token, t);
    if (!m) continue;
    for (const p of m.positions) positions.add(p);
  }
  return [...positions].sort((a, b) => a - b);
}

// `q` and `t` must already be lower-cased.
function matchToken(q: string, t: string): FuzzyMatch | null {
  const subIdx = t.indexOf(q);
  if (subIdx >= 0) {
    const positions: number[] = [];
    for (let i = 0; i < q.length; i++) positions.push(subIdx + i);
    return { score: subIdx * 2 + (t.length - q.length), positions };
  }

  let ti = 0;
  let score = 0;
  let lastMatch = -2;
  const positions: number[] = [];
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi];
    let found = -1;
    while (ti < t.length) {
      if (t[ti] === ch) {
        found = ti;
        ti++;
        break;
      }
      ti++;
    }
    if (found < 0) return null;
    let charCost = found * 0.1 + 1;
    if (found === lastMatch + 1) charCost -= 0.8;
    if (found === 0 || isWordBoundary(t[found - 1])) charCost -= 0.5;
    score += Math.max(charCost, 0);
    lastMatch = found;
    positions.push(found);
  }
  // keep subsequence matches strictly worse than substring
  return { score: score + 100, positions };
}

function isWordBoundary(ch: string): boolean {
  return ch === " " || ch === "-" || ch === "_" || ch === "/" || ch === ".";
}
