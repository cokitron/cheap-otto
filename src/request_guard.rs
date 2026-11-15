//! Request guard for `cheap-otto serve`: decides from a request's method, `Host` and `Origin`
//! alone whether it may reach a handler.
//!
//! The control plane has no authentication. The loopback bind is the only barrier, and a web
//! page open in the operator's browser can still send requests to loopback. The guard closes the
//! two ways such a page gets in:
//! - **DNS rebinding.** A hostile name re-pointed at `127.0.0.1` still sends its own name in
//!   `Host`, so every request must name a host this server answers to.
//! - **Cross-site requests.** Browsers attach `Origin` to every cross-origin request that can
//!   change state, so a state-changing request whose `Origin` is not on the allowlist is refused.
//!
//! Clients that send no `Origin` (the CLI, `curl`, external agents) are unaffected. The module
//! is pure: `src/server.rs` extracts the headers in a thin middleware and calls
//! [`RequestGuard::evaluate`]. `crates/code-reviewer/src/request_guard.rs` applies the same
//! rules to the reviewer dashboard; change the two together.

use thiserror::Error;

/// Host names that always reach a loopback listener, as they appear in a `Host` header.
pub const LOOPBACK_HOST_NAMES: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// Why the guard refused a request. Every variant is answered with `403 Forbidden`.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GuardRejection {
    /// The request carried no `Host` header (and no authority in its target).
    #[error("request refused: no Host header")]
    MissingHost,
    /// `Host` names something other than this server: the signature of DNS rebinding.
    #[error("request refused: Host is not a name this server answers to")]
    HostNotAllowed,
    /// A state-changing request came from a web origin that is not on the allowlist.
    #[error("cross-origin request refused: Origin is not on this server's allowlist")]
    OriginNotAllowed,
}

/// Errors from growing the origin allowlist.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RequestGuardError {
    /// The value is not a bare web origin (`http://host[:port]` or `https://host[:port]`).
    #[error("`{0}` is not a web origin: expected `http://host[:port]` with no path")]
    InvalidOrigin(String),
}

/// Allowlists of `Host` names and web origins for one local server.
///
/// Built once per listener with [`RequestGuard::for_listener`] and held in the server state.
/// It starts with the server's own origins only; [`RequestGuard::trust_loopback_port`] and
/// [`RequestGuard::trust_origin`] grow the allowlist when another local dashboard must be
/// allowed to call this API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestGuard {
    allowed_hosts: Vec<String>,
    trusted_origins: Vec<String>,
}

impl RequestGuard {
    /// Guard for a server listening on `bind_host:port`.
    ///
    /// Accepts the loopback `Host` names and trusts the server's own loopback origins
    /// (`http://127.0.0.1:<port>`, `http://localhost:<port>`, `http://[::1]:<port>`). A specific
    /// non-loopback `bind_host` is added as a `Host` name together with its own origin, so a
    /// server deliberately bound to a LAN address keeps serving its page. A wildcard bind
    /// (`0.0.0.0`, `::`) names no host a client can address and adds nothing.
    #[must_use = "the guard does nothing until the server evaluates requests with it"]
    pub fn for_listener(bind_host: &str, port: u16) -> Self {
        let mut guard = Self {
            allowed_hosts: LOOPBACK_HOST_NAMES
                .iter()
                .map(|h| (*h).to_string())
                .collect(),
            trusted_origins: Vec::with_capacity(4),
        };
        guard.trust_loopback_port(port);
        let bound = normalize_bind_host(bind_host);
        if !bound.is_empty() && !is_wildcard_host(&bound) && !guard.allowed_hosts.contains(&bound) {
            push_unique(&mut guard.trusted_origins, http_origin(&bound, port));
            guard.allowed_hosts.push(bound);
        }
        guard
    }

    /// Accepts `host` (a name or address, without a port) in the `Host` header.
    pub fn allow_host(&mut self, host: &str) {
        let name = normalize_bind_host(host);
        if !name.is_empty() {
            push_unique(&mut self.allowed_hosts, name);
        }
    }

    /// Trusts the three loopback origins of another local server listening on `port`.
    ///
    /// This is how one dashboard is allowed to send state-changing requests to another's API.
    /// It only lifts the guard: a cross-origin page also needs CORS response headers to read
    /// replies or to send JSON, and this server sends none.
    pub fn trust_loopback_port(&mut self, port: u16) {
        for host in LOOPBACK_HOST_NAMES {
            push_unique(&mut self.trusted_origins, http_origin(host, port));
        }
    }

    /// Trusts one explicit web origin, such as `http://127.0.0.1:7430`.
    ///
    /// # Errors
    /// Returns [`RequestGuardError::InvalidOrigin`] when `origin` is not a bare
    /// `http://host[:port]` or `https://host[:port]` value (a path, query, user-info or
    /// whitespace makes it invalid).
    pub fn trust_origin(&mut self, origin: &str) -> Result<(), RequestGuardError> {
        let normalized = normalize_origin(origin)
            .ok_or_else(|| RequestGuardError::InvalidOrigin(origin.to_string()))?;
        push_unique(&mut self.trusted_origins, normalized);
        Ok(())
    }

    /// `Host` names this server answers to (lowercase, no port).
    #[must_use = "the accepted host list is only useful when inspected or printed"]
    pub fn allowed_hosts(&self) -> &[String] {
        &self.allowed_hosts
    }

    /// Web origins allowed to send state-changing requests (lowercase, no trailing slash).
    #[must_use = "the origin allowlist is only useful when inspected or printed"]
    pub fn trusted_origins(&self) -> &[String] {
        &self.trusted_origins
    }

    /// Decides whether a request may reach a handler.
    ///
    /// 1. Every request must carry a `Host` whose name is accepted (DNS-rebinding defence).
    /// 2. A state-changing request (see [`is_state_changing_method`]) that carries an `Origin`
    ///    must carry a trusted one.
    /// 3. A request with no `Origin` passes rule 2: that is the CLI and external agents.
    ///
    /// `host` and `origin` are the raw header values. Pass `Some("")` for a header that is
    /// present but not valid text, so it is refused rather than treated as absent.
    ///
    /// # Errors
    /// Returns the [`GuardRejection`] naming the rule that failed.
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

/// Whether `method` can change state. Only the safe methods `GET`, `HEAD` and `OPTIONS` cannot;
/// `POST`, `PUT`, `PATCH`, `DELETE` and any unknown method are treated as state-changing.
#[must_use = "the classification decides whether the Origin rule applies"]
pub fn is_state_changing_method(method: &str) -> bool {
    !matches!(method, "GET" | "HEAD" | "OPTIONS")
}

/// Extracts the lowercase host name from a `Host` header value (`name`, `name:port`,
/// `[v6]` or `[v6]:port`). Returns `None` for anything else, so a malformed value never matches.
#[must_use = "a malformed Host must be refused, not ignored"]
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

fn normalize_bind_host(host: &str) -> String {
    let trimmed = host.trim().to_ascii_lowercase();
    // A bare IPv6 address is written in brackets in `Host` and in an origin.
    if trimmed.contains(':') && !trimmed.starts_with('[') {
        format!("[{trimmed}]")
    } else {
        trimmed
    }
}

fn is_wildcard_host(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "[::]" | "[::0]")
}

fn http_origin(host: &str, port: u16) -> String {
    // Browsers omit the scheme's default port when they serialise an origin.
    if port == 80 {
        format!("http://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

fn normalize_origin(origin: &str) -> Option<String> {
    let trimmed = origin.trim().to_ascii_lowercase();
    let value = trimmed.strip_suffix('/').unwrap_or(&trimmed);
    let authority = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))?;
    let clean = !authority.is_empty()
        && !authority.contains(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '@'));
    (clean && host_name_of(authority).is_some()).then(|| value.to_string())
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
        RequestGuard::for_listener("127.0.0.1", 7420)
    }

    #[test]
    fn trusts_only_own_loopback_origins_by_default() {
        let guard = guard();
        assert_eq!(
            guard.trusted_origins(),
            [
                "http://127.0.0.1:7420",
                "http://localhost:7420",
                "http://[::1]:7420"
            ]
        );
        assert_eq!(guard.allowed_hosts(), LOOPBACK_HOST_NAMES);
    }

    #[test]
    fn allows_same_origin_post_through_an_ssh_tunnel() {
        let guard = guard();
        for (host, origin) in [
            ("127.0.0.1:7420", "http://127.0.0.1:7420"),
            ("localhost:7420", "http://localhost:7420"),
            ("[::1]:7420", "http://[::1]:7420"),
            ("LOCALHOST:7420", "HTTP://LocalHost:7420"),
        ] {
            assert_eq!(guard.evaluate("POST", Some(host), Some(origin)), Ok(()));
        }
    }

    #[test]
    fn allows_state_changing_request_without_origin_for_cli_and_agents() {
        let guard = guard();
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            assert_eq!(guard.evaluate(method, Some("127.0.0.1:7420"), None), Ok(()));
        }
    }

    #[test]
    fn rejects_state_changing_request_from_foreign_origin() {
        let guard = guard();
        for method in ["POST", "PUT", "PATCH", "DELETE", "PROPFIND", "post"] {
            assert_eq!(
                guard.evaluate(method, Some("127.0.0.1:7420"), Some("http://evil.example")),
                Err(GuardRejection::OriginNotAllowed),
                "{method}"
            );
        }
    }

    #[test]
    fn rejects_lookalike_null_and_unreadable_origins() {
        let guard = guard();
        for origin in [
            "null",
            "",
            "http://127.0.0.1:7421",
            "https://127.0.0.1:7420",
            "http://127.0.0.1:7420.evil.example",
            "http://localhost.evil.example:7420",
            "http://evil.example/?http://127.0.0.1:7420",
            "http://127.0.0.1",
        ] {
            assert_eq!(
                guard.evaluate("POST", Some("localhost:7420"), Some(origin)),
                Err(GuardRejection::OriginNotAllowed),
                "{origin}"
            );
        }
    }

    #[test]
    fn lets_safe_methods_through_from_any_origin() {
        let guard = guard();
        for method in ["GET", "HEAD", "OPTIONS"] {
            assert_eq!(
                guard.evaluate(method, Some("127.0.0.1:7420"), Some("http://evil.example")),
                Ok(())
            );
        }
    }

    #[test]
    fn rejects_rebound_host_on_every_method() {
        let guard = guard();
        for method in ["GET", "HEAD", "OPTIONS", "POST", "DELETE"] {
            assert_eq!(
                guard.evaluate(method, Some("evil.example:7420"), None),
                Err(GuardRejection::HostNotAllowed)
            );
        }
        // A trusted Origin does not excuse a foreign Host.
        assert_eq!(
            guard.evaluate("POST", Some("evil.example"), Some("http://127.0.0.1:7420")),
            Err(GuardRejection::HostNotAllowed)
        );
    }

    #[test]
    fn rejects_missing_and_malformed_host() {
        let guard = guard();
        assert_eq!(
            guard.evaluate("GET", None, None),
            Err(GuardRejection::MissingHost)
        );
        for host in [
            "",
            "127.0.0.1.evil.example",
            "localhost.",
            "localhost@evil.example",
            "127.0.0.1:7420@evil.example",
            "127.0.0.1:",
            "127.0.0.1:74x0",
            "::1",
            "[::1",
            "[::1]evil.example",
            "evil.example:127.0.0.1",
        ] {
            assert_eq!(
                guard.evaluate("GET", Some(host), None),
                Err(GuardRejection::HostNotAllowed),
                "{host}"
            );
        }
    }

    #[test]
    fn accepts_loopback_host_with_any_or_no_port() {
        let guard = guard();
        for host in ["127.0.0.1", "localhost", "[::1]", "localhost:9999"] {
            assert_eq!(guard.evaluate("GET", Some(host), None), Ok(()), "{host}");
        }
    }

    #[test]
    fn keeps_serving_a_specific_non_loopback_bind_host() {
        let guard = RequestGuard::for_listener("192.168.1.50", 7420);
        assert_eq!(
            guard.evaluate(
                "POST",
                Some("192.168.1.50:7420"),
                Some("http://192.168.1.50:7420")
            ),
            Ok(())
        );
        assert_eq!(
            guard.evaluate(
                "POST",
                Some("127.0.0.1:7420"),
                Some("http://localhost:7420")
            ),
            Ok(())
        );
        assert_eq!(
            guard.evaluate("GET", Some("192.168.1.51:7420"), None),
            Err(GuardRejection::HostNotAllowed)
        );
    }

    #[test]
    fn wildcard_bind_adds_no_host_or_origin() {
        for bind in ["0.0.0.0", "::", "[::]"] {
            assert_eq!(RequestGuard::for_listener(bind, 7420), guard(), "{bind}");
        }
    }

    #[test]
    fn brackets_a_bare_ipv6_bind_host() {
        let guard = RequestGuard::for_listener("fd7a::1", 7420);
        assert_eq!(
            guard.evaluate(
                "POST",
                Some("[fd7a::1]:7420"),
                Some("http://[fd7a::1]:7420")
            ),
            Ok(())
        );
    }

    #[test]
    fn omits_default_port_in_own_origin() {
        let guard = RequestGuard::for_listener("127.0.0.1", 80);
        assert_eq!(
            guard.evaluate("POST", Some("localhost"), Some("http://localhost")),
            Ok(())
        );
    }

    #[test]
    fn allowlist_grows_by_loopback_port_or_explicit_origin() -> Result<(), RequestGuardError> {
        let mut guard = guard();
        let from_dashboard = Some("http://127.0.0.1:7430");
        assert_eq!(
            guard.evaluate("POST", Some("127.0.0.1:7420"), from_dashboard),
            Err(GuardRejection::OriginNotAllowed)
        );
        guard.trust_loopback_port(7430);
        assert_eq!(
            guard.evaluate("POST", Some("127.0.0.1:7420"), from_dashboard),
            Ok(())
        );

        guard.trust_origin("https://Otto.Tailnet.example/")?;
        assert_eq!(
            guard.evaluate(
                "DELETE",
                Some("localhost:7420"),
                Some("https://otto.tailnet.example")
            ),
            Ok(())
        );
        // Trusting an origin does not make its host an accepted `Host` name.
        assert_eq!(
            guard.evaluate("GET", Some("otto.tailnet.example"), None),
            Err(GuardRejection::HostNotAllowed)
        );
        guard.allow_host("Otto.Tailnet.example");
        assert_eq!(
            guard.evaluate("GET", Some("otto.tailnet.example"), None),
            Ok(())
        );
        Ok(())
    }

    #[test]
    fn rejects_values_that_are_not_bare_origins() {
        let mut guard = guard();
        for bad in [
            "",
            "127.0.0.1:7430",
            "http://",
            "ftp://127.0.0.1:7430",
            "http://127.0.0.1:7430/path",
            "http://user@127.0.0.1:7430",
            "http://127.0.0.1:7430?x=1",
            "http://127.0.0.1 :7430",
            "*",
            "null",
        ] {
            assert_eq!(
                guard.trust_origin(bad),
                Err(RequestGuardError::InvalidOrigin(bad.to_string())),
                "{bad}"
            );
        }
        assert_eq!(guard, RequestGuard::for_listener("127.0.0.1", 7420));
    }
}
