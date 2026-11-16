//! Structured, shell-free OS command specifications for the Functional Core.
//!
//! Representing external commands (`ssh`, `git`, `sudo`, `pmset`) as a discrete
//! `program` and `args` vector ensures the imperative shell (`src/main.rs`) invokes
//! `std::process::Command::args` directly without `sh -c` or `cmd /C` string concatenation.

use serde::{Deserialize, Serialize};

/// A deterministic, shell-free child process specification produced by pure planning functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Human-readable summary of what this step performs.
    pub description: String,
    /// Executable binary name (e.g., `"ssh"`, `"git"`, `"sudo"`).
    pub program: String,
    /// Discrete argument list passed directly via `Command::args` without a local shell.
    pub args: Vec<String>,
    /// Whether a non-zero exit code is tolerated (e.g., idempotent `git remote add`).
    #[serde(default)]
    pub allow_failure: bool,
    /// Optional LF-normalised payload piped into the child process's standard input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
}

/// Normalises CRLF and bare CR line endings to LF so remote POSIX `sh -s` scripts never see `\r`.
#[must_use = "returns the LF-normalised script text"]
pub fn normalize_script_lf(script: &str) -> String {
    script.replace("\r\n", "\n").replace('\r', "\n")
}

impl CommandSpec {
    /// Creates a mandatory command specification whose non-zero exit code aborts execution.
    #[must_use = "command specification should be stored in a plan or executed"]
    pub fn new<I, S>(description: &str, program: &str, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            description: description.to_string(),
            program: program.to_string(),
            args: args.into_iter().map(Into::into).collect(),
            allow_failure: false,
            stdin: None,
        }
    }

    /// Marks this command specification as tolerating a non-zero exit code.
    #[must_use = "returns the updated command specification"]
    pub fn with_allow_failure(mut self, allow_failure: bool) -> Self {
        self.allow_failure = allow_failure;
        self
    }

    /// Attaches an LF-normalised standard-input payload to pipe into the child process.
    #[must_use = "returns the updated command specification"]
    pub fn with_stdin(mut self, stdin: impl Into<String>) -> Self {
        let raw = stdin.into();
        self.stdin = Some(normalize_script_lf(&raw));
        self
    }

    /// Formats a human-readable shell preview of the command for dry-run inspection.
    #[must_use = "returns the formatted command preview string"]
    pub fn display_command(&self) -> String {
        let mut rendered = String::from(&self.program);
        for arg in &self.args {
            rendered.push(' ');
            if arg.is_empty()
                || arg
                    .chars()
                    .any(|c| c.is_whitespace() || matches!(c, '"' | '\'' | '$' | ';' | '&' | '|'))
            {
                let escaped = arg.replace('\\', "\\\\").replace('"', "\\\"");
                rendered.push('"');
                rendered.push_str(&escaped);
                rendered.push('"');
            } else {
                rendered.push_str(arg);
            }
        }
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clean_display_command_and_quotes_whitespace_arguments() {
        let spec = CommandSpec::new(
            "Run remote test suite",
            "ssh",
            ["-p", "22", "admin@192.168.1.42", "cargo test --all-targets"],
        );

        assert!(!spec.allow_failure);
        assert_eq!(
            spec.display_command(),
            "ssh -p 22 admin@192.168.1.42 \"cargo test --all-targets\""
        );
    }

    #[test]
    fn supports_allow_failure_builder_flag() {
        let spec = CommandSpec::new(
            "Add git remote if missing",
            "git",
            [
                "remote",
                "add",
                "otto-mac",
                "ssh://admin@otto-mac:22/~/repo",
            ],
        )
        .with_allow_failure(true);

        assert!(spec.allow_failure);
    }

    #[test]
    fn with_stdin_normalizes_crlf_and_bare_cr_and_omits_none_in_json()
    -> Result<(), serde_json::Error> {
        let plain = CommandSpec::new("Probe", "ssh", ["otto-mac", "true"]);
        assert_eq!(plain.stdin, None);
        let plain_json = serde_json::to_string(&plain)?;
        assert!(!plain_json.contains("\"stdin\""), "{plain_json}");
        let round_trip: CommandSpec = serde_json::from_str(&plain_json)?;
        assert_eq!(round_trip, plain);

        let scripted = CommandSpec::new("Run script", "ssh", ["otto-mac", "/bin/sh -s"])
            .with_stdin("set -eu\r\necho ok\rexit 0\r\n");
        assert_eq!(
            scripted.stdin.as_deref(),
            Some("set -eu\necho ok\nexit 0\n")
        );
        // `display_command` renders only discrete arguments and never leaks the stdin body.
        assert_eq!(scripted.display_command(), "ssh otto-mac \"/bin/sh -s\"");
        Ok(())
    }
}
