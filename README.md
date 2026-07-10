# arta

**arta** is an agent-native version control system written in Rust. It reimagines git from the ground up for a world where AI agents commit, branch, and reason about code autonomously — while remaining fully compatible with the existing git ecosystem.

> arta is not a fork of git. It is a new content-addressable store with a compatibility layer that speaks the `.git` wire format. Every arta repository is also a valid git repository — push it to GitHub and humans see an ordinary commit history.

---

## Why arta

Git was designed for humans writing commit messages and resolving merge conflicts by hand. arta is designed for agents that need to:

- Record *why* a change was made, not just what changed — intent, reasoning, and a confidence score live on every commit
- Operate on multiple branches concurrently without human intervention
- Roll back to a specific *intent* or confidence level, not just a hash
- Store tool-call traces, reasoning chains, and task hierarchies alongside diffs
- Stay interoperable with GitHub, GitLab, and every existing git tool

The agent metadata rides along in the commit body as structured JSON and is ignored by standard git tooling — so there is no lock-in.

---

## Architecture

arta is a Cargo workspace of four layered crates:

| Layer | Crate | Responsibility |
|-------|-------|----------------|
| L0 | [`arta-compat`](crates/arta-compat) | Git compatibility — reads/writes the `.git` wire format (objects, loose store, packfiles) |
| L1/L2 | [`arta-core`](crates/arta-core) | Content-addressable object store, BLAKE3 hashing, tree snapshots, agent context |
| L3 | [`arta-agent`](crates/arta-agent) | Agent layer — intent commits, task graph, checkpoints, rollback |
| L4 | [`arta-cli`](crates/arta-cli) | The `arta` binary — human and agent interfaces |

Two design choices set the foundation apart from git:

- **BLAKE3 over SHA1** for arta's native content addressing (faster, collision-resistant). The compat layer translates to git's SHA1 when talking to remotes.
- **Full snapshots over delta diffs** — arta stores content-addressed snapshots and computes diffs on read, with deduplication at the blob level.

See [`CLAUDE.md`](CLAUDE.md) for the full design rationale.

---

## Project status

Early, actively built out phase by phase.

- ✅ **Phase 1 — `arta-core` foundation**: `ContentHash` (BLAKE3), deduplicating `BlobStore`, recursive `TreeSnapshot`, `AgentContext` (intent / reasoning / confidence).
- ✅ **Phase 2 — `arta-compat` git object model**: `GitOid` (SHA1), `GitObject` (blob / tree / commit) with git's exact framing and parsing, and a `LooseObjectStore` that reads and writes zlib-compressed objects in git's `.git/objects/aa/bbbb…` layout. Object ids match git byte-for-byte — verified against `git hash-object`/`git mktree`, and real `git` reads objects arta writes (`git fsck` clean).
- ⏳ **Phase 2 (cont.)** — packfile reader/writer.
- ✅ **Phase 3 — `arta-agent` layer**: `AgentCommit` (intent-aware, parent-chained commits over `arta-core` snapshots), named `Checkpoint`s, a `TaskNode` graph (sub-tasks link to parents; commits collect on the active task), and an `AgentRepo` with a moveable `HEAD` supporting `rollback_to_intent` (exact then substring match) and `rollback_to_confidence` ("back to the last point I was sure about"). Mutable pointers persist to `.arta/state.json`; objects live in the content-addressed store. Auto-branch/merge on task open/complete lands with the branch model in a later phase.
- ✅ **Phase 4 — `arta-cli`**: the `arta` binary now drives the agent layer end to end. `init` creates a repository; `snapshot` hashes the working tree and records an intent commit; `status` and `log` (human or `--json`) read history; and `agent commit` / `checkpoint` / `rollback --to-intent|--to-confidence` / `task open|complete|list` expose the richer API. Commands discover the repository by walking up to the nearest `.arta`, and errors render as single human-readable lines with a non-zero exit code.
- ⏳ **Phase 4 (cont.)** — `push`/`pull` delegation to `arta-compat` (waits on the packfile writer).

---

## Build

Requires Rust stable (1.78+).

```bash
# Build everything
cargo build --workspace

# Run the tests
cargo test --workspace

# Build the CLI binary (target/release/arta)
cargo build -p arta-cli --release

# WASM target for the core
rustup target add wasm32-unknown-unknown
cargo build -p arta-core --target wasm32-unknown-unknown
```

---

## CLI

Human commands mirror familiar git UX; agent commands expose the richer API. Everything below works today except `push`/`pull`, which wait on the packfile writer.

```bash
# Human-facing
arta init                                    # create a repository here
arta snapshot --intent "..." [--confidence 0.9] [--reasoning "..."]
arta status                                  # HEAD, active task, counts
arta log [--show-intent] [--json]

# Agent-facing
arta agent commit --intent "..." --confidence 0.9
arta agent checkpoint --reason "before_risky_refactor"
arta agent rollback --to-intent "working auth"
arta agent rollback --to-confidence 0.8
arta agent task open "refactor auth module"
arta agent task complete <uuid>
arta agent task list
arta agent log                               # machine-readable JSON

# Not yet wired up:
# arta push / pull                           # will delegate to arta-compat
```

---

## Contributing

Conventions the codebase follows (see [`CLAUDE.md`](CLAUDE.md) for the full list):

- Public APIs carry `///` doc comments; crates set `#![warn(missing_docs)]`.
- Errors use `thiserror` and propagate with `?` — no `unwrap()` in library code.
- Unit tests live beside the code in `#[cfg(test)]` modules; use `tempfile` for fixtures.
- `cargo clippy --workspace --all-targets` stays clean.

---

## License

Licensed under either of MIT or Apache-2.0, at your option.
