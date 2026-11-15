---
paths:
  - "src/**/*.rs"
  - "tests/**/*.rs"
  - "crates/**/*.rs"
---

# Architecture & Testing Standards

## 1. Functional Core, Imperative Shell (`cheap-otto`)

The root crate separates pure decision-making from side effects.

1. **Functional core:** every module in `src/` except `main.rs` and `server.rs`.
   - Takes structured inputs or raw text snapshots (`&str`, `&HardwareProfile`, `&ArbiterSnapshot`, `&PeerNode`) and returns typed values, plans or domain errors.
   - Performs no filesystem I/O, no network requests and no process spawning. Side effects are *described* as `CommandSpec` values inside plan structs (`WorkspaceSyncPlan`, `DelegatedWorktreePlan`, `WorktreePullPlan`, `PortTunnelPlan`, `AppliancePowerPlan`).
   - Takes the current time as a parameter (`now_epoch_secs: u64`). The one exception is session-id generation in `harness.rs`.
   - Must compile and pass its tests on Windows, Linux and macOS without Apple hardware.
2. **Imperative shells:**
   - `src/main.rs` (sync): clap parsing, state JSON load and save, printing, and `execute_command_spec` behind `--execute`. Handlers are `run_<subcommand>(opts: &XOptions<'_>)`.
   - `src/server.rs` (async, axum): the `serve` REST API. It may spawn processes and read state, but it calls the same core planners as the CLI.
   - `scripts/bootstrap-macos.sh`: one-shot setup on the Mac.

**Where new code goes.** A new feature is a planner in the core that returns data, a thin handler in `main.rs` that prints and optionally executes it, and, if Mission Control needs it, a route in `server.rs` that calls the same planner. If you want to call `std::fs` or `Command` from a core module, the design is wrong: return a plan instead.

`code-reviewer` is a single binary whose modules mix I/O with logic. Keep the parts that are already pure that way and test them directly: diff hunk classification (`diff.rs`), trigger and observation state machines (`state.rs`), config parsing (`config.rs`), prompt and reply formatting.

---

## 2. Test Layout

| Kind | Where | How it runs |
| --- | --- | --- |
| Unit tests | `#[cfg(test)] mod tests` at the bottom of each module | `cargo test -p cheap-otto --lib <module>::tests` or `cargo test -p code-reviewer <module>::tests` |
| API tests | `src/server.rs`, `#[tokio::test]` | Build the router with `build_router` and drive it with `tower::ServiceExt::oneshot`. No socket is opened. Send a loopback `Host` header, or the request guard answers `403`. |
| CLI integration | `tests/cli_integration.rs` | Spawns the real binary via `env!("CARGO_BIN_EXE_cheap-otto")` in an isolated temp dir |

- Integration tests must stay hermetic: create a temp dir with `create_isolated_temp_dir`, pass `--out-dir` or `--state-dir` into it, pass `--offline` to `init` and `doctor`, and never pass `--execute` or `--apply`. They assert on the printed plan and on the state files written.
- `code-reviewer` has no integration tests and no library target, so its tests live inside the binary's modules.

---

## 3. What Every Change Needs

- **Naming:** snake_case sentences that state behaviour and condition, as the suite does today:
  - `evicts_local_llm_when_ci_build_starts`
  - `pauses_queues_and_evicts_llm_on_serious_thermal_pressure`
  - `rejects_unsafe_battery_ceiling`
  - `merges_ssh_config_block_idempotently_without_duplicating_entries`
  - `escapes_single_quotes_in_remote_safe_agent_invocation`
  - `status_banners_carry_anti_loop_markers_and_never_retrigger`
- **Coverage triad:**
  1. Happy path on the canonical hardware (16 GB M1 Air, AC power, nominal thermals).
  2. Boundary and degraded conditions: 8 GB x86, `Serious` thermals, the 300 s idle threshold, memory under 2048 MB, a CI build pre-empting the LLM.
  3. Invalid input: too little RAM, a ceiling outside `50..=100`, malformed or missing `pmset`/`sysctl` sections, unsafe identifiers.
- **Anything that builds a remote command gets an injection test** with quotes, `;`, `$()` and a leading `-` in the input.
- **Deterministic assertions:** compare whole structs and enums with `assert_eq!` where possible. No wall-clock time, randomness or network in tests. Don't assert on absolute paths or path separators; tests run on all three OSes.
- **A bug fix starts with a failing test** that reproduces it.
