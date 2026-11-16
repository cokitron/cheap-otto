#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct ParsedCustomRule {
    name: String,
    description: String,
    file_paths: Vec<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn parse_cubic_custom_rules(yaml: &str) -> Vec<ParsedCustomRule> {
    let mut rules: Vec<ParsedCustomRule> = Vec::new();
    let mut in_custom_rules = false;
    let mut in_description_block = false;
    let mut in_file_paths = false;

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ').count();

        if indent == 0 && !line.starts_with(' ') {
            in_custom_rules = false;
            in_description_block = false;
            in_file_paths = false;
        }
        if indent == 2 && trimmed == "custom_rules:" {
            in_custom_rules = true;
            in_description_block = false;
            in_file_paths = false;
            continue;
        }
        if !in_custom_rules {
            continue;
        }
        if indent == 2 && trimmed != "custom_rules:" {
            break;
        }

        if indent == 4 && trimmed.starts_with("- name:") {
            in_description_block = false;
            in_file_paths = false;
            let raw_name = trimmed.trim_start_matches("- name:").trim();
            rules.push(ParsedCustomRule {
                name: raw_name.trim_matches('"').to_string(),
                description: String::new(),
                file_paths: Vec::new(),
            });
            continue;
        }

        let Some(current) = rules.last_mut() else {
            continue;
        };

        if indent == 6 {
            in_description_block = false;
            in_file_paths = false;
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let rest = rest.trim();
                if rest == "|" || rest == ">" {
                    in_description_block = true;
                } else {
                    current.description = rest.trim_matches('"').to_string();
                }
            } else if trimmed == "file_paths:" {
                in_file_paths = true;
            }
            continue;
        }

        if in_description_block && indent >= 8 {
            if !current.description.is_empty() {
                current.description.push('\n');
            }
            current.description.push_str(trimmed);
        } else if in_file_paths && indent >= 8 && trimmed.starts_with("- ") {
            let rel = trimmed
                .trim_start_matches("- ")
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            current.file_paths.push(rel.to_string());
        }
    }

    rules
}

/// Keys that Cubic's schema allows in each object it closes with `additionalProperties: false`,
/// listed under the dotted path of that object in `cubic.yaml`.
///
/// One key outside these lists makes Cubic reject the whole file and review with the settings
/// saved in its dashboard, so none of the custom rules apply. Keep the lists in step with
/// <https://cubic.dev/schema/cubic-repository-config.schema.json>.
const CUBIC_CLOSED_OBJECTS: &[(&str, &[&str])] = &[
    (
        "reviews",
        &[
            "senior_reviewers",
            "enabled",
            "sensitivity",
            "incremental_commits",
            "check_drafts",
            "architecture_diagrams",
            "external_contributors_require_manual_review",
            "show_ai_feedback_buttons",
            "resolve_threads_when_addressed",
            "merge_confidence_summary",
            "auto_approve_behavior",
            "auto_approve",
            "auto_approve_custom_prompt",
            "auto_approve_rules",
            "ultrareview",
            "auto_ultrareview",
            "auto_ultrareview_custom_prompt",
            "auto_ultrareview_file_patterns",
            "custom_instructions",
            "ignore",
            "custom_rules",
        ],
    ),
    (
        "reviews.ignore",
        &[
            "files",
            "head_branches",
            "base_branches",
            "pr_labels",
            "pr_titles",
            "max_changed_lines",
        ],
    ),
    (
        "reviews.auto_approve_rules",
        &[
            "exclude",
            "only_files",
            "exclude_external_contributors",
            "allow_open_issues",
            "exclude_authors",
            "only_authors",
            "exclude_head_branches",
            "only_head_branches",
            "exclude_base_branches",
            "only_base_branches",
            "exclude_labels",
            "only_labels",
            "exclude_titles",
            "only_titles",
        ],
    ),
    (
        "reviews.custom_rules",
        &[
            "name",
            "description",
            "file_paths",
            "include",
            "exclude",
            "only_repositories",
            "exclude_repositories",
        ],
    ),
    (
        "pr_descriptions",
        &[
            "generate",
            "instructions",
            "cubic_review_link",
            "skip_if_author_description",
        ],
    ),
    (
        "issues",
        &[
            "fix_with_cubic_buttons",
            "pr_comment_fixes",
            "fix_commits_to_pr",
            "auto_fix_sign_commits",
            "coding_agent_provider",
            "auto_fix",
            "auto_fix_rules",
            "coding_agent_instructions",
        ],
    ),
    (
        "issues.auto_fix_rules",
        &[
            "exclude_authors",
            "only_authors",
            "exclude_head_branches",
            "only_head_branches",
            "exclude_base_branches",
            "only_base_branches",
            "exclude_labels",
            "only_labels",
            "exclude_titles",
            "only_titles",
        ],
    ),
];

/// Splits `key: value` (or a bare `key:`) at the first colon when the text before it is a plain
/// YAML key. Quoted scalars such as `"a: b"` are not keys.
fn split_yaml_key(text: &str) -> Option<(&str, &str)> {
    let (key, value) = text.split_once(':')?;
    let is_plain_key = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let value_is_separated = value.is_empty() || value.starts_with(' ');
    (is_plain_key && value_is_separated).then_some((key, value.trim()))
}

/// Lists every key in `yaml` that sits inside one of [`CUBIC_CLOSED_OBJECTS`] but is missing from
/// that object's list, as a dotted path such as `reviews.merge_confidence`.
///
/// It reads the block style that `cubic.yaml` is written in: nesting by indentation, `- `
/// sequence items, and `|` or `>` block scalars, whose text is skipped. Flow mappings
/// (`{a: 1}`) are not read.
fn unknown_cubic_keys(yaml: &str) -> Vec<String> {
    let mut unknown = Vec::new();
    // The keys that enclose the current line, each with the column it starts in.
    let mut enclosing: Vec<(usize, &str)> = Vec::new();
    let mut block_scalar_owner: Option<usize> = None;

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut column = line.chars().take_while(|c| *c == ' ').count();
        if let Some(owner) = block_scalar_owner {
            if column > owner {
                continue;
            }
            block_scalar_owner = None;
        }

        // In `- name: x` the key starts after the dash, level with the other keys of that item.
        let mut text = trimmed;
        while let Some(item) = text.strip_prefix("- ") {
            let item = item.trim_start();
            column += text.len() - item.len();
            text = item;
        }

        let Some((key, value)) = split_yaml_key(text) else {
            continue;
        };
        while enclosing.last().is_some_and(|(start, _)| *start >= column) {
            enclosing.pop();
        }
        let parent = enclosing
            .iter()
            .map(|(_, name)| *name)
            .collect::<Vec<_>>()
            .join(".");
        let rejected = CUBIC_CLOSED_OBJECTS
            .iter()
            .any(|(path, allowed)| *path == parent && !allowed.contains(&key));
        if rejected {
            unknown.push(format!("{parent}.{key}"));
        }
        if value.starts_with('|') || value.starts_with('>') {
            block_scalar_owner = Some(column);
        }
        enclosing.push((column, key));
    }

    unknown
}

#[test]
fn cubic_yaml_conforms_to_schema_budgets_and_tender_coexistence() -> Result<(), String> {
    let root = repo_root();
    let cubic_path = root.join("cubic.yaml");
    let raw = fs::read_to_string(&cubic_path)
        .map_err(|e| format!("failed reading {}: {}", cubic_path.display(), e))?;

    if !raw.contains("https://cubic.dev/schema/cubic-repository-config.schema.json") {
        return Err("cubic.yaml must declare official JSON schema header".to_string());
    }
    if !raw.lines().any(|l| l.trim() == "version: 1") {
        return Err("cubic.yaml must set `version: 1`".to_string());
    }
    if !raw.lines().any(|l| l.trim() == "auto_fix: \"off\"") {
        return Err(
            "cubic.yaml must set `auto_fix: \"off\"` so Cubic never races Code Tender".to_string(),
        );
    }

    let rules = parse_cubic_custom_rules(&raw);
    if rules.is_empty() || rules.len() > 5 {
        return Err(format!(
            "cubic.yaml must define 1..=5 custom_rules, found {}",
            rules.len()
        ));
    }

    for rule in &rules {
        if rule.name.is_empty() || rule.description.is_empty() {
            return Err(format!(
                "custom rule `{}` must have non-empty name and description",
                rule.name
            ));
        }
        if rule.file_paths.is_empty() || rule.file_paths.len() > 10 {
            return Err(format!(
                "custom rule `{}` must list 1..=10 instruction file_paths, found {}",
                rule.name,
                rule.file_paths.len()
            ));
        }

        let mut total_chars = rule.description.chars().count();
        for rel in &rule.file_paths {
            let full = root.join(Path::new(rel));
            let content = fs::read_to_string(&full).map_err(|e| {
                format!(
                    "custom rule `{}` references missing file `{}`: {}",
                    rule.name, rel, e
                )
            })?;
            total_chars = total_chars.saturating_add(content.chars().count());
        }
        if total_chars > 10_000 {
            return Err(format!(
                "custom rule `{}` exceeds Cubic's 10,000-character budget ({} chars)",
                rule.name, total_chars
            ));
        }
    }

    Ok(())
}

#[test]
fn cubic_yaml_uses_only_keys_the_cubic_schema_allows() -> Result<(), String> {
    let cubic_path = repo_root().join("cubic.yaml");
    let raw = fs::read_to_string(&cubic_path)
        .map_err(|e| format!("failed reading {}: {}", cubic_path.display(), e))?;

    let unknown = unknown_cubic_keys(&raw);
    if !unknown.is_empty() {
        return Err(format!(
            "cubic.yaml uses keys the Cubic schema rejects, so Cubic ignores the whole file: {}",
            unknown.join(", ")
        ));
    }
    Ok(())
}

#[test]
fn unknown_cubic_keys_flags_misnamed_keys_and_skips_block_scalar_text() {
    let yaml = [
        "version: 1",
        "reviews:",
        "  merge_confidence: true",
        "  custom_instructions: |",
        "    not_a_key: prose inside a block scalar",
        "  ignore:",
        "    files:",
        "      - \"a: b\"",
        "    labels:",
        "      - \"skip\"",
        "  custom_rules:",
        "    - name: \"x\"",
        "      description: |",
        "        severity: also prose",
        "      severity: high",
        "pr_descriptions:",
        "  generate: true",
        "issues:",
        "  auto_fix: \"off\"",
        "  autofix: \"off\"",
        "unlisted_section:",
        "  anything: 1",
    ]
    .join("\n");

    assert_eq!(
        unknown_cubic_keys(&yaml),
        [
            "reviews.merge_confidence",
            "reviews.ignore.labels",
            "reviews.custom_rules.severity",
            "issues.autofix",
        ]
    );
}

#[test]
fn open_source_governance_and_safety_files_are_present() -> Result<(), String> {
    let root = repo_root();
    let required_files = [
        "LICENSE",
        "README.md",
        "CLAUDE.md",
        "AGENTS.md",
        ".agents/skills.json",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "CODE_OF_CONDUCT.md",
        "cubic.yaml",
        ".ai/architecture-and-invariants.md",
        ".ai/security-and-tender-boundaries.md",
        ".claude/rules/06-git-workflow-and-governance.md",
        ".github/CODEOWNERS",
        ".github/PULL_REQUEST_TEMPLATE.md",
        ".github/dependabot.yml",
        ".github/ISSUE_TEMPLATE/bug_report.yml",
        ".github/ISSUE_TEMPLATE/feature_request.yml",
        ".github/ISSUE_TEMPLATE/config.yml",
        ".github/workflows/ci.yml",
        "crates/code-reviewer/config/repos.example.toml",
    ];

    for rel in required_files {
        let p = root.join(rel);
        if !p.is_file() {
            return Err(format!("missing required governance file: {}", rel));
        }
    }

    let skills_registry = fs::read_to_string(root.join(".agents/skills.json"))
        .map_err(|e| format!("failed reading .agents/skills.json: {e}"))?;
    let skills_registry: serde_json::Value = serde_json::from_str(&skills_registry)
        .map_err(|e| format!(".agents/skills.json is not valid JSON: {e}"))?;
    let registers_code_reviewer_skills = skills_registry
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry.get("path").and_then(serde_json::Value::as_str)
                    == Some("crates/code-reviewer/.agents/skills")
            })
        });
    if !registers_code_reviewer_skills {
        return Err(
            ".agents/skills.json must register `crates/code-reviewer/.agents/skills` in `entries`"
                .to_string(),
        );
    }

    for rust_entry in [
        "src/lib.rs",
        "src/main.rs",
        "crates/code-reviewer/src/main.rs",
    ] {
        let content = fs::read_to_string(root.join(rust_entry))
            .map_err(|e| format!("failed reading {}: {}", rust_entry, e))?;
        if !content
            .lines()
            .any(|l| l.trim() == "#![forbid(unsafe_code)]")
        {
            return Err(format!(
                "`{}` must enforce `#![forbid(unsafe_code)]`",
                rust_entry
            ));
        }
    }

    Ok(())
}

#[test]
fn agents_md_stays_under_antigravity_inline_cap_and_skills_mirror() -> Result<(), String> {
    const ANTIGRAVITY_RULE_CAP_BYTES: usize = 24_000;
    let root = repo_root();
    let agents_md = fs::read_to_string(root.join("AGENTS.md"))
        .map_err(|e| format!("failed reading AGENTS.md: {e}"))?;

    let mut expanded_bytes = 0usize;
    for line in agents_md.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@[")
            && let Some((_, after_bracket)) = rest.split_once("](")
            && let Some((rel_path, "")) = after_bracket.split_once(')')
        {
            let inc = fs::read(root.join(rel_path)).map_err(|e| {
                format!("AGENTS.md @[...] include `{rel_path}` could not be read: {e}")
            })?;
            expanded_bytes = expanded_bytes.saturating_add(inc.len()).saturating_add(1);
        } else {
            expanded_bytes = expanded_bytes.saturating_add(line.len()).saturating_add(1);
        }
    }

    if expanded_bytes >= ANTIGRAVITY_RULE_CAP_BYTES {
        return Err(format!(
            "AGENTS.md expands to {expanded_bytes} bytes (>= {ANTIGRAVITY_RULE_CAP_BYTES} byte Antigravity cap); move path-scoped rules to standard markdown links"
        ));
    }

    let agy_root = root.join("crates/code-reviewer/.agents/skills");
    let claude_root = root.join("crates/code-reviewer/.claude/skills");
    let mut agy_tree = BTreeMap::new();
    collect_tree(&agy_root, &agy_root, &mut agy_tree)?;
    let mut claude_tree = BTreeMap::new();
    collect_tree(&claude_root, &claude_root, &mut claude_tree)?;
    if let Some(drift) = first_tree_drift(&agy_tree, &claude_tree) {
        return Err(format!(
            "skills drifted between .agents/skills and .claude/skills: {drift}"
        ));
    }

    Ok(())
}

/// Reads every file under `dir` into `out`, keyed by its `/`-joined path relative to `base`.
fn collect_tree(
    base: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed listing {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("failed listing {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            collect_tree(base, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(base)
            .map_err(|e| format!("{} is outside {}: {e}", path.display(), base.display()))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let bytes =
            fs::read(&path).map_err(|e| format!("failed reading {}: {e}", path.display()))?;
        out.insert(rel, bytes);
    }
    Ok(())
}

/// Names the first path that is missing from, extra in, or different between the mirrored trees.
fn first_tree_drift(
    agy: &BTreeMap<String, Vec<u8>>,
    claude: &BTreeMap<String, Vec<u8>>,
) -> Option<String> {
    for (rel, agy_bytes) in agy {
        match claude.get(rel) {
            None => return Some(format!("`{rel}` is missing from .claude/skills")),
            Some(claude_bytes) if claude_bytes != agy_bytes => {
                return Some(format!("`{rel}` differs between the two trees"));
            }
            Some(_) => {}
        }
    }
    claude
        .keys()
        .find(|rel| !agy.contains_key(*rel))
        .map(|rel| format!("`{rel}` is extra in .claude/skills (missing from .agents/skills)"))
}

#[test]
fn first_tree_drift_reports_missing_extra_and_differing_paths() {
    let tree = |files: &[(&str, &str)]| -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.as_bytes().to_vec()))
            .collect()
    };
    let base = tree(&[("a/SKILL.md", "one"), ("b/SKILL.md", "two")]);
    assert_eq!(first_tree_drift(&base, &base.clone()), None);
    assert!(
        first_tree_drift(&base, &tree(&[("a/SKILL.md", "one")]))
            .is_some_and(|m| m.contains("b/SKILL.md") && m.contains("missing"))
    );
    assert!(
        first_tree_drift(
            &base,
            &tree(&[("a/SKILL.md", "one"), ("b/SKILL.md", "two"), ("c/x.md", "")])
        )
        .is_some_and(|m| m.contains("c/x.md") && m.contains("extra"))
    );
    assert!(
        first_tree_drift(&base, &tree(&[("a/SKILL.md", "one"), ("b/SKILL.md", "2")]))
            .is_some_and(|m| m.contains("b/SKILL.md") && m.contains("differs"))
    );
}
