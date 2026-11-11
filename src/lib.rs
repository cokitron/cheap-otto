//! `cheap-otto` library crate: Cross-OS Dev Bridge, Parallel Agent Worktree Farm,
//! multi-harness control plane (`claude`, `agy`, `kiro`, `custom`), addressable per-agent
//! sessions (`inspect`/`ask`), repeatable task templates, `pixel-agents` visual office bridge,
//! reversible macOS clamshell power guardrails, and unified memory/thermal arbitration.

#![forbid(unsafe_code)]

pub mod agent_session;
pub mod arbiter;
pub mod bridge;
pub mod command;
pub mod doctor;
pub mod hardware;
pub mod harness;
pub mod macos;
pub mod schedule;
pub mod server;
pub mod template;
pub mod terminal;
pub mod ui_assets;
pub mod workspace;
pub mod worktree;
