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
        }
    }

    /// Marks this command specification as tolerating a non-zero exit code.
    #[must_use = "returns the updated command specification"]
    pub fn with_allow_failure(mut self, allow_failure: bool) -> Self {
        self.allow_failure = allow_failure;
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
}
