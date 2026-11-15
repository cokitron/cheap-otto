//! The dashboard scripts run on the Mac under `/bin/sh`. These tests render them and ask the
//! local `sh -n` to parse them, so a syntax error fails CI on Linux and macOS instead of on the
//! appliance. Windows has no `/bin/sh`, so there the tests compile and do not run.
#![forbid(unsafe_code)]

#[cfg(unix)]
use cheap_otto::dashboards::{DashboardSet, render_install_script};

/// Writes `script` to a temp file and returns whether `sh -n` accepts it, with its stderr.
#[cfg(unix)]
fn sh_syntax_check(name: &str, script: &str) -> Result<(bool, String), std::io::Error> {
    let path = std::env::temp_dir().join(format!(
        "cheap-otto-dashboard-{}-{name}.sh",
        std::process::id()
    ));
    std::fs::write(&path, script)?;
    let output = std::process::Command::new("sh")
        .arg("-n")
        .arg(&path)
        .output();
    std::fs::remove_file(&path)?;
    let output = output?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

#[cfg(unix)]
#[test]
fn rendered_install_script_passes_sh_syntax_check() -> Result<(), std::io::Error> {
    let script = render_install_script(&DashboardSet::standard());
    let (ok, stderr) = sh_syntax_check("install", &script)?;
    assert!(ok, "sh -n rejected the install script: {stderr}");
    Ok(())
}
