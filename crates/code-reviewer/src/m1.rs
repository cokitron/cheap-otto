//! Turnkey remote provisioning, deployment, file transfer, token sync, Drive bridge,
//! package installation, and telemetry for the headless M1 (`otto-mac`).

use crate::config::WorkspaceConfig;
use crate::drive::sync_pr_review_artifacts;
use crate::github::gh_binary;
use crate::state::{LedgerStore, TrackedPrState};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const DEFAULT_M1_HOST: &str = "otto-mac";
pub const DEFAULT_M1_REPO_PATH: &str = "~/otto-workspaces/code-reviewer";

pub struct M1SyncOptions<'a> {
    pub host: &'a str,
    pub remote_repo_path: &'a str,
    pub sync_tokens: bool,
    pub sync_drive: bool,
    pub brew_pkgs: &'a str,
    pub cargo_pkgs: &'a str,
    pub npm_pkgs: &'a str,
}

/// Runs a POSIX `sh -s` script on `otto-mac` over SSH stdin so PowerShell / Windows quoting
/// and UTF-8 BOMs never corrupt multi-line commands or special characters.
pub fn run_ssh_script(host: &str, script: &str) -> Result<String, String> {
    let mut child = Command::new("ssh")
        .args(["-o", "BatchMode=yes", host, "sh", "-s"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn `ssh`: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        // Normalize CRLF -> LF so POSIX sh on macOS never sees '\r'.
        let normalized = script.replace("\r\n", "\n");
        stdin
            .write_all(normalized.as_bytes())
            .map_err(|e| format!("failed writing SSH script to stdin: {}", e))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("failed waiting on `ssh`: {}", e))?;

    if !out.status.success() {
        return Err(format!(
            "remote command on `{}` exited with {:?}:\nstdout: {}\nstderr: {}",
            host,
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Atomically writes arbitrary bytes to a remote path on `otto-mac` with exact octal permissions (`mode`, e.g. `"600"` or `"755"`).
pub fn push_bytes_to_m1(
    host: &str,
    remote_path: &str,
    bytes: &[u8],
    mode: &str,
) -> Result<(), String> {
    let b64 = base64_encode(bytes);
    let safe_mode: String = mode.chars().filter(|c| c.is_ascii_digit()).collect();
    let mode_oct = if safe_mode.is_empty() {
        "600"
    } else {
        safe_mode.as_str()
    };

    // Expand leading `~/` to `$HOME/` safely inside single-quoted path handling.
    let target_expr = if let Some(rest) = remote_path.strip_prefix("~/") {
        format!("$HOME/{}", shell_escape_rel(rest))
    } else {
        shell_escape_rel(remote_path)
    };

    let script = format!(
        "set -eu\n\
         TARGET=\"{target}\"\n\
         DIR=$(dirname \"$TARGET\")\n\
         mkdir -p \"$DIR\"\n\
         TMP=\"$TARGET.tmp.$$\"\n\
         printf '%s' '{b64}' | base64 -d > \"$TMP\"\n\
         chmod {mode} \"$TMP\"\n\
         mv -f \"$TMP\" \"$TARGET\"\n",
        target = target_expr,
        b64 = b64,
        mode = mode_oct,
    );
    run_ssh_script(host, &script)?;
    Ok(())
}

/// Full turnkey sync from local workstation to `otto-mac`:
/// 1. Configures git remote `m1` & force-pushes `HEAD -> otto-sync`
/// 2. Compiles, tests (`cargo test` + `clippy`), and installs `~/.cargo/bin/code-reviewer`
/// 3. Runs `scripts/m1-provision.sh` (CLI symlinks, global Claude/Antigravity skills, launchd wrapper, optional packages)
/// 4. Optionally refreshes isolated GitHub account tokens (`--sync-tokens`) and bridges Drive docs (`--sync-drive`).
pub fn sync_everything_to_m1(
    workspace: &Path,
    cfg: &WorkspaceConfig,
    opts: &M1SyncOptions<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let remote_url = if opts.remote_repo_path.starts_with("~/") {
        format!("ssh://{}/{}", opts.host, opts.remote_repo_path)
    } else {
        format!("ssh://{}{}", opts.host, opts.remote_repo_path)
    };
    let remote_dir_expr = if let Some(rest) = opts.remote_repo_path.strip_prefix("~/") {
        format!("$HOME/{}", shell_escape_rel(rest))
    } else {
        shell_escape_rel(opts.remote_repo_path)
    };

    // 1. Ensure named `m1` remote points to `remote_url` and push `HEAD:otto-sync`
    let _ = Command::new("git")
        .args(["-C", &workspace.to_string_lossy(), "remote", "remove", "m1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let add_status = Command::new("git")
        .args([
            "-C",
            &workspace.to_string_lossy(),
            "remote",
            "add",
            "m1",
            &remote_url,
        ])
        .status()?;
    if !add_status.success() {
        return Err("failed to configure git remote `m1`".into());
    }

    println!(
        "[m1-sync] Pushing local HEAD -> {} (otto-sync)...",
        remote_url
    );
    let push_status = Command::new("git")
        .args([
            "-C",
            &workspace.to_string_lossy(),
            "push",
            "m1",
            "HEAD:otto-sync",
            "--force",
        ])
        .status()?;
    if !push_status.success() {
        return Err(format!(
            "`git push m1 HEAD:otto-sync --force` failed for {}",
            remote_url
        )
        .into());
    }

    // 2. Build, test, clippy, install binary, and run `scripts/m1-provision.sh` on M1
    println!("[m1-sync] Building, testing, installing binary, and provisioning `otto-mac`...");
    let build_and_provision = format!(
        "set -eu\n\
         export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n\
         cd \"{repo}\"\n\
         git checkout -- scripts/ crates/code-reviewer/scripts/ 2>/dev/null || true\n\
         git checkout -B main otto-sync\n\
         cargo test --all-targets\n\
         cargo clippy --all-targets -- -D warnings\n\
         if [ -d crates/code-reviewer ]; then\n\
           cargo install --path crates/code-reviewer --locked --force\n\
           CR_ROOT=\"$(pwd)/crates/code-reviewer\"\n\
         else\n\
           cargo install --path . --locked --force\n\
           CR_ROOT=\"$(pwd)\"\n\
         fi\n\
         chmod +x \"$CR_ROOT/scripts/m1-provision.sh\" \"$CR_ROOT/scripts/m1-oauth-login.sh\" 2>/dev/null || true\n\
         BREW_PKGS='{brew}' CARGO_PKGS='{cargo}' NPM_PKGS='{npm}' REPO_ROOT=\"$CR_ROOT\" /bin/sh \"$CR_ROOT/scripts/m1-provision.sh\"\n",
        repo = remote_dir_expr,
        brew = opts.brew_pkgs.replace('\'', ""),
        cargo = opts.cargo_pkgs.replace('\'', ""),
        npm = opts.npm_pkgs.replace('\'', ""),
    );
    let out = run_ssh_script(opts.host, &build_and_provision)?;
    for line in out.lines() {
        println!("  {}", line);
    }

    // 3. Optionally sync GitHub tokens (`primary` & `secondary`) from local `gh` keyring
    if opts.sync_tokens {
        sync_github_tokens_to_m1(cfg, opts.host)?;
    }

    // 4. Optionally bridge any pending M1 PR review artifacts to Google Drive
    if opts.sync_drive {
        sync_m1_reviews_to_drive(workspace, cfg, opts.host, opts.remote_repo_path)?;
    }

    println!("[m1-sync] `otto-mac` is 100% up to date.");
    Ok(())
}

/// Transfers fresh `primary` and `secondary` GitHub tokens from local `gh auth token` to M1 `~/.code-reviewer/tokens/*.token` (`0600`).
pub fn sync_github_tokens_to_m1(
    cfg: &WorkspaceConfig,
    host: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for acct in [&cfg.accounts.primary, &cfg.accounts.secondary] {
        let out = Command::new(gh_binary())
            .args(["auth", "token", "--user", acct])
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            eprintln!(
                "[WARN] Local `gh auth token --user {}` not available on this host; skipping token push for `{}`.",
                acct, acct
            );
            continue;
        }
        let clean_token = String::from_utf8_lossy(&out.stdout)
            .trim()
            .trim_start_matches('\u{feff}')
            .trim()
            .to_string();
        if clean_token.is_empty() {
            continue;
        }
        let remote_tok = format!("~/.code-reviewer/tokens/{}.token", acct);
        push_bytes_to_m1(host, &remote_tok, clean_token.as_bytes(), "600")?;

        let verify_script = format!(
            "set -eu\n\
             GH_TOKEN=$(tr -d '\\r\\n' < \"$HOME/.code-reviewer/tokens/{acct}.token\") /opt/homebrew/bin/gh api user --jq .login\n",
            acct = acct
        );
        let login = run_ssh_script(host, &verify_script)?;
        println!(
            "[m1-tokens] Synced & verified GitHub token for `{}` on M1 (login: `{}`)",
            acct,
            login.trim()
        );
    }
    Ok(())
}

/// Pulls `observations.json` files produced on M1, uploads/updates their Google Docs via local `gdrive`+`gdocs`,
/// and writes `gdrive_doc_id` + `gdrive_doc_url` back to M1 (plus `state.json` when M1's `poll.lock` is idle).
pub fn sync_m1_reviews_to_drive(
    workspace: &Path,
    cfg: &WorkspaceConfig,
    host: &str,
    remote_repo_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let list_script = format!(
        "find \"{}/reviews\" -name 'observations.json' -type f 2>/dev/null || true\n",
        shell_escape_rel(remote_repo_path)
    );
    let paths_raw = run_ssh_script(host, &list_script)?;
    let mut updated_prs: Vec<TrackedPrState> = Vec::new();

    for remote_json_path in paths_raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let cat_script = format!("cat \"{}\"\n", shell_escape_rel(remote_json_path));
        let raw_json = run_ssh_script(host, &cat_script)?;
        let mut pr: TrackedPrState = serde_json::from_str(&raw_json)?;

        let Some(repo_cfg) = cfg
            .repos
            .iter()
            .find(|r| r.full_name.eq_ignore_ascii_case(&pr.repo))
        else {
            continue;
        };

        let res = sync_pr_review_artifacts(workspace, cfg, repo_cfg, &mut pr)?;
        if let Some(url) = &res.gdrive_doc_url {
            println!(
                "[m1-drive-bridge] Synced {}#{} -> {}",
                pr.repo, pr.pr_number, url
            );
            let new_json = serde_json::to_string_pretty(&pr)?;
            push_bytes_to_m1(host, remote_json_path, new_json.as_bytes(), "644")?;
            updated_prs.push(pr);
        } else if let Some(warn) = &res.drive_warning {
            eprintln!(
                "[WARN] Local Drive sync failed for {}#{}: {}",
                pr.repo, pr.pr_number, warn
            );
        }
    }

    if !updated_prs.is_empty() {
        // Only patch `~/.code-reviewer/state.json` on M1 if neither `poll.lock` nor any async worker is active!
        let lock_check = run_ssh_script(
            host,
            "if [ -f \"$HOME/.code-reviewer/poll.lock\" ]; then \
               P=$(head -n 1 \"$HOME/.code-reviewer/poll.lock\" 2>/dev/null | tr -d '[:space:]'); \
               if [ -n \"$P\" ] && kill -0 \"$P\" 2>/dev/null; then echo ACTIVE; exit 0; fi; \
             fi; \
             for wf in \"$HOME/.code-reviewer/workers/\"*.json; do \
               [ -f \"$wf\" ] && echo ACTIVE && exit 0; \
             done; \
             cat \"$HOME/.code-reviewer/state.json\" 2>/dev/null || echo '{}'\n",
        )?;
        if lock_check.trim() == "ACTIVE" {
            println!(
                "[m1-drive-bridge] M1 async worker is actively running; updated per-PR `observations.json` without touching live `state.json`."
            );
        } else if let Ok(mut remote_ledger) = serde_json::from_str::<LedgerStore>(&lock_check) {
            for up in updated_prs {
                let key = LedgerStore::pr_key(&up.repo, up.pr_number);
                if let Some(entry) = remote_ledger.pull_requests.get_mut(&key) {
                    entry.gdrive_doc_id = up.gdrive_doc_id;
                    entry.gdrive_doc_url = up.gdrive_doc_url;
                }
            }
            let serialized = serde_json::to_string_pretty(&remote_ledger)?;
            push_bytes_to_m1(
                host,
                "~/.code-reviewer/state.json",
                serialized.as_bytes(),
                "600",
            )?;
            println!(
                "[m1-drive-bridge] Updated `gdrive_doc_id` links inside M1 `~/.code-reviewer/state.json`."
            );
        }
    }

    Ok(())
}

/// Prints a live single-command dashboard of `otto-mac` hardware, active parallel worker sessions, `launchd`, tracked PRs, and logs.
pub fn print_m1_live_status(host: &str, remote_repo_path: &str) -> Result<(), String> {
    let script = format!(
        "export PATH=\"/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$HOME/.gemini/antigravity/bin:/usr/bin:/bin:/usr/sbin:/sbin\"\n\
         echo '=== 1. M1 Hardware & Power ==='\n\
         pmset -g batt | head -n 2 2>/dev/null || true\n\
         printf 'Thermal State: %s\\n' \"$(sysctl -n kern.thermalstate 2>/dev/null || echo unknown)\"\n\
         echo ''\n\
         echo '=== 2. Launchd Services & Parallel Async Sessions ==='\n\
         UID_NUM=$(id -u)\n\
         if launchctl print \"gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller\" >/dev/null 2>&1; then\n\
           echo 'poller service : LOADED (every 120s)'\n\
         else\n\
           echo 'poller service : NOT LOADED'\n\
         fi\n\
         if launchctl print \"gui/$UID_NUM/com.cheapotto.code-reviewer.code-tender\" >/dev/null 2>&1; then\n\
           echo 'tender service : LOADED (every 120s)'\n\
         else\n\
           echo 'tender service : NOT LOADED'\n\
         fi\n\
         if nc -z 127.0.0.1 7430 2>/dev/null; then\n\
           echo 'web ui server  : LISTENING on 127.0.0.1:7430 (open via `code-reviewer ui`)'\n\
         else\n\
           echo 'web ui server  : OFFLINE'\n\
         fi\n\
         ps aux | grep -E '[c]ode-reviewer.*(poll|review|tend|serve)|[c]laude -p|[a]gy -p' | cut -c1-150 || echo '(no active review/tender process running right now)'\n\
         echo ''\n\
         echo '=== 3. Tracked Pull Requests Ledger ==='\n\
         code-reviewer --workspace \"{repo}\" status || true\n\
         echo ''\n\
         echo '=== 4. Recent Poller Log Tail ==='\n\
         tail -n 18 \"$HOME/.cheap-otto/logs/code-reviewer/sched-pr-poller.log\" 2>/dev/null || true\n",
        repo = shell_escape_rel(remote_repo_path),
    );
    let out = run_ssh_script(host, &script)?;
    print!("{}", out);
    Ok(())
}

/// Triggers an immediate `launchd` poll tick on `otto-mac` (safely no-ops if a poll is already active).
pub fn kickstart_m1_poller(host: &str) -> Result<(), String> {
    let out = run_ssh_script(
        host,
        "UID_NUM=$(id -u)\n\
         launchctl kickstart \"gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller\"\n\
         echo \"Triggered launchd kickstart on gui/$UID_NUM/com.cheapotto.code-reviewer.pr-poller\"\n",
    )?;
    print!("{}", out);
    Ok(())
}

fn shell_escape_rel(input: &str) -> String {
    input.replace('\\', "/").replace('"', "\\\"")
}

/// Minimal RFC-4648 Base64 encoder (avoids extra crate dependency while keeping binary transfers 8-bit safe).
pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_rfc4648_base64_accurately() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"otto-mac:m1"), "b3R0by1tYWM6bTE=");
    }
}
