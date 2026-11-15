---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---

# Rust Idioms & Edition 2024 Guidelines

Both crates target **Rust Edition 2024** on the stable toolchain (no `rust-toolchain.toml`, no nightly features). Write idiomatic Rust that uses the type system to make invalid states unrepresentable.

The two crates differ in strictness. The root `cheap-otto` library holds itself to every rule below. `crates/code-reviewer` is a single appliance binary: the no-panic and error-enum rules apply to it, and the rest are the direction to move in when you touch its code.

---

## 1. Type-Driven Design & Domain Modeling

- **Make invalid states unrepresentable.**
  - Prefer `enum` and `struct` over raw primitives or stringly-typed flags.
  - Example: `CpuArch::{AppleSilicon, X86_64, Arm64Generic}`, and `ThermalLevel::{Nominal, Fair, Serious, Critical}` deriving `PartialOrd, Ord` so that `snapshot.thermal_level >= ThermalLevel::Serious` is a compile-checked comparison.
- **Derive standard traits consistently.**
  - Public domain types derive `Debug, Clone, PartialEq, Eq`, plus `Copy` for small value enums (`CpuArch`, `ThermalLevel`, `CheckStatus`, `AgentRunner`, `ReasoningEffort`).
  - Derive `Serialize, Deserialize` on anything that is persisted or returned by an API: state snapshots, plans, registries, reports.
- **Put `#[must_use]` on pure computations and plans** (root crate: 70+ uses today). Give the reason:
  `#[must_use = "command specification should be stored in a plan or executed"]`.
  Examples: `HardwareProfile::compute_memory_budget`, `MacOsStateBackup::build_clamshell_plan`, `ArbiterPolicy::evaluate`, `SystemProbeInput::evaluate_health`, `CommandSpec::new`.

---

## 2. Error Handling Discipline

- **Fallible root library modules** define a domain error enum with `thiserror` v2: `#[derive(Debug, Error, PartialEq, Eq)]` (`BridgeError`, `WorktreeError`, `HarnessError`, `ScheduleError`, …). Keep `PartialEq, Eq` so tests can `assert_eq!` on exact variants.
- **`code-reviewer` modules** use `#[derive(Debug, Error)]` (`GitHubError`, `StateError`, `BrainError`, …) because they wrap I/O errors.
- **Binary execution paths** return `Result<(), Box<dyn std::error::Error>>` and propagate with `?`. In `src/main.rs` that is `main` itself; in `crates/code-reviewer/src/main.rs` it is `run()`, and `main` converts an error to an exit status.
- **No `.unwrap()`, `.expect()`, `panic!()`, `todo!()` or `unimplemented!()` outside `#[cfg(test)]`.** Both crates are at zero today, so keep them there.
  - Use combinators (`ok_or`, `ok_or_else`, `unwrap_or`, `unwrap_or_default`, `map_or`) or pattern matching (`if let`, `let … else`, which the code uses widely).
  - Handlers that must not fail on a spawn error discard it explicitly: `let _ = Command::new(…)…`.
- **In tests,** prefer `fn name() -> Result<(), DomainError>` with `?` in the root crate. `code-reviewer` tests use `.unwrap()`, which is fine there.

---

## 3. Ownership, Borrowing & Memory Efficiency

- **Borrow in signatures.** Take `&str`, `&Path` and `&[T]`, not `String`, `&PathBuf` or `&Vec<T>`, unless ownership moves.
- **Bundle many borrowed parameters into an options struct** rather than silencing `clippy::too_many_arguments`; use this for handlers with several related inputs. `run_delegate` is one example: `struct DelegateOptions<'a> { … }` and `fn run_delegate(opts: &DelegateOptions<'_>)`.
- **Return `&'static str` or `&'static [T]` for fixed catalogs** (display names, harness flag names) to avoid allocation.
- **Pre-allocate when the size is known**, as in `Vec::with_capacity(8)` in `SystemProbeInput::evaluate_health`.

---

## 4. Control Flow & Pattern Matching

- **Match exhaustively.** No catch-all `_ =>` arm on internal enums (`CpuArch`, `ThermalLevel`, `CheckStatus`, `ArbiterAction`, `AgentRunner`), so adding a variant flags every call site.
- A wildcard arm is fine only when parsing external key-value streams (`pmset`, `sysctl`, `stream-json` events) where unknown keys are ignored on purpose.
- **Prefer iterator pipelines and `matches!`** (`.iter().any(…)`, `.iter().all(…)`) over mutable boolean flags.
