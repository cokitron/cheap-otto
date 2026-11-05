---
paths:
  - "**/*.rs"
  - "Cargo.toml"
---

# Clippy, Formatting & Documentation Standards

## 1. Zero-Warning Compiler & Clippy Policy

All code in `cheap-otto` must compile with **zero warnings** under `rustc` and `clippy`:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Key Lint Rules to Respect
- **No needless clones or allocations:** Pass references (`&T`, `&str`, `&Path`) unless ownership is required; avoid `.to_string()` when `&'static str` or `format!` directly into a buffer works.
- **Use explicit cast conversion where lossless:** Prefer `u8::from(bool)` over `bool as u8`.
- **Format string inline variables:** Prefer `println!("  * {line}")` and `format!("Fix: {fix}")` (Rust 2021/2024 inline format args) when interpolating simple identifiers.
- **Keep functions focused and readable:** Extract helper functions if cyclomatic complexity grows or if a function mixes parsing, policy evaluation, and string formatting.

---

## 2. Rustfmt Standards

- All Rust files must be formatted with standard `rustfmt` (`cargo fmt --all`).
- Do not use `#[rustfmt::skip]` unless aligning a complex lookup table that is demonstrably harder to read when formatted.

---

## 3. Rustdoc & Inline Comments

- **Document All Public APIs:**
  - Every public module, struct, enum, and public method in `src/*.rs` should have a concise `///` doc comment explaining **why** it exists and what invariants it preserves.
  - Document `# Errors` on public functions returning `Result<T, E>` to specify the exact conditions under which an `Err` is returned.
- **Explain "Why", Not "What" in Inline Comments:**
  - Good: `// Keep normal battery sleep intact (5 min) in case AC power is disconnected for long periods`
  - Good: `// Rule 1: Fanless Thermal Guard (pause heavy queues when thermal state >= Serious)`
  - Avoid redundant comments that merely restate the Rust syntax.

---

## 4. Pre-Completion Verification Checklist

Before declaring any code change complete, always execute:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`
4. If CLI behavior changed, verify with `cargo run -- <subcommand>`.
