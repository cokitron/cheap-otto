//! Local M1 review document persistence + private Google Drive (`gdocs` & `gdrive`) synchronization.
//!
//! Every PR review writes:
//! 1. Local disk artifacts under `<workspace>/reviews/<owner>__<repo>/pr-<N>/` (`latest-review.md` + `observations.json`)
//! 2. A living Google Doc inside the private `Internal CI — Code Reviews / <repo>` folder so any
//!    agent (Claude Code, Antigravity, Codex) on any machine can read open observations and fix them.

use crate::config::{WatchedRepoConfig, WorkspaceConfig, expand_tilde, repo_fs_slug};
use crate::state::{TrackedPrState, render_agent_drive_document};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriveSyncError {
    #[error("failed to write local review artifact at {path}: {source}")]
    LocalIo {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize observations JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Drive CLI `{bin}` failed ({code}): {stderr}")]
    CliFailed {
        bin: String,
        code: i32,
        stderr: String,
    },
    #[error("could not extract Google Doc ID from `gdocs` output: {0}")]
    MissingDocId(String),
}

#[derive(Debug, Clone)]
pub struct SyncedReviewLocations {
    pub local_markdown_path: PathBuf,
    pub gdrive_doc_url: Option<String>,
    pub drive_warning: Option<String>,
}

/// Writes local M1 review files and synchronizes the living Google Doc in Drive.
///
/// If `gdocs`/`gdrive` on the M1 have not completed interactive OAuth yet, local artifacts still
/// succeed and `drive_warning` reports the exact command to enable Drive sync.
pub fn sync_pr_review_artifacts(
    workspace_root: &Path,
    cfg: &WorkspaceConfig,
    repo_cfg: &WatchedRepoConfig,
    pr: &mut TrackedPrState,
) -> Result<SyncedReviewLocations, DriveSyncError> {
    let dir = workspace_root
        .join("reviews")
        .join(repo_fs_slug(&pr.repo))
        .join(format!("pr-{}", pr.pr_number));
    fs::create_dir_all(&dir).map_err(|source| DriveSyncError::LocalIo {
        path: dir.clone(),
        source,
    })?;

    let md_content = render_agent_drive_document(pr);
    let md_path = dir.join("latest-review.md");
    fs::write(&md_path, &md_content).map_err(|source| DriveSyncError::LocalIo {
        path: md_path.clone(),
        source,
    })?;

    let json_path = dir.join("observations.json");
    let json_body = serde_json::to_string_pretty(pr)?;
    fs::write(&json_path, json_body).map_err(|source| DriveSyncError::LocalIo {
        path: json_path.clone(),
        source,
    })?;

    if !cfg.gdrive.enabled {
        return Ok(SyncedReviewLocations {
            local_markdown_path: md_path,
            gdrive_doc_url: pr.gdrive_doc_url.clone(),
            drive_warning: None,
        });
    }

    let doc_title = build_pr_doc_title(pr);
    let target_folder = repo_cfg
        .gdrive_folder_id
        .as_deref()
        .unwrap_or(cfg.gdrive.root_folder_id.as_str());

    match upsert_google_doc(
        cfg,
        &md_path,
        &doc_title,
        target_folder,
        pr.gdrive_doc_id.as_deref(),
    ) {
        Ok((doc_id, doc_url)) => {
            pr.gdrive_doc_id = Some(doc_id);
            pr.gdrive_doc_url = Some(doc_url.clone());
            Ok(SyncedReviewLocations {
                local_markdown_path: md_path,
                gdrive_doc_url: Some(doc_url),
                drive_warning: None,
            })
        }
        Err(err) => Ok(SyncedReviewLocations {
            local_markdown_path: md_path,
            gdrive_doc_url: pr.gdrive_doc_url.clone(),
            drive_warning: Some(err.to_string()),
        }),
    }
}

/// Syncs a repository's consolidated Living Documentation markdown into Google Drive (`00 Living Docs`).
pub fn sync_living_docs_to_drive(
    workspace_root: &Path,
    cfg: &WorkspaceConfig,
    repo_full_name: &str,
    existing_doc_id: Option<&str>,
) -> Result<(String, String), DriveSyncError> {
    let repo_dir = workspace_root
        .join("living-docs")
        .join(repo_fs_slug(repo_full_name));
    let bundle_path = repo_dir.join("_compiled-living-context.md");

    let mut combined = format!(
        "# Living System Context — `{}`\n\n_Auto-maintained on every merged issue/PR by Internal CI `code-reviewer`._\n\n",
        repo_full_name
    );
    for section in [
        "00-system-map.md",
        "01-product-flows.md",
        "02-contracts-schema.md",
        "03-invariants-adrs.md",
    ] {
        let p = repo_dir.join(section);
        if let Ok(content) = fs::read_to_string(&p) {
            combined.push_str("\n---\n\n");
            combined.push_str(&content);
            combined.push('\n');
        }
    }
    fs::create_dir_all(&repo_dir).map_err(|source| DriveSyncError::LocalIo {
        path: repo_dir.clone(),
        source,
    })?;
    fs::write(&bundle_path, combined).map_err(|source| DriveSyncError::LocalIo {
        path: bundle_path.clone(),
        source,
    })?;

    let title = format!("Living Context — {}", repo_full_name);
    upsert_google_doc(
        cfg,
        &bundle_path,
        &title,
        &cfg.gdrive.living_docs_folder_id,
        existing_doc_id,
    )
}

fn build_pr_doc_title(pr: &TrackedPrState) -> String {
    let open_count = pr.open_observations().len();
    let status_prefix = if open_count == 0 {
        "PASS".to_string()
    } else {
        format!("OPEN:{}", open_count)
    };
    let short_repo = pr.repo.rsplit('/').next().unwrap_or(pr.repo.as_str());
    format!(
        "[{}] {} PR #{} — {}",
        status_prefix, short_repo, pr.pr_number, pr.title
    )
}

fn upsert_google_doc(
    cfg: &WorkspaceConfig,
    md_path: &Path,
    title: &str,
    folder_id: &str,
    existing_doc_id: Option<&str>,
) -> Result<(String, String), DriveSyncError> {
    let gdocs = resolve_plugin_cli(&cfg.gdrive.gdocs_bin, "gdocs");
    let gdrive = resolve_plugin_cli(&cfg.gdrive.gdrive_bin, "gdrive");
    let md_str = md_path.to_string_lossy().to_string();

    if let Some(doc_id) = existing_doc_id {
        run_cli(
            &gdocs,
            &["mutate", "import-md", &md_str, "--update", doc_id],
        )?;
        let _ = run_cli(&gdocs, &["mutate", "rename", doc_id, title]);
        let url = format!("https://docs.google.com/document/d/{}/edit", doc_id);
        return Ok((doc_id.to_string(), url));
    }

    let out = run_cli(&gdocs, &["mutate", "import-md", &md_str, "--title", title])?;
    let doc_id = extract_google_doc_id(&out)
        .ok_or_else(|| DriveSyncError::MissingDocId(out.trim().to_string()))?;

    if !folder_id.trim().is_empty() {
        let _ = run_cli(&gdrive, &["mutate", "mv", &doc_id, folder_id]);
    }
    let url = format!("https://docs.google.com/document/d/{}/edit", doc_id);
    Ok((doc_id, url))
}

pub fn resolve_plugin_cli(configured: &str, fallback_name: &str) -> PathBuf {
    if let Some(path_var) = std::env::var_os("PATH") {
        let candidates: &[String] = if cfg!(windows) {
            &[
                format!("{}.cmd", fallback_name),
                format!("{}.exe", fallback_name),
            ]
        } else {
            &[fallback_name.to_string()]
        };
        for dir in std::env::split_paths(&path_var) {
            for cand in candidates {
                let full = dir.join(cand);
                if full.exists() {
                    return full;
                }
            }
        }
    }

    let expanded = expand_tilde(configured);
    if expanded.exists() {
        return expanded;
    }
    if cfg!(windows) {
        let with_cmd = expanded.with_extension("cmd");
        if with_cmd.exists() {
            return with_cmd;
        }
    }
    PathBuf::from(fallback_name)
}

fn run_cli(bin: &Path, args: &[&str]) -> Result<String, DriveSyncError> {
    let out = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| DriveSyncError::CliFailed {
            bin: bin.display().to_string(),
            code: -1,
            stderr: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(DriveSyncError::CliFailed {
            bin: bin.display().to_string(),
            code: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.stderr.is_empty() {
        combined.push('\n');
        combined.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    Ok(combined)
}

/// Extracts a Google Doc ID from `gdocs mutate import-md` stdout (`.../document/d/<ID>` or `ID: <ID>`).
pub fn extract_google_doc_id(output: &str) -> Option<String> {
    if let Some(idx) = output.find("/document/d/") {
        let rest = &output[idx + "/document/d/".len()..];
        let id: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if id.len() >= 15 {
            return Some(id);
        }
    }

    for line in output.lines() {
        for prefix in ["ID:", "Document ID:", "id:"] {
            if let Some(pos) = line.find(prefix) {
                let candidate: String = line[pos + prefix.len()..]
                    .trim()
                    .trim_matches(|c: char| c == '"' || c == '\'' || c == ')' || c == '(')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if candidate.len() >= 15 {
                    return Some(candidate);
                }
            }
        }
    }

    // JSON output fallback if `documentId` appears
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(output.trim()) {
        for key in ["documentId", "id", "fileId"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str())
                && s.len() >= 15
            {
                return Some(s.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_doc_id_from_url_and_id_labels() {
        assert_eq!(
            extract_google_doc_id(
                "Created document: https://docs.google.com/document/d/1ExampleDocIdentifierAlphaBeta0001/edit"
            ),
            Some("1ExampleDocIdentifierAlphaBeta0001".to_string())
        );
        assert_eq!(
            extract_google_doc_id("Imported markdown (ID: 1ExampleDocIdentifierGammaDelta002)"),
            Some("1ExampleDocIdentifierGammaDelta002".to_string())
        );
    }
}
