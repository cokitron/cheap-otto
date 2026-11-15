//! Request guard for the dashboard server: decides from a request's method and its `Host`,
//! `Origin` and `Content-Type` headers whether it may reach the router.
//!
//! The dashboard has no authentication and its `POST` routes start reviews, push fixes and
//! rewrite `config/repos.toml`. The loopback bind is the only barrier, and a web page open in
//! the operator's browser can still send requests to loopback, so:
//! - every request must name this server in `Host` (DNS-rebinding defence);
//! - a state-changing request that carries an `Origin` must carry an allowlisted one;
//! - a `POST` with a body must be `application/json`, which a foreign page cannot send without
//!   a CORS preflight this server never approves.
//!
//! Clients that send no `Origin` (`curl`, agents) are unaffected. This synchronous module avoids
//! async runtimes, HTTP clients and TLS stacks. It mirrors `src/request_guard.rs` in the root
//! crate, which cannot be a dependency here because it would pull in the async stack; change the
//! two together.

use thiserror::Error;

/// Host names that always reach a loopback listener, as they appear in a `Host` header.
pub const LOOPBACK_HOST_NAMES: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// Why a listener bind address was refused.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RequestGuardError {
    #[error("refusing to bind {0:?}: the dashboard has no authentication and is loopback-only")]
    NonLoopbackBind(String),
}

/// Why the guard refused a request.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GuardRejection {
    #[error("request refused: no Host header")]
    MissingHost,
    #[error("request refused: Host is not a name this server answers to")]
    HostNotAllowed,
    #[error("cross-origin request refused: Origin is not on this server's allowlist")]
    OriginNotAllowed,
    #[error("request refused: a POST body must be sent as Content-Type: application/json")]
    BodyNotJson,
}

impl GuardRejection {
    /// HTTP status the server answers with.
    pub fn http_status(self) -> u16 {
        match self {
            Self::MissingHost | Self::HostNotAllowed | Self::OriginNotAllowed => 403,
            Self::BodyNotJson => 415,
        }
    }
}

/// Allowlists of `Host` names and web origins for one listener, held in the server context.
///
/// It starts with the server's own loopback origins only. `trust_loopback_port` is the way to
/// let another local dashboard send state-changing requests here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestGuard {
    allowed_hosts: Vec<String>,
    trusted_origins: Vec<String>,
}

impl RequestGuard {
    /// Guard for a server listening on loopback `port`: accepts the loopback `Host` names and
    /// trusts the server's own loopback origins. Callers must refuse a non-loopback bind first
    /// with `require_loopback_bind`, because the guard is not authentication.
    pub fn for_listener(port: u16) -> Self {
        let mut guard = Self {
            allowed_hosts: LOOPBACK_HOST_NAMES
                .iter()
                .map(|h| (*h).to_string())
                .collect(),
            trusted_origins: Vec::with_capacity(3),
        };
        guard.trust_loopback_port(port);
        guard
    }

    /// Trusts the three loopback origins of a local server listening on `port`. This only lifts
    /// the guard: a cross-origin page also needs CORS response headers, and none are sent.
    pub fn trust_loopback_port(&mut self, port: u16) {
        for host in LOOPBACK_HOST_NAMES {
            push_unique(&mut self.trusted_origins, http_origin(host, port));
        }
    }

    /// Web origins allowed to send state-changing requests.
    pub fn trusted_origins(&self) -> &[String] {
        &self.trusted_origins
    }

    /// Decides whether a request may reach the router.
    ///
    /// 1. Every request must carry a `Host` whose name is accepted.
    /// 2. A state-changing request that carries an `Origin` must carry a trusted one.
    /// 3. A request with no `Origin` passes rule 2.
    ///
    /// `host` and `origin` are raw header values; an unreadable header must be passed as
    /// `Some("")` so it is refused rather than treated as absent.
    pub fn evaluate(
        &self,
        method: &str,
        host: Option<&str>,
        origin: Option<&str>,
    ) -> Result<(), GuardRejection> {
        let host = host.ok_or(GuardRejection::MissingHost)?;
        let name = host_name_of(host).ok_or(GuardRejection::HostNotAllowed)?;
        if !self.allowed_hosts.contains(&name) {
            return Err(GuardRejection::HostNotAllowed);
        }
        if is_state_changing_method(method)
            && let Some(origin) = origin
            && !self
                .trusted_origins
                .contains(&origin.trim().to_ascii_lowercase())
        {
            return Err(GuardRejection::OriginNotAllowed);
        }
        Ok(())
    }
}

/// Accepts only a loopback bind host (`127.0.0.1`, `localhost`, `::1` or `[::1]`, any case).
///
/// # Errors
/// `RequestGuardError::NonLoopbackBind` for anything else, including the empty string and every
/// wildcard spelling.
pub fn require_loopback_bind(host: &str) -> Result<(), RequestGuardError> {
    match host.trim().to_ascii_lowercase().as_str() {
        "127.0.0.1" | "localhost" | "::1" | "[::1]" => Ok(()),
        _ => Err(RequestGuardError::NonLoopbackBind(host.to_string())),
    }
}

/// Requires `Content-Type: application/json` on a `POST` that declares a body.
///
/// Every handler parses its body as JSON whatever the header says, so without this a foreign
/// page could send JSON as `text/plain`, a "simple" request that browsers do not preflight.
pub fn require_json_body(
    method: &str,
    content_type: Option<&str>,
    content_length: usize,
) -> Result<(), GuardRejection> {
    if method != "POST" || content_length == 0 {
        return Ok(());
    }
    let media_type = content_type
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if media_type.eq_ignore_ascii_case("application/json") {
        Ok(())
    } else {
        Err(GuardRejection::BodyNotJson)
    }
}

/// Whether `method` can change state. Only `GET`, `HEAD` and `OPTIONS` cannot; `POST`, `PUT`,
/// `PATCH`, `DELETE` and any unknown method are treated as state-changing.
pub fn is_state_changing_method(method: &str) -> bool {
    !matches!(method, "GET" | "HEAD" | "OPTIONS")
}

/// Extracts the lowercase host name from a `Host` header value or a bind address (`name`,
/// `name:port`, `[v6]` or `[v6]:port`). Returns `None` for anything else.
pub fn host_name_of(host_header: &str) -> Option<String> {
    let value = host_header.trim().to_ascii_lowercase();
    let (name, port) = if value.starts_with('[') {
        let end = value.find(']')?;
        let (name, rest) = value.split_at(end + 1);
        match rest.strip_prefix(':') {
            Some(port) => (name, Some(port)),
            None if rest.is_empty() => (name, None),
            None => return None,
        }
    } else {
        match value.rsplit_once(':') {
            Some((name, port)) => (name, Some(port)),
            None => (value.as_str(), None),
        }
    };
    let port_ok = port.is_none_or(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    let name_ok = !name.is_empty() && (name.starts_with('[') || !name.contains(':'));
    (port_ok && name_ok).then(|| name.to_string())
}

fn http_origin(host: &str, port: u16) -> String {
    // Browsers omit the scheme's default port when they serialise an origin.
    if port == 80 {
        format!("http://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

fn push_unique(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> RequestGuard {
        RequestGuard::for_listener(7430)
    }

    #[test]
    fn trusts_only_own_loopback_origins_by_default() {
        assert_eq!(
            guard().trusted_origins(),
            [
                "http://127.0.0.1:7430",
                "http://localhost:7430",
                "http://[::1]:7430"
            ]
        );
    }

    #[test]
    fn allows_same_origin_post_through_an_ssh_tunnel() {
        let guard = guard();
        for (host, origin) in [
            ("127.0.0.1:7430", "http://127.0.0.1:7430"),
            ("localhost:7430", "http://localhost:7430"),
            ("[::1]:7430", "http://[::1]:7430"),
        ] {
            assert_eq!(guard.evaluate("POST", Some(host), Some(origin)), Ok(()));
        }
    }

    #[test]
    fn allows_post_without_origin_for_curl_and_agents() {
        assert_eq!(
            guard().evaluate("POST", Some("127.0.0.1:7430"), None),
            Ok(())
        );
    }

    #[test]
    fn rejects_state_changing_request_from_foreign_null_or_lookalike_origin() {
        let guard = guard();
        for origin in [
            "http://evil.example",
            "null",
            "",
            "http://127.0.0.1:7420",
            "https://127.0.0.1:7430",
            "http://127.0.0.1:7430.evil.example",
        ] {
            for method in ["POST", "PUT", "PATCH", "DELETE", "post"] {
                assert_eq!(
                    guard.evaluate(method, Some("127.0.0.1:7430"), Some(origin)),
                    Err(GuardRejection::OriginNotAllowed),
                    "{method} from {origin}"
                );
            }
        }
    }

    #[test]
    fn lets_safe_methods_through_from_any_origin() {
        let guard = guard();
        for method in ["GET", "HEAD", "OPTIONS"] {
            assert_eq!(
                guard.evaluate(method, Some("localhost:7430"), Some("http://evil.example")),
                Ok(())
            );
        }
    }

    #[test]
    fn rejects_rebound_missing_and_malformed_host() {
        let guard = guard();
        assert_eq!(
            guard.evaluate("GET", None, None),
            Err(GuardRejection::MissingHost)
        );
        for host in [
            "evil.example",
            "evil.example:7430",
            "",
            "127.0.0.1.evil.example",
            "localhost@evil.example",
            "127.0.0.1:7430@evil.example",
            "127.0.0.1:",
            "::1",
            "[::1]evil.example",
        ] {
            assert_eq!(
                guard.evaluate("GET", Some(host), Some("http://127.0.0.1:7430")),
                Err(GuardRejection::HostNotAllowed),
                "{host}"
            );
        }
    }

    #[test]
    fn accepts_only_loopback_bind_hosts() {
        for host in ["127.0.0.1", "localhost", "LocalHost", " ::1 ", "[::1]"] {
            assert_eq!(require_loopback_bind(host), Ok(()), "{host}");
        }
    }

    #[test]
    fn rejects_non_loopback_and_wildcard_bind_hosts() {
        for host in [
            "",
            "0.0.0.0",
            "::",
            "[::]",
            "::0",
            "[0:0:0:0:0:0:0:0]",
            "192.168.1.50",
            "100.101.102.103",
            "example.com",
        ] {
            assert_eq!(
                require_loopback_bind(host),
                Err(RequestGuardError::NonLoopbackBind(host.to_string())),
                "{host}"
            );
        }
    }

    #[test]
    fn allowlist_grows_by_loopback_port() {
        let mut guard = guard();
        let from_mission_control = Some("http://localhost:7420");
        assert_eq!(
            guard.evaluate("POST", Some("localhost:7430"), from_mission_control),
            Err(GuardRejection::OriginNotAllowed)
        );
        guard.trust_loopback_port(7420);
        assert_eq!(
            guard.evaluate("POST", Some("localhost:7430"), from_mission_control),
            Ok(())
        );
    }

    #[test]
    fn requires_json_content_type_on_post_with_body() {
        for content_type in [
            None,
            Some("text/plain"),
            Some("text/plain;charset=UTF-8"),
            Some("application/x-www-form-urlencoded"),
            Some("multipart/form-data; boundary=x"),
            Some("application/jsonp"),
            Some(""),
        ] {
            assert_eq!(
                require_json_body("POST", content_type, 2),
                Err(GuardRejection::BodyNotJson),
                "{content_type:?}"
            );
        }
        for content_type in [
            "application/json",
            "Application/JSON",
            "application/json; charset=utf-8",
        ] {
            assert_eq!(require_json_body("POST", Some(content_type), 2), Ok(()));
        }
    }

    #[test]
    fn skips_content_type_rule_without_a_body_or_on_other_methods() {
        assert_eq!(require_json_body("POST", None, 0), Ok(()));
        assert_eq!(require_json_body("GET", Some("text/plain"), 9), Ok(()));
    }

    #[test]
    fn maps_rejections_to_http_status() {
        assert_eq!(GuardRejection::MissingHost.http_status(), 403);
        assert_eq!(GuardRejection::HostNotAllowed.http_status(), 403);
        assert_eq!(GuardRejection::OriginNotAllowed.http_status(), 403);
        assert_eq!(GuardRejection::BodyNotJson.http_status(), 415);
    }
}
