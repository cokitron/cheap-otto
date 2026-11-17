//! Pure `.env` scope validation, strict dotenv/schema parsing, and blocked-variable guards
//! (`specs/m1-env-secrets`). Performs zero filesystem, process, network, clock, or entropy I/O.

use crate::bridge::validate_safe_identifier;
use crate::command::normalize_script_lf;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Default `.env` profile name when `--profile` is omitted (`"dev"`).
pub const DEFAULT_ENV_PROFILE: &str = "dev";
/// Maximum byte length of a validated `.env` variable name.
pub const MAX_ENV_KEY_LEN: usize = 128;
/// Maximum byte length of a relative monorepo `subpath` (`"apps/web"`).
pub const MAX_ENV_SUBPATH_LEN: usize = 128;
/// Maximum directory segment depth in a monorepo `subpath`.
pub const MAX_ENV_SUBPATH_SEGMENTS: usize = 8;

/// Environment variable names blocked from `.env` files and runtime injection because they can
/// hijack dynamic linkers, shell startup, PATH resolution, SSH agents, Git hooks, or compilers.
#[rustfmt::skip]
pub const BLOCKED_ENV_KEYS: &[&str] = &[
    "PATH", "CDPATH", "IFS", "HOME", "USER", "LOGNAME", "SHELL", "SSH_AUTH_SOCK",
    "BASH_ENV", "ENV", "ZDOTDIR", "LD_PRELOAD", "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "DYLD_FRAMEWORK_PATH", "DYLD_FALLBACK_LIBRARY_PATH",
    "GIT_SSH_COMMAND", "GIT_EXEC_PATH", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS",
    "NODE_OPTIONS", "PYTHONSTARTUP", "PYTHONPATH", "RUBYOPT", "PERL5OPT", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER",
];

/// Domain errors returned when validating `.env` scopes, variable names, dotenv syntax, or
/// `.env.example` agent dummy literals.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnvError {
    /// Project slug failed `validate_safe_identifier`.
    #[error("invalid project name '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidProject(String),
    /// Profile slug failed `validate_safe_identifier`.
    #[error("invalid env profile '{0}': must contain only ASCII alphanumeric, '-', '_', or '.'")]
    InvalidProfile(String),
    /// Relative monorepo subpath failed path-safety rules.
    #[error("invalid env subpath '{0}': {1}")]
    InvalidSubpath(String, String),
    /// Deserialized `EnvScope` differed from its canonical validated form.
    #[error("invalid serialized env scope: field '{0}' is not canonical")]
    NonCanonicalScope(String),
    /// Variable name is empty, too long, or outside `^[A-Za-z_][A-Za-z0-9_]*$`.
    #[error("invalid env variable name '{0}': expected [A-Za-z_][A-Za-z0-9_]* (<= 128 chars)")]
    InvalidKey(String),
    /// Variable name appears in [`BLOCKED_ENV_KEYS`].
    #[error("env variable '{0}' is blocked for execution and linker safety")]
    BlockedKey(String),
    /// Input or rendered value contained an interior NUL (`\0`) byte.
    #[error("dotenv input must not contain NUL ('\\0') bytes")]
    NulByte,
    /// Quoted value reached end of input without a matching closing quote (`(line, quote)`).
    #[error("line {0}: unclosed {1} quote in dotenv value")]
    UnclosedQuote(usize, char),
    /// Malformed assignment or trailing tokens after a quoted value (`(line, message)`).
    #[error("line {0}: {1}")]
    Syntax(usize, String),
    /// `# @otto:agent-dummy=<literal>` carried credentials or real-looking tokens (`(key, reason)`).
    #[error("unsafe agent dummy value for '{0}': {1}")]
    UnsafeAgentDummy(String, String),
}

/// Validated `{ project, profile, subpath }` identifier keying a secret set across
/// `~/otto-workspaces/<project>` and `~/repos/<gh-account>/<project>`.
///
/// Fields are private so every scope passes through [`EnvScope::new`]. Identifiers are
/// case-folded to ASCII lowercase so store paths and Keychain services cannot collide on
/// case-insensitive filesystems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EnvScopeWire")]
pub struct EnvScope {
    project: String,
    profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subpath: Option<String>,
}

#[derive(Deserialize)]
struct EnvScopeWire {
    project: String,
    profile: String,
    #[serde(default)]
    subpath: Option<String>,
}

impl TryFrom<EnvScopeWire> for EnvScope {
    type Error = EnvError;

    fn try_from(wire: EnvScopeWire) -> Result<Self, Self::Error> {
        let rebuilt = Self::new(&wire.project, &wire.profile, wire.subpath.as_deref())?;
        for (same, field) in [
            (rebuilt.project == wire.project, "project"),
            (rebuilt.profile == wire.profile, "profile"),
            (rebuilt.subpath == wire.subpath, "subpath"),
        ] {
            if !same {
                return Err(EnvError::NonCanonicalScope(field.to_string()));
            }
        }
        Ok(rebuilt)
    }
}

impl EnvScope {
    /// Validates and constructs an [`EnvScope`], case-folding project, profile and subpath to
    /// ASCII lowercase so distinct spellings cannot map to one path on case-insensitive filesystems.
    ///
    /// # Errors
    /// Returns [`EnvError::InvalidProject`], [`EnvError::InvalidProfile`], or
    /// [`EnvError::InvalidSubpath`] when any component fails path and shell safety checks.
    pub fn new(project: &str, profile: &str, subpath: Option<&str>) -> Result<Self, EnvError> {
        let clean_project = project.trim();
        if !validate_safe_identifier(clean_project) {
            return Err(EnvError::InvalidProject(project.to_string()));
        }
        let clean_profile = profile.trim();
        if !validate_safe_identifier(clean_profile) {
            return Err(EnvError::InvalidProfile(profile.to_string()));
        }
        Ok(Self {
            project: clean_project.to_ascii_lowercase(),
            profile: clean_profile.to_ascii_lowercase(),
            subpath: subpath
                .map(validate_subpath)
                .transpose()?
                .map(|s| s.to_ascii_lowercase()),
        })
    }

    /// Canonical lowercase repository or project slug (`"cheap-otto"`).
    #[must_use = "returns the validated project slug"]
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Canonical lowercase environment profile slug (`"dev"`, `"test"`).
    #[must_use = "returns the validated profile slug"]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Canonical lowercase monorepo relative subpath (`Some("apps/web")`), if any.
    #[must_use = "returns the validated subpath"]
    pub fn subpath(&self) -> Option<&str> {
        self.subpath.as_deref()
    }

    /// Relative path under `~/.cheap-otto/` for the `0600` secret file (`env/<p>/<prof>/tree/[<s>/].env`).
    #[must_use = "returns the collision-free relative store path"]
    pub fn relative_store_path(&self) -> String {
        match &self.subpath {
            None => format!("env/{}/{}/tree/.env", self.project, self.profile),
            Some(sub) => format!("env/{}/{}/tree/{sub}/.env", self.project, self.profile),
        }
    }

    /// Bijective macOS Keychain service identifier (`com.cheapotto.env/<p>/<prof>[/<s>]`).
    #[must_use = "returns the bijective macOS Keychain service name"]
    pub fn keychain_service(&self) -> String {
        match &self.subpath {
            None => format!("com.cheapotto.env/{}/{}", self.project, self.profile),
            Some(sub) => format!("com.cheapotto.env/{}/{}/{sub}", self.project, self.profile),
        }
    }

    /// Map key inside `env/<project>/<profile>/keychain.json` (`""` for root, `"apps/web"` for subpath).
    #[must_use = "returns the canonical keychain.json map key"]
    pub fn keychain_scope_key(&self) -> &str {
        self.subpath.as_deref().unwrap_or("")
    }
}

fn validate_subpath(raw: &str) -> Result<String, EnvError> {
    let trimmed = raw.trim();
    let invalid = |msg: &str| Err(EnvError::InvalidSubpath(raw.to_string(), msg.to_string()));
    if trimmed.is_empty() || trimmed.len() > MAX_ENV_SUBPATH_LEN {
        return invalid("subpath must be 1..=128 bytes; omit --subpath for repository root");
    }
    if trimmed.contains('\\') || trimmed.starts_with('/') || trimmed.ends_with('/') {
        return invalid("subpath must use relative '/' segments without leading/trailing '/'");
    }
    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.len() > MAX_ENV_SUBPATH_SEGMENTS {
        return invalid("subpath exceeds 8 segments");
    }
    for seg in segments {
        let valid = seg
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
            && !seg.contains("..")
            && seg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if !valid {
            return invalid("each segment must start with [A-Za-z0-9] and use [A-Za-z0-9._-]");
        }
    }
    Ok(trimmed.to_string())
}

/// Validates an environment variable name against POSIX syntax and [`BLOCKED_ENV_KEYS`].
///
/// # Errors
/// Returns [`EnvError::InvalidKey`] for malformed names and [`EnvError::BlockedKey`] for blocked
/// execution-hijack variables.
pub fn validate_env_key(raw: &str) -> Result<&str, EnvError> {
    let bytes = raw.as_bytes();
    let Some(&first) = bytes.first() else {
        return Err(EnvError::InvalidKey(raw.to_string()));
    };
    if bytes.len() > MAX_ENV_KEY_LEN
        || !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        return Err(EnvError::InvalidKey(raw.to_string()));
    }
    if BLOCKED_ENV_KEYS.iter().any(|b| raw.eq_ignore_ascii_case(b)) {
        return Err(EnvError::BlockedKey(raw.to_string()));
    }
    Ok(raw)
}

/// Parsed `.env` key-value map (`last-wins`) along with duplicate-key warnings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDotenv {
    /// Validated `KEY -> VALUE` entries sorted lexicographically by key.
    pub entries: BTreeMap<String, String>,
    /// Keys assigned more than once in the input, in encounter order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicate_keys: Vec<String>,
}

/// Schema parsed from a repository's `.env.example` file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSchema {
    /// Declared variable names in first-seen order.
    pub keys: Vec<String>,
    /// Optional `# @otto:agent-dummy=<literal>` non-secret test defaults keyed by variable name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub agent_dummies: BTreeMap<String, String>,
}

/// Parses `.env` text into validated key-value pairs without shell command expansion.
///
/// # Errors
/// Returns [`EnvError::NulByte`], [`EnvError::InvalidKey`], [`EnvError::BlockedKey`],
/// [`EnvError::UnclosedQuote`], or [`EnvError::Syntax`] when the input is malformed.
pub fn parse_dotenv(raw: &str) -> Result<ParsedDotenv, EnvError> {
    let p = parse_dotenv_inner(raw, false)?;
    Ok(ParsedDotenv {
        entries: p.entries,
        duplicate_keys: p.duplicate_keys,
    })
}

/// Parses `.env.example` into declared variable names and validated `# @otto:agent-dummy=<literal>`
/// non-secret test defaults.
///
/// # Errors
/// Returns [`EnvError`] on invalid keys, malformed dotenv syntax, or credential-bearing agent
/// dummy literals (`://user:pass@` or token prefixes).
pub fn parse_env_example(raw: &str) -> Result<EnvSchema, EnvError> {
    let p = parse_dotenv_inner(raw, true)?;
    Ok(EnvSchema {
        keys: p.ordered_keys,
        agent_dummies: p.agent_dummies,
    })
}

/// Renders validated `.env` entries into canonical double-quoted `KEY="VALUE"\n` format.
///
/// # Errors
/// Returns [`EnvError::InvalidKey`], [`EnvError::BlockedKey`], or [`EnvError::NulByte`] if any
/// entry is invalid.
pub fn render_dotenv(entries: &BTreeMap<String, String>) -> Result<String, EnvError> {
    let mut out = String::new();
    for (key, value) in entries {
        let _ = validate_env_key(key)?;
        if value.contains('\0') {
            return Err(EnvError::NulByte);
        }
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('$', "\\$");
        out.push_str(&format!("{key}=\"{escaped}\"\n"));
    }
    Ok(out)
}

#[derive(Default)]
struct InnerParseOutput {
    entries: BTreeMap<String, String>,
    ordered_keys: Vec<String>,
    duplicate_keys: Vec<String>,
    duplicate_seen: BTreeSet<String>,
    agent_dummies: BTreeMap<String, String>,
}

fn parse_dotenv_inner(raw: &str, parse_dummies: bool) -> Result<InnerParseOutput, EnvError> {
    if raw.contains('\0') {
        return Err(EnvError::NulByte);
    }
    let normalized = normalize_script_lf(raw.strip_prefix('\u{feff}').unwrap_or(raw));
    let mut rest = normalized.as_str();
    let mut line_no: usize = 1;
    let mut pending_dummy: Option<(usize, String)> = None;
    let mut out = InnerParseOutput::default();

    while !rest.is_empty() {
        rest = rest.trim_start_matches([' ', '\t']);
        if let Some(after_nl) = rest.strip_prefix('\n') {
            pending_dummy = None;
            line_no = line_no.saturating_add(1);
            rest = after_nl;
            continue;
        }
        if let Some(after_hash) = rest.strip_prefix('#') {
            let (comment_line, after_line) = split_at_newline(after_hash);
            if parse_dummies {
                // Any non-marker comment clears the marker so it only binds to the key directly below.
                pending_dummy = comment_line
                    .trim()
                    .strip_prefix("@otto:agent-dummy=")
                    .map(|lit| (line_no, lit.trim().to_string()));
            }
            rest = advance_newline(after_line, &mut line_no);
            continue;
        }
        if rest.is_empty() {
            break;
        }

        let after_export = match rest.strip_prefix("export") {
            Some(tail) if tail.starts_with([' ', '\t']) => tail.trim_start_matches([' ', '\t']),
            _ => rest,
        };
        let (head_line, _) = split_at_newline(after_export);
        let Some(eq_idx) = head_line.find('=') else {
            return Err(EnvError::Syntax(
                line_no,
                "expected KEY=VALUE assignment".to_string(),
            ));
        };
        let key = validate_env_key(head_line[..eq_idx].trim_matches([' ', '\t']))?.to_string();
        let raw_after_eq = &after_export[eq_idx + 1..];
        let after_eq = raw_after_eq.trim_start_matches([' ', '\t']);
        // The whitespace after `=` is the delimiter that makes a following `#` an inline comment.
        let leading_ws = after_eq.len() < raw_after_eq.len();
        let (value, next_rest, next_line) = parse_dotenv_value(after_eq, leading_ws, line_no)?;

        if let Some((_, dummy_lit)) = pending_dummy.take() {
            validate_agent_dummy_literal(&key, &dummy_lit)?;
            out.agent_dummies.insert(key.clone(), dummy_lit);
        }
        if out.entries.insert(key.clone(), value).is_none() {
            out.ordered_keys.push(key);
        } else if out.duplicate_seen.insert(key.clone()) {
            out.duplicate_keys.push(key);
        }
        rest = next_rest;
        line_no = next_line;
    }

    if let Some((line, _)) = pending_dummy {
        return Err(EnvError::Syntax(
            line,
            "`@otto:agent-dummy=` must stand immediately above a KEY=VALUE line".to_string(),
        ));
    }
    Ok(out)
}

fn parse_dotenv_value(
    input: &str,
    leading_ws: bool,
    start_line: usize,
) -> Result<(String, &str, usize), EnvError> {
    if let Some(after_sq) = input.strip_prefix('\'') {
        let mut line = start_line;
        for (idx, ch) in after_sq.char_indices() {
            if ch == '\n' {
                line = line.saturating_add(1);
            } else if ch == '\'' {
                let (next_rest, next_line) = consume_line_tail(&after_sq[idx + 1..], line, '\'')?;
                return Ok((after_sq[..idx].to_string(), next_rest, next_line));
            }
        }
        return Err(EnvError::UnclosedQuote(start_line, '\''));
    }

    if let Some(after_dq) = input.strip_prefix('"') {
        let mut value = String::new();
        let mut chars = after_dq.char_indices();
        let mut line = start_line;
        while let Some((idx, ch)) = chars.next() {
            match ch {
                '\n' => {
                    line = line.saturating_add(1);
                    value.push('\n');
                }
                '"' => {
                    let (next_rest, next_line) =
                        consume_line_tail(&after_dq[idx + 1..], line, '"')?;
                    return Ok((value, next_rest, next_line));
                }
                '\\' => {
                    let (_, esc) = chars
                        .next()
                        .ok_or(EnvError::UnclosedQuote(start_line, '"'))?;
                    match esc {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '"' | '\\' | '$' => value.push(esc),
                        '0' => return Err(EnvError::NulByte),
                        '\n' => {
                            line = line.saturating_add(1);
                            value.push('\n');
                        }
                        other => value.extend(['\\', other]),
                    }
                }
                other => value.push(other),
            }
        }
        return Err(EnvError::UnclosedQuote(start_line, '"'));
    }

    let (raw_line, after_line) = split_at_newline(input);
    let stripped = strip_unquoted_comment(raw_line, leading_ws).trim_end_matches([' ', '\t']);
    let mut line = start_line;
    let next_rest = advance_newline(after_line, &mut line);
    Ok((stripped.to_string(), next_rest, line))
}

fn consume_line_tail(tail: &str, mut line: usize, quote: char) -> Result<(&str, usize), EnvError> {
    let (same_line, after_line) = split_at_newline(tail);
    let trimmed = same_line.trim_start_matches([' ', '\t']);
    if !trimmed.is_empty() && !trimmed.starts_with('#') {
        return Err(EnvError::Syntax(
            line,
            format!("unexpected trailing characters after closing {quote} quote"),
        ));
    }
    Ok((advance_newline(after_line, &mut line), line))
}

fn advance_newline<'a>(s: &'a str, line: &mut usize) -> &'a str {
    s.strip_prefix('\n').map_or(s, |after_nl| {
        *line = line.saturating_add(1);
        after_nl
    })
}

fn split_at_newline(s: &str) -> (&str, &str) {
    s.find('\n').map_or((s, ""), |idx| (&s[..idx], &s[idx..]))
}

fn strip_unquoted_comment(line: &str, leading_ws: bool) -> &str {
    let mut prev_ws = leading_ws;
    for (idx, ch) in line.char_indices() {
        if ch == '#' && prev_ws {
            return &line[..idx];
        }
        prev_ws = matches!(ch, ' ' | '\t');
    }
    line
}

fn validate_agent_dummy_literal(key: &str, literal: &str) -> Result<(), EnvError> {
    let reject = |msg: &str| Err(EnvError::UnsafeAgentDummy(key.to_string(), msg.to_string()));
    if literal.contains(['\0', '\r', '\n']) {
        return reject("dummy value must be a single line without NUL bytes");
    }
    if let Some((_, after_scheme)) = literal.split_once("://") {
        let end = after_scheme
            .find(['/', '?', '#'])
            .unwrap_or(after_scheme.len());
        if after_scheme[..end].contains('@') {
            return reject("URL userinfo ('user:pass@host') is forbidden in agent dummy values");
        }
    }
    let lower = literal.to_ascii_lowercase();
    for pfx in [
        "sk-", "rk_live_", "sk_live_", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "xox", "xoxb-",
        "xoxp-", "xoxa-", "xapp-",
    ] {
        if lower.contains(pfx) {
            return reject(&format!("token prefix '{pfx}' is forbidden in agent dummy"));
        }
    }
    if literal.contains("eyJ") {
        return reject("JWT prefix 'eyJ' is forbidden in agent dummy values");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_paths_are_bijective_across_root_and_nested_monorepo_subpaths() -> Result<(), EnvError>
    {
        let root = EnvScope::new("cheap-otto", DEFAULT_ENV_PROFILE, None)?;
        let apps = EnvScope::new("cheap-otto", "dev", Some("apps"))?;
        let web = EnvScope::new("cheap-otto", "dev", Some("apps/web"))?;
        let dotted = EnvScope::new("cheap-otto", "dev", Some("apps.web"))?;

        assert_eq!(root.relative_store_path(), "env/cheap-otto/dev/tree/.env");
        assert_eq!(
            apps.relative_store_path(),
            "env/cheap-otto/dev/tree/apps/.env"
        );
        assert_eq!(
            web.relative_store_path(),
            "env/cheap-otto/dev/tree/apps/web/.env"
        );
        assert_ne!(web.relative_store_path(), dotted.relative_store_path());
        assert_ne!(web.keychain_service(), dotted.keychain_service());
        assert_eq!(
            (root.keychain_scope_key(), web.keychain_scope_key()),
            ("", "apps/web")
        );

        let json = serde_json::to_string(&web).unwrap_or_default();
        assert_eq!(serde_json::from_str::<EnvScope>(&json).ok(), Some(web));

        for bad in "|/a|a/|a//b|a\\b|../x|a/../b|.env|a/.env|-x|a/b/c/d/e/f/g/h/i".split('|') {
            assert!(
                EnvScope::new("cheap-otto", "dev", Some(bad)).is_err(),
                "{bad}"
            );
        }
        for bad_json in [
            r#"{"project":"../bad","profile":"dev"}"#,
            r#"{"project":" cheap-otto ","profile":"dev"}"#,
        ] {
            assert!(serde_json::from_str::<EnvScope>(bad_json).is_err());
        }
        Ok(())
    }

    #[test]
    fn scope_identifiers_are_case_canonical_so_store_paths_cannot_collide_on_case_insensitive_filesystems()
    -> Result<(), EnvError> {
        let lower = EnvScope::new("foo", "dev", Some("apps/web"))?;
        let mixed = EnvScope::new("Foo", "DEV", Some("Apps/Web"))?;
        assert_eq!(mixed, lower);
        assert_eq!(mixed.relative_store_path(), lower.relative_store_path());
        assert_eq!(mixed.keychain_service(), lower.keychain_service());
        assert_eq!(
            (mixed.project(), mixed.profile(), mixed.subpath()),
            ("foo", "dev", Some("apps/web"))
        );
        assert_eq!(EnvScope::new("foo", "dev", None)?.subpath(), None);
        let rejected = serde_json::from_str::<EnvScope>(r#"{"project":"Foo","profile":"dev"}"#)
            .map_err(|e| e.to_string())
            .err()
            .unwrap_or_default();
        assert!(
            rejected.contains(&EnvError::NonCanonicalScope("project".to_string()).to_string()),
            "{rejected}"
        );
        Ok(())
    }

    #[test]
    fn agent_dummy_marker_does_not_survive_an_intervening_comment_or_blank_line()
    -> Result<(), EnvError> {
        let schema = parse_env_example("# @otto:agent-dummy=abc\n# ordinary note\nSOME_KEY=\n")?;
        assert!(schema.agent_dummies.is_empty());
        let schema = parse_env_example("# @otto:agent-dummy=abc\n\nSOME_KEY=\n")?;
        assert!(schema.agent_dummies.is_empty());
        let schema = parse_env_example("# @otto:agent-dummy=abc\nSOME_KEY=\n")?;
        assert_eq!(
            schema.agent_dummies.get("SOME_KEY").map(String::as_str),
            Some("abc")
        );
        Ok(())
    }

    #[test]
    fn rejects_blocked_execution_hijack_and_malformed_env_keys() {
        assert_eq!(validate_env_key("DATABASE_URL"), Ok("DATABASE_URL"));
        assert_eq!(validate_env_key("_INTERNAL_API_2"), Ok("_INTERNAL_API_2"));
        for k in "dyld_insert_libraries|LD_PRELOAD|Path|GIT_SSH_COMMAND".split('|') {
            assert_eq!(
                validate_env_key(k),
                Err(EnvError::BlockedKey(k.to_string()))
            );
        }
        for k in "|9STARTS_DIGIT|BAD-KEY|A=B|SPACE KEY".split('|') {
            assert!(matches!(validate_env_key(k), Err(EnvError::InvalidKey(_))));
        }
    }

    #[test]
    fn treats_hash_after_whitespace_following_equals_as_inline_comment() -> Result<(), EnvError> {
        let parsed = parse_dotenv("A= # comment\nB=\t#tab comment\nC= v # trailing\n")?;
        for (k, expected) in [("A", ""), ("B", ""), ("C", "v")] {
            assert_eq!(parsed.entries.get(k).map(String::as_str), Some(expected));
        }
        Ok(())
    }

    #[test]
    fn keeps_hash_directly_after_equals_as_value() -> Result<(), EnvError> {
        let parsed = parse_dotenv("A=#F7D842\nB=#x # comment\n")?;
        assert_eq!(parsed.entries.get("A").map(String::as_str), Some("#F7D842"));
        assert_eq!(parsed.entries.get("B").map(String::as_str), Some("#x"));
        Ok(())
    }

    #[test]
    fn preserves_first_seen_order_and_reports_each_duplicate_key_once() -> Result<(), EnvError> {
        let raw = "B=1\nA=2\nB=3\nC=4\nB=5\nA=6\n";
        assert_eq!(parse_env_example(raw)?.keys, vec!["B", "A", "C"]);
        let parsed = parse_dotenv(raw)?;
        assert_eq!(parsed.duplicate_keys, vec!["B", "A"]);
        assert_eq!(parsed.entries.get("B").map(String::as_str), Some("5"));
        assert_eq!(parsed.entries.get("A").map(String::as_str), Some("6"));
        Ok(())
    }

    #[test]
    fn parses_quoted_escaped_and_commented_dotenv_and_rejects_nul_or_unclosed_quotes()
    -> Result<(), EnvError> {
        let sample = "\u{feff}# leading comment\r\n\
            export API_URL=https://api.example.com/v1#anchor # trailing comment\r\n\
            SINGLE_QUOTED='literal $HOME \\n # kept'\n\
            DOUBLE_QUOTED=\"line1\\nline2 \\\"quoted\\\" \\$ literal\"\n\
            BRAND_HEX=#F7D842\n\
            API_URL=https://override.example.com\n";
        let parsed = parse_dotenv(sample)?;
        for (k, expected) in [
            ("API_URL", "https://override.example.com"),
            ("SINGLE_QUOTED", "literal $HOME \\n # kept"),
            ("DOUBLE_QUOTED", "line1\nline2 \"quoted\" $ literal"),
            ("BRAND_HEX", "#F7D842"),
        ] {
            assert_eq!(parsed.entries.get(k).map(String::as_str), Some(expected));
        }
        assert_eq!(parsed.duplicate_keys, vec!["API_URL".to_string()]);
        assert_eq!(
            parse_dotenv(&render_dotenv(&parsed.entries)?)?.entries,
            parsed.entries
        );
        assert_eq!(parse_dotenv("A=1\0B=2"), Err(EnvError::NulByte));
        assert_eq!(parse_dotenv("A=\"\\0\""), Err(EnvError::NulByte));
        assert_eq!(
            parse_dotenv("BROKEN=\"unterminated"),
            Err(EnvError::UnclosedQuote(1, '"'))
        );
        assert!(matches!(
            parse_dotenv("TRAILING='val' extra"),
            Err(EnvError::Syntax(1, _))
        ));
        Ok(())
    }

    #[test]
    fn parses_env_example_and_rejects_credential_bearing_agent_dummies() -> Result<(), EnvError> {
        let ex = "# @otto:agent-dummy=postgres://127.0.0.1:5432/app_test\nDATABASE_URL=\nSTRIPE_SECRET_KEY=\n";
        let schema = parse_env_example(ex)?;
        assert_eq!(schema.keys, vec!["DATABASE_URL", "STRIPE_SECRET_KEY"]);
        assert_eq!(
            schema.agent_dummies.get("DATABASE_URL").map(String::as_str),
            Some("postgres://127.0.0.1:5432/app_test")
        );
        assert!(!schema.agent_dummies.contains_key("STRIPE_SECRET_KEY"));

        for bad in [
            "# @otto:agent-dummy=postgres://admin:secret@127.0.0.1:5432/db\nDATABASE_URL=\n",
            "# @otto:agent-dummy=sk-proj-123456789\nOPENAI_API_KEY=\n",
            "# @otto:agent-dummy=ghp_1234567890abcdef\nGITHUB_API_TOKEN=\n",
            "# @otto:agent-dummy=eyJhbGciOiJIUzI1NiJ9.x.y\nJWT_TOKEN=\n",
            "# @otto:agent-dummy=xoxs-123456\nSLACK_TOKEN=\n",
            "# @otto:agent-dummy=xoxe.xoxp-1-abc\nSLACK_REFRESH_TOKEN=\n",
            "# @otto:agent-dummy=xoxe-1-abc\nSLACK_CONFIG_TOKEN=\n",
        ] {
            assert!(matches!(
                parse_env_example(bad),
                Err(EnvError::UnsafeAgentDummy(_, _))
            ));
        }
        Ok(())
    }
}
