//! Deterministic unified diff parser and GitHub inline comment hunk validator.
//!
//! GitHub REST `POST /repos/{owner}/{repo}/pulls/{number}/reviews` rejects the entire review
//! with `HTTP 422 Unprocessable Entity` if even one inline comment points to a `(path, line)`
//! outside the PR's unified diff hunks (or spans across two distinct hunks).
//! This module parses `diff.patch` and classifies every finding into a guaranteed-valid inline
//! diff comment or a top-level Cross-File / Out-of-Hunk summary observation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Inclusive right-side (new-file) line range reachable inside a single unified diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunkRange {
    pub start_line: u32,
    pub end_line: u32,
}

impl DiffHunkRange {
    pub const fn contains(self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    pub const fn contains_span(self, start: u32, end: u32) -> bool {
        start <= end && start >= self.start_line && end <= self.end_line
    }
}

/// Map of relative repository paths (`apps/access/src/page.tsx`) to their right-side diff hunks.
#[derive(Debug, Clone, Default)]
pub struct ParsedDiffMap {
    pub files: BTreeMap<String, Vec<DiffHunkRange>>,
}

/// Validated placement decision for a review observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlinePlacement {
    /// Safe to attach as a single-line GitHub inline review comment (`side: "RIGHT"`).
    SingleLine { path: String, line: u32 },
    /// Safe to attach as a multi-line GitHub inline review comment (`side: "RIGHT"`).
    MultiLine {
        path: String,
        start_line: u32,
        line: u32,
    },
    /// Line falls outside the PR's diff hunks; render in the top-level summary instead.
    OutOfHunk {
        path: String,
        start_line: Option<u32>,
        end_line: Option<u32>,
    },
}

impl ParsedDiffMap {
    /// Parses a standard `git diff` / GitHub `application/vnd.github.v3.diff` patch string.
    pub fn parse(patch: &str) -> Self {
        let mut files: BTreeMap<String, Vec<DiffHunkRange>> = BTreeMap::new();
        let mut current_path: Option<String> = None;
        let mut in_hunk = false;
        let mut right_cursor: u32 = 0;
        let mut hunk_min_right: Option<u32> = None;
        let mut hunk_max_right: Option<u32> = None;

        let flush_hunk = |files: &mut BTreeMap<String, Vec<DiffHunkRange>>,
                          path: &Option<String>,
                          min_r: &mut Option<u32>,
                          max_r: &mut Option<u32>| {
            if let (Some(p), Some(start_line), Some(end_line)) = (path.as_ref(), *min_r, *max_r) {
                files.entry(p.clone()).or_default().push(DiffHunkRange {
                    start_line,
                    end_line,
                });
            }
            *min_r = None;
            *max_r = None;
        };

        for raw_line in patch.lines() {
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

            if let Some(rest) = line.strip_prefix("diff --git ") {
                flush_hunk(
                    &mut files,
                    &current_path,
                    &mut hunk_min_right,
                    &mut hunk_max_right,
                );
                in_hunk = false;
                current_path = parse_diff_git_b_path(rest);
                continue;
            }

            if let Some(plus_path) = line.strip_prefix("+++ ") {
                flush_hunk(
                    &mut files,
                    &current_path,
                    &mut hunk_min_right,
                    &mut hunk_max_right,
                );
                in_hunk = false;
                if plus_path.trim() == "/dev/null" {
                    // Deleted file has no RIGHT-side lines.
                    current_path = None;
                } else if let Some(stripped) = plus_path.trim().strip_prefix("b/") {
                    current_path = Some(stripped.to_string());
                }
                continue;
            }

            if line.starts_with("--- ") {
                continue;
            }

            if let Some(after_at) = line.strip_prefix("@@ ") {
                flush_hunk(
                    &mut files,
                    &current_path,
                    &mut hunk_min_right,
                    &mut hunk_max_right,
                );
                if let Some(new_start) = parse_hunk_header_new_start(after_at) {
                    in_hunk = true;
                    right_cursor = new_start;
                } else {
                    in_hunk = false;
                }
                continue;
            }

            if !in_hunk || current_path.is_none() {
                continue;
            }

            if line.starts_with('+') || line.starts_with(' ') {
                // Added (`+`) and context (` `) lines both exist on `side: "RIGHT"` in GitHub's diff view!
                if hunk_min_right.is_none() {
                    hunk_min_right = Some(right_cursor);
                }
                hunk_max_right = Some(right_cursor);
                right_cursor = right_cursor.saturating_add(1);
            } else if line.starts_with('-') {
                // Removed (`-`) line only exists on LEFT; right_cursor does not advance.
            } else if line.starts_with('\\') {
                // `\ No newline at end of file`
            }
        }

        flush_hunk(
            &mut files,
            &current_path,
            &mut hunk_min_right,
            &mut hunk_max_right,
        );
        Self { files }
    }

    /// Classifies `(path, start_line, end_line)` into a guaranteed-safe GitHub inline comment or out-of-hunk fallback.
    pub fn classify(
        &self,
        raw_path: &str,
        start_line: Option<u32>,
        end_line: Option<u32>,
    ) -> InlinePlacement {
        let normalized = raw_path.trim().trim_start_matches("./").replace('\\', "/");

        let target_end = match end_line.or(start_line) {
            Some(line) if line > 0 => line,
            _ => {
                return InlinePlacement::OutOfHunk {
                    path: normalized,
                    start_line,
                    end_line,
                };
            }
        };
        let target_start = start_line.unwrap_or(target_end).max(1);

        let Some(hunks) = self.files.get(&normalized) else {
            return InlinePlacement::OutOfHunk {
                path: normalized,
                start_line: Some(target_start),
                end_line: Some(target_end),
            };
        };

        if target_start < target_end
            && hunks
                .iter()
                .any(|h| h.contains_span(target_start, target_end))
        {
            return InlinePlacement::MultiLine {
                path: normalized,
                start_line: target_start,
                line: target_end,
            };
        }

        if hunks.iter().any(|h| h.contains(target_end)) {
            return InlinePlacement::SingleLine {
                path: normalized,
                line: target_end,
            };
        }

        if hunks.iter().any(|h| h.contains(target_start)) {
            return InlinePlacement::SingleLine {
                path: normalized,
                line: target_start,
            };
        }

        InlinePlacement::OutOfHunk {
            path: normalized,
            start_line: Some(target_start),
            end_line: Some(target_end),
        }
    }

    pub fn changed_files(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }
}

fn parse_diff_git_b_path(rest: &str) -> Option<String> {
    let idx = rest.rfind(" b/")?;
    Some(rest[idx + 3..].trim().to_string())
}

fn parse_hunk_header_new_start(after_at: &str) -> Option<u32> {
    // Format: `-12,5 +14,8 @@ optional context` or `-0,0 +1 @@`
    let header_end = after_at.find("@@")?;
    let spec = after_at[..header_end].trim();
    let plus_part = spec.split_whitespace().find(|tok| tok.starts_with('+'))?;
    let nums = plus_part.strip_prefix('+')?;
    let (start_str, count_opt) = match nums.split_once(',') {
        Some((s, c)) => (s, Some(c)),
        None => (nums, None),
    };
    if count_opt == Some("0") {
        return None;
    }
    start_str.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str = "\
diff --git a/apps/access/src/page.tsx b/apps/access/src/page.tsx
index 1111111..2222222 100644
--- a/apps/access/src/page.tsx
+++ b/apps/access/src/page.tsx
@@ -10,4 +10,6 @@ export function Page() {
   const a = 1;
-  const b = 2;
+  const b = 3;
+  const c = 4;
+  const d = 5;
   return a + b;
@@ -40,2 +42,3 @@ export function Footer() {
   return null;
+  // unreachable
 }
";

    #[test]
    fn parses_hunks_and_classifies_single_multi_and_out_of_hunk() {
        let map = ParsedDiffMap::parse(SAMPLE_DIFF);
        assert_eq!(map.changed_files(), vec!["apps/access/src/page.tsx"]);

        // Multi-line inside hunk 1 (lines 10..15)
        assert_eq!(
            map.classify("apps/access/src/page.tsx", Some(11), Some(13)),
            InlinePlacement::MultiLine {
                path: "apps/access/src/page.tsx".to_string(),
                start_line: 11,
                line: 13,
            }
        );

        // Span crossing hunk 1 (10..15) into hunk 2 (42..44) degrades safely to single-line on line 43!
        assert_eq!(
            map.classify("apps/access/src/page.tsx", Some(12), Some(43)),
            InlinePlacement::SingleLine {
                path: "apps/access/src/page.tsx".to_string(),
                line: 43,
            }
        );

        // Untouched line 28 promotes safely to OutOfHunk instead of crashing GitHub API with 422
        assert_eq!(
            map.classify("apps/access/src/page.tsx", Some(28), Some(28)),
            InlinePlacement::OutOfHunk {
                path: "apps/access/src/page.tsx".to_string(),
                start_line: Some(28),
                end_line: Some(28),
            }
        );
    }
}
