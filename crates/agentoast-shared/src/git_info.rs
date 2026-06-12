//! Resolve git repository info (root, name, branch) by reading `.git`
//! metadata files directly instead of spawning `git` processes.
//!
//! The session poller resolves git info for every unique pane path on a 2s
//! interval; with `git rev-parse` + `git remote get-url` + `git branch` that
//! used to cost 3 process spawns per path per cycle. Reading `HEAD` / `config`
//! straight off disk costs microseconds and needs no git binary at all.
//!
//! Resolution mirrors what the spawned commands returned:
//! - repo_root  ⇔ `git rev-parse --show-toplevel` (worktree-aware)
//! - branch     ⇔ `git branch --show-current` (detached HEAD → None)
//! - repo_name  ⇔ origin URL last path component, falling back to the last
//!   component of repo_root
//!
//! Bare repositories resolve to None, matching `--show-toplevel` failing
//! outside a work tree.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub repo_root: String,
    pub repo_name: String,
    pub branch: Option<String>,
}

/// Stable per-path facts cached across polling cycles. `branch` is excluded:
/// it changes on checkout, so it is re-read from HEAD on every lookup.
#[derive(Debug, Clone)]
struct StableRepoInfo {
    repo_root: String,
    repo_name: String,
    /// Directory containing this work tree's HEAD (worktree-specific gitdir).
    gitdir: PathBuf,
}

struct CacheEntry {
    /// None = path was not inside a git work tree when last probed.
    resolved: Option<StableRepoInfo>,
    checked_at: Instant,
}

/// Non-git directories are re-probed after this TTL so a later `git init`
/// (or a worktree appearing at the same path) is eventually picked up.
const NEGATIVE_TTL: Duration = Duration::from_secs(30);

static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();

/// Cached resolver for long-running processes (the GUI poller). The stable
/// part (repo_root, repo_name) is cached per path; the branch is re-read from
/// HEAD on every call so checkouts show up within one polling cycle. If the
/// gitdir disappears (worktree pruned, repo deleted), the entry is dropped
/// and the path is re-resolved from scratch.
pub fn resolve_git_info(current_path: &str) -> Option<GitInfo> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = match cache.lock() {
        Ok(g) => g,
        Err(_) => return resolve_git_info_uncached(Path::new(current_path)),
    };

    if let Some(entry) = guard.get(current_path) {
        match &entry.resolved {
            Some(stable) => {
                // HEAD readable → repo still exists; branch is current.
                if let Ok(head) = fs::read_to_string(stable.gitdir.join("HEAD")) {
                    return Some(GitInfo {
                        repo_root: stable.repo_root.clone(),
                        repo_name: stable.repo_name.clone(),
                        branch: parse_head_branch(&head),
                    });
                }
                // gitdir vanished — fall through to a fresh resolve below.
            }
            None => {
                if entry.checked_at.elapsed() < NEGATIVE_TTL {
                    return None;
                }
            }
        }
    }

    let path = Path::new(current_path);
    let fresh = resolve_dirs(path).map(|dirs| {
        let repo_name = read_origin_url(&dirs.common_dir)
            .and_then(|url| extract_repo_name_from_url(&url))
            .unwrap_or_else(|| last_component(&dirs.repo_root));
        StableRepoInfo {
            repo_root: dirs.repo_root,
            repo_name,
            gitdir: dirs.gitdir,
        }
    });

    let result = fresh.as_ref().map(|stable| GitInfo {
        repo_root: stable.repo_root.clone(),
        repo_name: stable.repo_name.clone(),
        branch: fs::read_to_string(stable.gitdir.join("HEAD"))
            .ok()
            .and_then(|h| parse_head_branch(&h)),
    });

    guard.insert(
        current_path.to_string(),
        CacheEntry {
            resolved: fresh,
            checked_at: Instant::now(),
        },
    );

    result
}

/// One-shot resolver for short-lived processes (CLI hooks) where caching has
/// no benefit.
pub fn resolve_git_info_uncached(path: &Path) -> Option<GitInfo> {
    let dirs = resolve_dirs(path)?;
    let repo_name = read_origin_url(&dirs.common_dir)
        .and_then(|url| extract_repo_name_from_url(&url))
        .unwrap_or_else(|| last_component(&dirs.repo_root));
    let branch = fs::read_to_string(dirs.gitdir.join("HEAD"))
        .ok()
        .and_then(|h| parse_head_branch(&h));
    Some(GitInfo {
        repo_root: dirs.repo_root,
        repo_name,
        branch,
    })
}

struct ResolvedDirs {
    repo_root: String,
    /// Where this work tree's HEAD lives. For worktrees this is the
    /// worktree-private gitdir (`<main>/.git/worktrees/<name>`).
    gitdir: PathBuf,
    /// Where the shared config lives (`commondir` for worktrees, the gitdir
    /// itself otherwise).
    common_dir: PathBuf,
}

/// Walk up from `start` looking for a `.git` entry, mirroring git's own
/// repository discovery for work trees.
fn resolve_dirs(start: &Path) -> Option<ResolvedDirs> {
    let mut dir = start;
    loop {
        let dot_git = dir.join(".git");
        let meta = fs::symlink_metadata(&dot_git).ok();
        if let Some(meta) = meta {
            if meta.is_dir() {
                return Some(ResolvedDirs {
                    repo_root: canonical_root(dir),
                    gitdir: dot_git.clone(),
                    common_dir: dot_git,
                });
            }
            if meta.is_file() {
                // Worktree or submodule: `.git` is a file containing
                // `gitdir: <path>` (relative paths resolve against `dir`).
                let content = fs::read_to_string(&dot_git).ok()?;
                let pointed = content
                    .lines()
                    .find_map(|l| l.strip_prefix("gitdir:"))
                    .map(str::trim)?;
                let gitdir = if Path::new(pointed).is_absolute() {
                    PathBuf::from(pointed)
                } else {
                    dir.join(pointed)
                };
                // Worktree gitdirs carry a `commondir` file pointing at the
                // shared `.git`; submodule gitdirs don't (they are complete).
                let common_dir = match fs::read_to_string(gitdir.join("commondir")) {
                    Ok(c) => {
                        let c = c.trim();
                        if Path::new(c).is_absolute() {
                            PathBuf::from(c)
                        } else {
                            gitdir.join(c)
                        }
                    }
                    Err(_) => gitdir.clone(),
                };
                return Some(ResolvedDirs {
                    repo_root: canonical_root(dir),
                    gitdir,
                    common_dir,
                });
            }
        }
        dir = dir.parent()?;
    }
}

/// `git rev-parse --show-toplevel` returns a symlink-resolved path; match
/// that so repo grouping / mute keys stay stable regardless of how the user
/// cd'd into the repo.
fn canonical_root(dir: &Path) -> String {
    fs::canonicalize(dir)
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// `ref: refs/heads/<branch>` → Some(branch); detached HEAD (bare SHA) → None.
fn parse_head_branch(head: &str) -> Option<String> {
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .filter(|b| !b.is_empty())
        .map(str::to_string)
}

/// Read `[remote "origin"] url` from `<common_dir>/config` with a minimal
/// line parser. `[include]`/`[includeIf]` indirection is not followed — when
/// origin lives there the caller falls back to the repo_root component, same
/// as the previous `git remote get-url origin` failure path.
fn read_origin_url(common_dir: &Path) -> Option<String> {
    let config = fs::read_to_string(common_dir.join("config")).ok()?;
    parse_origin_url(&config)
}

fn parse_origin_url(config: &str) -> Option<String> {
    let mut in_origin = false;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Section names are case-insensitive, subsection names are not.
            in_origin = trimmed.to_ascii_lowercase().starts_with("[remote")
                && trimmed.contains("\"origin\"");
            continue;
        }
        if !in_origin {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("url") else {
            continue;
        };
        let rest = rest.trim_start();
        if let Some(value) = rest.strip_prefix('=') {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract repository name from a git remote URL.
/// Supports HTTPS (`https://github.com/owner/repo.git`) and SSH (`git@github.com:owner/repo.git`).
pub fn extract_repo_name_from_url(url: &str) -> Option<String> {
    let path = if let Some(rest) = url.strip_prefix("git@") {
        // SSH: git@github.com:owner/repo.git
        rest.split(':').nth(1)?
    } else {
        // HTTPS: https://github.com/owner/repo.git
        url.split("://").nth(1).unwrap_or(url)
    };
    let name = path.rsplit('/').next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn last_component(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Lay out a fake repository on disk without invoking git.
    fn make_repo(root: &Path, branch: &str, origin_url: Option<&str>) {
        let git = root.join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), format!("ref: refs/heads/{}\n", branch)).unwrap();
        let mut config = String::from("[core]\n    bare = false\n");
        if let Some(url) = origin_url {
            config.push_str(&format!("[remote \"origin\"]\n\turl = {}\n", url));
        }
        fs::write(git.join("config"), config).unwrap();
    }

    #[test]
    fn resolves_normal_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("myrepo");
        make_repo(&root, "main", Some("git@github.com:owner/cool-repo.git"));

        let info = resolve_git_info_uncached(&root).unwrap();
        assert_eq!(
            PathBuf::from(&info.repo_root),
            fs::canonicalize(&root).unwrap()
        );
        assert_eq!(info.repo_name, "cool-repo");
        assert_eq!(info.branch.as_deref(), Some("main"));
    }

    #[test]
    fn resolves_from_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("myrepo");
        make_repo(&root, "develop", None);
        let sub = root.join("src/deep/nested");
        fs::create_dir_all(&sub).unwrap();

        let info = resolve_git_info_uncached(&sub).unwrap();
        assert_eq!(
            PathBuf::from(&info.repo_root),
            fs::canonicalize(&root).unwrap()
        );
        // No origin → falls back to repo_root last component.
        assert_eq!(info.repo_name, "myrepo");
        assert_eq!(info.branch.as_deref(), Some("develop"));
    }

    #[test]
    fn detached_head_has_no_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        make_repo(&root, "main", None);
        fs::write(
            root.join(".git/HEAD"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )
        .unwrap();

        let info = resolve_git_info_uncached(&root).unwrap();
        assert_eq!(info.branch, None);
    }

    #[test]
    fn branch_with_slashes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        make_repo(&root, "feature/foo/bar", None);

        let info = resolve_git_info_uncached(&root).unwrap();
        assert_eq!(info.branch.as_deref(), Some("feature/foo/bar"));
    }

    #[test]
    fn non_git_directory_resolves_none() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("plain");
        fs::create_dir_all(&dir).unwrap();
        assert!(resolve_git_info_uncached(&dir).is_none());
    }

    #[test]
    fn resolves_linked_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        make_repo(&main, "main", Some("https://github.com/owner/wt-repo.git"));

        // Linked worktree layout as `git worktree add` produces it.
        let wt_gitdir = main.join(".git/worktrees/feature-x");
        fs::create_dir_all(&wt_gitdir).unwrap();
        fs::write(wt_gitdir.join("HEAD"), "ref: refs/heads/feature-x\n").unwrap();
        fs::write(wt_gitdir.join("commondir"), "../..\n").unwrap();

        let wt_root = tmp.path().join("feature-x");
        fs::create_dir_all(&wt_root).unwrap();
        fs::write(
            wt_root.join(".git"),
            format!("gitdir: {}\n", wt_gitdir.to_string_lossy()),
        )
        .unwrap();

        let info = resolve_git_info_uncached(&wt_root).unwrap();
        assert_eq!(
            PathBuf::from(&info.repo_root),
            fs::canonicalize(&wt_root).unwrap()
        );
        // origin comes from the shared config via commondir.
        assert_eq!(info.repo_name, "wt-repo");
        assert_eq!(info.branch.as_deref(), Some("feature-x"));
    }

    #[test]
    fn cached_lookup_tracks_branch_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("switching");
        make_repo(&root, "main", None);
        let key = root.to_string_lossy().into_owned();

        let first = resolve_git_info(&key).unwrap();
        assert_eq!(first.branch.as_deref(), Some("main"));

        fs::write(root.join(".git/HEAD"), "ref: refs/heads/hotfix\n").unwrap();
        let second = resolve_git_info(&key).unwrap();
        assert_eq!(second.branch.as_deref(), Some("hotfix"));
    }

    #[test]
    fn cached_lookup_recovers_from_deleted_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("doomed");
        make_repo(&root, "main", None);
        let key = root.to_string_lossy().into_owned();

        assert!(resolve_git_info(&key).is_some());
        fs::remove_dir_all(root.join(".git")).unwrap();
        assert!(resolve_git_info(&key).is_none());
    }

    #[test]
    fn parses_origin_url_variants() {
        assert_eq!(
            parse_origin_url("[remote \"origin\"]\n\turl = git@github.com:o/r.git\n"),
            Some("git@github.com:o/r.git".to_string())
        );
        // url under a different remote must not match
        assert_eq!(
            parse_origin_url("[remote \"upstream\"]\n\turl = git@github.com:o/up.git\n"),
            None
        );
        // section after origin ends the origin scope
        assert_eq!(
            parse_origin_url(
                "[remote \"origin\"]\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n[branch \"main\"]\n\turl = bogus\n"
            ),
            None
        );
    }

    #[test]
    fn extracts_repo_name_from_urls() {
        assert_eq!(
            extract_repo_name_from_url("https://github.com/owner/repo.git").as_deref(),
            Some("repo")
        );
        assert_eq!(
            extract_repo_name_from_url("git@github.com:owner/repo.git").as_deref(),
            Some("repo")
        );
        assert_eq!(
            extract_repo_name_from_url("https://github.com/owner/repo").as_deref(),
            Some("repo")
        );
    }

    /// Cross-check against the real git binary when available (covers the
    /// formats the hand-rolled parser must stay compatible with).
    #[test]
    fn matches_real_git_output() {
        let git = which_git();
        let Some(git) = git else {
            eprintln!("git binary not found; skipping cross-check test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("real");
        fs::create_dir_all(&root).unwrap();
        let run = |args: &[&str], cwd: &Path| {
            let out = std::process::Command::new(&git)
                .args(args)
                .current_dir(cwd)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        run(&["init", "-b", "main", "."], &root);
        run(
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:owner/real-repo.git",
            ],
            &root,
        );
        // A commit is required before `git worktree add` can branch off HEAD.
        run(
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ],
            &root,
        );

        let expected_root = run(&["rev-parse", "--show-toplevel"], &root);
        let expected_branch = run(&["branch", "--show-current"], &root);

        let info = resolve_git_info_uncached(&root).unwrap();
        // Compare canonicalized paths: macOS tempdirs sit behind /private symlinks.
        assert_eq!(
            fs::canonicalize(&info.repo_root).unwrap(),
            fs::canonicalize(&expected_root).unwrap()
        );
        assert_eq!(info.branch.as_deref(), Some(expected_branch.as_str()));
        assert_eq!(info.repo_name, "real-repo");

        // Worktree cross-check
        let wt = tmp.path().join("real-wt");
        run(
            &[
                "worktree",
                "add",
                "-b",
                "feature-y",
                wt.to_string_lossy().as_ref(),
            ],
            &root,
        );
        let expected_wt_root = run(&["rev-parse", "--show-toplevel"], &wt);
        let expected_wt_branch = run(&["branch", "--show-current"], &wt);
        let wt_info = resolve_git_info_uncached(&wt).unwrap();
        assert_eq!(
            fs::canonicalize(&wt_info.repo_root).unwrap(),
            fs::canonicalize(&expected_wt_root).unwrap()
        );
        assert_eq!(wt_info.branch.as_deref(), Some(expected_wt_branch.as_str()));
        assert_eq!(wt_info.repo_name, "real-repo");
    }

    fn which_git() -> Option<std::path::PathBuf> {
        [
            "/usr/bin/git",
            "/opt/homebrew/bin/git",
            "/usr/local/bin/git",
        ]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
    }
}
