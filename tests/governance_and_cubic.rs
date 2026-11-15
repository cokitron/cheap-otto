#![forbid(unsafe_code)]

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
fn open_source_governance_and_safety_files_are_present() -> Result<(), String> {
    let root = repo_root();
    let required_files = [
        "LICENSE",
        "README.md",
        "CLAUDE.md",
        "AGENTS.md",
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
