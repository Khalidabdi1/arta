//! # arta (L4 CLI)
//!
//! The `arta` binary. Wraps the human-facing workflow and the agent API.
//!
//! Command handling is not yet implemented — see Phase 4 in `CLAUDE.md`. For
//! now the binary prints its version so the workspace has a runnable entry
//! point while the lower layers land.

fn main() {
    println!("arta {} — agent-native version control", env!("CARGO_PKG_VERSION"));
    println!("core object store online; CLI commands land in Phase 4 (see CLAUDE.md)");
}
