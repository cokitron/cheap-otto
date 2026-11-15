---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---

# Clippy, Formatting & Documentation Standards

## 1. Zero-Warning Policy

The gate is the default Clippy lint set with warnings denied, across the whole workspace:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- `--workspace` matters. Without it you may lint only one package and miss the other.
- **Pedantic is not part of the gate.** `-W clippy::pedantic` reports dozens of warnings in the root crate today. Don't enable it in manifests or CI, and don't sweep those warnings in an unrelated change. Writing new code that would pass pedantic is welcome.
- **Fix the cause, not the lint.** The root crate has no `#[allow(...)]` attributes: when a handler grew too many parameters it got an options struct. `code-reviewer` has a handful of `#[allow(clippy::too_many_arguments)]`; don't add more, and never add a crate-level `#![allow]`.

### Lints that come up most here
- **Needless clones and allocations:** pass `&T`, `&str`, `&Path`; skip `.to_string()` when a `&'static str` will do.
- **Lossless casts:** `u8::from(bool)` and `u64::from(x)` instead of `as`.
- **Inline format args:** `println!("  * {line}")`, not `println!("  * {}", line)`.
- **Large enum variants:** box them, as `ScheduleCommands::Add(Box<ScheduleAddArgs>)` does.
- **Focused functions:** split a function that mixes parsing, policy and formatting.

---

## 2. Rustfmt

- Standard `rustfmt`, no `rustfmt.toml`. Run `cargo fmt --all` after editing; CI checks with `--check`.
- No `#[rustfmt::skip]` anywhere today. Keep it that way unless a lookup table is clearly harder to read when formatted.
- The embedded HTML pages are raw string literals, which `rustfmt` leaves alone, so their formatting is yours to keep tidy.

---

## 3. Rustdoc & Comments

- **Module docs:** every module starts with a `//!` header that says what it owns and how it fits the larger flow. Update it when the module's job changes.
- **Public API docs:** every public struct, enum, field and function in the root library has a `///` comment explaining why it exists and what it preserves. Fewer than ten items lack one today, so don't add to that count.
- **`# Errors` section** on public functions returning `Result`, naming the variants and the conditions.
- **clap doc comments are the help text.** A `///` on a subcommand or flag shows up in `--help`, so write it for the operator: purpose, units, default.
- **Comments explain why.**
  - Good: `// Keep normal battery sleep intact (5 min) in case AC power is disconnected for long periods`
  - Good: `// Rule 1: Fanless Thermal Guard (pause heavy queues when thermal state >= Serious)`
  - Don't restate the code.

---

## 4. Completion Checklist

Run these before saying a change is done. They are quick; run them rather than predicting the result.

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo test --workspace --all-targets`
4. If CLI behaviour changed: run the subcommand as a dry run against a temp state dir, for example
   `cargo run -p cheap-otto -- delegate --project demo --id t1 --agent claude --task "x" --state-dir <tmp>`,
   and read the printed plan. Never add `--execute` or `--apply` to verify.
5. If a flag, default or command name changed: update the README, `docs/index.html` and the `CLAUDE.md` command list in the same change.

Report what you ran and what it printed. If a gate fails, say so with the output.
