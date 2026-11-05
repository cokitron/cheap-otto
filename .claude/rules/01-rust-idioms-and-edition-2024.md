---
paths:
  - "**/*.rs"
  - "Cargo.toml"
---

# Rust Idioms & Edition 2024 Guidelines

This project targets **Rust Edition 2024** (`edition = "2024"` in `Cargo.toml`). Write idiomatic, zero-cost, expressive, and maintainable Rust that leverages the type system to make invalid states unrepresentable.

---

## 1. Type-Driven Design & Domain Modeling

- **Make Invalid States Unrepresentable:**
  - Prefer rich algebraic data types (`enum` and `struct`) over raw primitives or stringly-typed flags.
  - Example: Use `CpuArch::{AppleSilicon, X86_64, Arm64Generic}` and `ThermalLevel::{Nominal, Fair, Serious, Critical}` with `#[derive(PartialOrd, Ord)]` so thermal severity comparisons (`snapshot.thermal_level >= ThermalLevel::Serious`) are checked at compile time.
- **Derive Standard Traits Consistently:**
  - Public domain structs and enums should derive `Debug, Clone, PartialEq, Eq` (and `Copy` for small value enums like `CpuArch`, `ThermalLevel`, `CheckStatus`, `CapabilityPack`).
  - Derive `Serialize, Deserialize` (`serde`) on state snapshots, configuration backups, and reports so all plans and telemetry can be serialized deterministically.
- **Enforce `#[must_use]` on Pure Computations & Plans:**
  - Annotate functions that compute plans, budgets, health reports, or arbitration actions with `#[must_use = "reason explaining why ignoring the return value is a bug"]`.
  - Examples in this codebase:
    - `HardwareProfile::compute_memory_budget`
    - `MacOsStateBackup::build_clamshell_plan`
    - `ArbiterPolicy::evaluate`
    - `SystemProbeInput::evaluate_health`
    - `packs::render_compose_stack`

---

## 2. Error Handling Discipline

- **Library vs. Binary Error Boundary:**
  - **Library modules (`src/hardware.rs`, `src/macos.rs`, etc.):** Define granular, domain-specific error enums using `thiserror::Error` v2 (`#[derive(Debug, Error, PartialEq, Eq)]`). Derive `PartialEq, Eq` on domain errors whenever possible so unit tests can assert exact error variants with `assert_eq!`.
  - **Binary entrypoint (`src/main.rs`):** Propagate errors cleanly at the CLI boundary using `?` and clear user-facing context.
- **Zero `.unwrap()` or `.expect()` in Production Library Code:**
  - Never call `.unwrap()`, `.expect()`, `panic!()`, `todo!()`, or `unimplemented!()` in non-test library code (`src/*.rs` outside `#[cfg(test)]`).
  - Use combinators (`ok_or`, `ok_or_else`, `unwrap_or`, `unwrap_or_default`, `map_or`) or explicit pattern matching (`if let`, `let ... else`).
  - In `#[cfg(test)]` modules, prefer returning `Result<(), DomainError>` from test functions and using the `?` operator over `.unwrap()`.

---

## 3. Ownership, Borrowing & Memory Efficiency

- **Borrow Signatures Over Owned Inputs:**
  - Accept `&str` instead of `String`, `&Path` instead of `&PathBuf` or `PathBuf`, and `&[T]` instead of `&Vec<T>` in function parameters unless ownership transfer is required.
  - Examples: `run_init(..., out_dir: &Path)`, `render_compose_stack(packs: &[CapabilityPack], budget: &MemoryBudget)`, `from_pmset_custom(pmset_output: &str, ...)`.
- **Static Slices for Fixed Catalogs:**
  - Return `&'static [T]` or `&'static str` for compile-time constants (e.g., `CapabilityPack::all() -> &'static [CapabilityPack]`, `CapabilityPack::display_name(self) -> &'static str`) to avoid unnecessary heap allocations.
- **Pre-allocate Collections When Capacity Is Known:**
  - Use `Vec::with_capacity(n)` when building fixed-size reports (e.g., `Vec::with_capacity(8)` in `SystemProbeInput::evaluate_health`).

---

## 4. Control Flow & Pattern Matching

- **Exhaustive Matching:**
  - Match all enum variants explicitly (`match self { ... }`). Avoid catch-all `_ =>` arms when matching internal enums (`CpuArch`, `ThermalLevel`, `CapabilityPack`, `CheckStatus`, `ArbiterAction`) so the compiler flags every call site when a new variant is added.
  - Wildcard `_ => {}` is only acceptable when parsing external, untrusted key-value streams (`pmset` or `sysctl` lines) where unknown keys are intentionally ignored.
- **Prefer Iterator Pipelines & `matches!` Macro:**
  - Use `.iter().any(...)`, `.iter().all(...)`, and `matches!(...)` for expressive predicate checks instead of mutable boolean flags.
