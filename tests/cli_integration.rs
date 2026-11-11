#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cheap_otto_bin() -> &'static str {
    env!("CARGO_BIN_EXE_cheap-otto")
}

fn create_isolated_temp_dir(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = env::temp_dir().join(format!("cheap-otto-it-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn run_cli_in(
    cwd: &Path,
    args: &[&str],
) -> Result<(bool, String, String), Box<dyn std::error::Error>> {
    let output = Command::new(cheap_otto_bin())
        .current_dir(cwd)
        .args(args)
        .output()?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

#[test]
fn init_persists_backup_and_restore_reads_exact_original_pmset()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = create_isolated_temp_dir("init-restore")?;
    let state_dir = temp.join(".cheap-otto");
    let state_str = state_dir.to_string_lossy().into_owned();

    let (ok, stdout, stderr) = run_cli_in(
        &temp,
        &[
            "init",
            "--ram-gb",
            "16",
            "--battery-cap",
            "78",
            "--offline",
            "--out-dir",
            &state_str,
        ],
    )?;
    assert!(ok, "init failed: {stderr}");
    assert!(stdout.contains("78%"));
    assert!(state_dir.join("state-backup.json").exists());

    let (doc_ok, doc_out, doc_err) =
        run_cli_in(&temp, &["doctor", "--offline", "--state-dir", &state_str])?;
    assert!(doc_ok, "doctor failed: {doc_err}");
    assert!(doc_out.contains("Charge ceiling capped at 78%"));

    let (res_ok, res_out, res_err) = run_cli_in(&temp, &["restore", "--state-dir", &state_str])?;
    assert!(res_ok, "restore failed: {res_err}");
    assert!(
        res_out
            .contains("sudo pmset -c sleep 1 displaysleep 10 autorestart 0 womp 0 tcpkeepalive 1")
    );

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn auto_detects_git_project_and_tracks_multi_project_registry()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = create_isolated_temp_dir("auto-project")?;
    let repo_dir = temp.join("my-awesome-cli");
    fs::create_dir_all(&repo_dir)?;

    // Initialize a real local Git repository with a feature branch to verify auto-discovery
    let git_init = Command::new("git")
        .current_dir(&repo_dir)
        .args(["init", "-b", "feat/auto-discover"])
        .status();

    let state_dir = repo_dir.join(".cheap-otto");
    let state_str = state_dir.to_string_lossy().into_owned();

    // Pair MacBook into the repo's state directory
    let (pair_ok, _, pair_err) = run_cli_in(
        &repo_dir,
        &[
            "pair",
            "--host",
            "192.168.1.42",
            "--user",
            "admin",
            "--alias",
            "otto-mac",
            "--out-dir",
            &state_str,
        ],
    )?;
    assert!(pair_ok, "pair failed: {pair_err}");

    // Run `sync` WITHOUT `--project` -> must auto-detect `my-awesome-cli`
    let (sync_ok, sync_out, sync_err) =
        run_cli_in(&repo_dir, &["sync", "--state-dir", &state_str])?;
    assert!(sync_ok, "sync failed: {sync_err}");
    assert!(sync_out.contains("my-awesome-cli"));
    if git_init.is_ok_and(|s| s.success()) {
        assert!(sync_out.contains("feat/auto-discover"));
    }

    // Run `delegate` WITHOUT `--project` -> must auto-detect `my-awesome-cli` and record task
    let (del_ok, del_out, del_err) = run_cli_in(
        &repo_dir,
        &[
            "delegate",
            "--id",
            "add-smoke-tests",
            "--agent",
            "claude",
            "--task",
            "Add CLI smoke tests",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(del_ok, "delegate failed: {del_err}");
    assert!(del_out.contains("otto/add-smoke-tests"));
    assert!(del_out.contains("~/otto-workspaces/my-awesome-cli/.otto-worktrees/add-smoke-tests"));

    // Run `projects` -> must display `my-awesome-cli` and `add-smoke-tests`
    let (proj_ok, proj_out, proj_err) =
        run_cli_in(&repo_dir, &["projects", "--state-dir", &state_str])?;
    assert!(proj_ok, "projects failed: {proj_err}");
    assert!(proj_out.contains("my-awesome-cli"));
    assert!(proj_out.contains("add-smoke-tests"));

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn terminal_shell_attach_logs_and_sync_guard_workflows_work_end_to_end()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = create_isolated_temp_dir("terminal-visibility")?;
    let repo_dir = temp.join("m1-terminal-app");
    fs::create_dir_all(&repo_dir)?;

    let _ = Command::new("git")
        .current_dir(&repo_dir)
        .args(["init", "-b", "main"])
        .status();

    let state_dir = repo_dir.join(".cheap-otto");
    let state_str = state_dir.to_string_lossy().into_owned();

    let (pair_ok, _, pair_err) = run_cli_in(
        &repo_dir,
        &[
            "pair",
            "--host",
            "100.101.102.103",
            "--user",
            "admin",
            "--alias",
            "otto-mac",
            "--out-dir",
            &state_str,
        ],
    )?;
    assert!(pair_ok, "pair failed: {pair_err}");

    // 1. Default `shell` opens persistent `manual` tmux session inside project dir
    let (sh_ok, sh_out, sh_err) = run_cli_in(&repo_dir, &["shell", "--state-dir", &state_str])?;
    assert!(sh_ok, "shell failed: {sh_err}");
    assert!(sh_out.contains("~/otto-workspaces/m1-terminal-app"));
    assert!(sh_out.contains("Persistent tmux (`manual`"));

    // 2. `shell --session otto-bad` must fail to protect automated namespace
    let (bad_sh_ok, _, _) = run_cli_in(
        &repo_dir,
        &[
            "shell",
            "--session",
            "otto-reserved",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(!bad_sh_ok, "expected reserved otto- session prefix to fail");

    // 3. `attach --id fix-auth --read-only` & `logs --id fix-auth --follow`
    let (att_ok, att_out, att_err) = run_cli_in(
        &repo_dir,
        &[
            "attach",
            "--id",
            "fix-auth",
            "--read-only",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(att_ok, "attach failed: {att_err}");
    assert!(att_out.contains("otto-fix-auth"));
    assert!(att_out.contains("Read-Only"));

    let (log_ok, log_out, log_err) = run_cli_in(
        &repo_dir,
        &[
            "logs",
            "--id",
            "fix-auth",
            "--lines",
            "50",
            "--follow",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(log_ok, "logs failed: {log_err}");
    assert!(log_out.contains("~/.cheap-otto/logs/m1-terminal-app/fix-auth.log"));
    assert!(log_out.contains("following live"));

    // 4. `sync` enables manual guard by default; `sync --force` disables it
    let (sync_ok, sync_out, _) = run_cli_in(&repo_dir, &["sync", "--state-dir", &state_str])?;
    assert!(sync_ok);
    assert!(sync_out.contains("protects uncommitted manual edits"));

    let (fsync_ok, fsync_out, _) =
        run_cli_in(&repo_dir, &["sync", "--force", "--state-dir", &state_str])?;
    assert!(fsync_ok);
    assert!(fsync_out.contains("disabled (--force"));

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

#[test]
fn schedule_lifecycle_persists_launchd_jobs_and_supports_runs_and_restore()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = create_isolated_temp_dir("schedule-lifecycle")?;
    let repo_dir = temp.join("ci-service");
    fs::create_dir_all(&repo_dir)?;

    let _ = Command::new("git")
        .current_dir(&repo_dir)
        .args(["init", "-b", "main"])
        .status();

    let state_dir = repo_dir.join(".cheap-otto");
    let state_str = state_dir.to_string_lossy().into_owned();

    // Initialize backup + pair
    let (init_ok, _, init_err) =
        run_cli_in(&repo_dir, &["init", "--offline", "--out-dir", &state_str])?;
    assert!(init_ok, "init failed: {init_err}");

    // 1. Add recurring on-change CI job (`--every 15m`)
    let (ci_ok, ci_out, ci_err) = run_cli_in(
        &repo_dir,
        &[
            "schedule",
            "add",
            "--id",
            "main-ci",
            "--every",
            "15m",
            "--ci",
            "cargo clippy --all-targets -- -D warnings && cargo test",
            "--on-change",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(ci_ok, "schedule add ci failed: {ci_err}");
    assert!(ci_out.contains("com.cheapotto.ci-service.main-ci"));
    assert!(state_dir.join("schedules.json").exists());

    // 2. Add nightly autonomous agent job (`--daily 02:30 --draft-pr`)
    let (ag_ok, ag_out, ag_err) = run_cli_in(
        &repo_dir,
        &[
            "schedule",
            "add",
            "--id",
            "nightly-audit",
            "--daily",
            "02:30",
            "--agent",
            "claude",
            "--task",
            "Audit dependencies and add missing tests",
            "--draft-pr",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(ag_ok, "schedule add agent failed: {ag_err}");
    assert!(ag_out.contains("daily at 02:30"));

    // 3. List configured schedules
    let (ls_ok, ls_out, ls_err) =
        run_cli_in(&repo_dir, &["schedule", "list", "--state-dir", &state_str])?;
    assert!(ls_ok, "schedule list failed: {ls_err}");
    assert!(ls_out.contains("ci-service/main-ci"));
    assert!(ls_out.contains("ci-service/nightly-audit"));

    // 4. Kickstart (`run-now`) & query `runs` dry-run
    let (rn_ok, rn_out, rn_err) = run_cli_in(
        &repo_dir,
        &[
            "schedule",
            "run-now",
            "--id",
            "main-ci",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(rn_ok, "schedule run-now failed: {rn_err}");
    assert!(rn_out.contains("launchctl kickstart -k"));

    let (runs_ok, runs_out, runs_err) =
        run_cli_in(&repo_dir, &["runs", "--state-dir", &state_str])?;
    assert!(runs_ok, "runs failed: {runs_err}");
    assert!(runs_out.contains("history.jsonl"));

    // 5. Remove `main-ci` and verify `nightly-audit` remains
    let (rm_ok, _, rm_err) = run_cli_in(
        &repo_dir,
        &[
            "schedule",
            "remove",
            "--id",
            "main-ci",
            "--state-dir",
            &state_str,
        ],
    )?;
    assert!(rm_ok, "schedule remove failed: {rm_err}");

    let (_, ls_after, _) = run_cli_in(&repo_dir, &["schedule", "list", "--state-dir", &state_str])?;
    assert!(!ls_after.contains("ci-service/main-ci"));
    assert!(ls_after.contains("ci-service/nightly-audit"));

    // 6. `restore --unload-schedules` prints LaunchAgent bootout loop
    let (res_ok, res_out, res_err) = run_cli_in(
        &repo_dir,
        &["restore", "--unload-schedules", "--state-dir", &state_str],
    )?;
    assert!(res_ok, "restore --unload-schedules failed: {res_err}");
    assert!(res_out.contains("com.cheapotto.*.plist"));

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}
