//! The scripts `src/dashboards.rs` renders run under `/bin/sh` on the Mac. These tests write each
//! one to a temp file and run `sh -n` on it, so a syntax error fails here instead of on the Mac.
//! Unix only: Windows has no `/bin/sh` to check with.
#![forbid(unsafe_code)]

#[cfg(unix)]
#[test]
fn rendered_status_script_passes_sh_syntax_check() -> Result<(), Box<dyn std::error::Error>> {
    use cheap_otto::dashboards::{DashboardSet, render_status_script};

    let script = render_status_script(&DashboardSet::standard());
    let path = std::env::temp_dir().join(format!(
        "cheap-otto-status-script-{}.sh",
        std::process::id()
    ));
    std::fs::write(&path, &script)?;
    let output = std::process::Command::new("sh")
        .arg("-n")
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    let output = output?;
    assert!(
        output.status.success(),
        "sh -n rejected the status script: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
