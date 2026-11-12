# Active Lessons — `03-security-boundaries`

- **[L-SEC-001]** (`*`) Never log tokens, cookies, passwords, card numbers, CVV, or raw PII in error handlers, telemetry, or test snapshots.
- **[L-SEC-002]** (`*`) Verify proxy/BFF routes never forward raw upstream credentials or relax `HttpOnly`, `Secure`, and `SameSite` cookie attributes.
