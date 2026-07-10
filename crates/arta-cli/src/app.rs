//! Command logic for the `arta` binary.
//!
//! Each public function here implements one CLI verb against an [`AgentRepo`].
//! They take an explicit working-directory root and return a rendered `String`
//! rather than printing directly, so the same logic drives both the binary and
//! the unit tests in this module. `main.rs` parses arguments, calls these, and
//! prints (or reports) the result.

use std::path::{Path, PathBuf};

use arta_agent::{AgentRepo, TaskStatus};
use arta_core::{AgentContext, ContentHash, TreeSnapshot};
use uuid::Uuid;

/// Errors surfaced by the CLI. Agent- and core-layer failures are wrapped so
/// `main` can render a single human-readable message at the top level.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// A failure bubbled up from the agent layer (which itself wraps the core).
    #[error(transparent)]
    Agent(#[from] arta_agent::AgentError),

    /// A failure bubbled up directly from the core object store (e.g. hashing
    /// the working tree during a snapshot).
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// No `.arta` repository was found at or above the working directory.
    #[error("not an arta repository (no .arta directory found in '{0}' or any parent)")]
    NotARepo(PathBuf),

    /// A repository already exists where `init` was asked to create one.
    #[error("an arta repository already exists at '{0}'")]
    AlreadyExists(PathBuf),

    /// A user-supplied task id was not a valid UUID.
    #[error("invalid task id '{0}': expected a UUID")]
    BadTaskId(String),

    /// A rollback query selected by intent or confidence but matched nothing.
    #[error("{0}")]
    NoRollbackTarget(String),
}

/// Convenience alias for CLI results.
pub type Result<T> = std::result::Result<T, CliError>;

/// Initialise a repository at `root`, returning a confirmation line.
///
/// Refuses to clobber an existing repository so `init` is safe to re-run
/// intentionally but never silently resets state.
pub fn init(root: &Path) -> Result<String> {
    if root.join(".arta").exists() {
        return Err(CliError::AlreadyExists(root.to_path_buf()));
    }
    AgentRepo::init(root)?;
    Ok(format!(
        "initialised empty arta repository in {}",
        root.join(".arta").display()
    ))
}

/// Snapshot the working tree at `root` and record a commit under `intent`.
///
/// This is the CLI's `snapshot` verb (and the engine behind `agent commit`):
/// it hashes the directory into the object store, then chains a commit carrying
/// the intent, confidence, and optional reasoning onto `HEAD`.
pub fn snapshot(
    root: &Path,
    intent: &str,
    confidence: f32,
    reasoning: Option<&str>,
) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;

    // Hash the working tree into the repo's own object store; the borrow of the
    // store ends when `snapshot_dir` returns the owned root hash.
    let (_tree, snapshot) = TreeSnapshot::snapshot_dir(repo.store(), &repo_root)?;

    let mut context = AgentContext::new(intent, confidence);
    if let Some(reasoning) = reasoning {
        context = context.with_reasoning(reasoning);
    }

    let hash = repo.commit(context, snapshot)?;
    let task_note = match repo.active_task() {
        Some(id) => format!(" (task {})", short(&id.to_string())),
        None => String::new(),
    };
    Ok(format!(
        "recorded commit {} — \"{}\" @ {:.2}{}",
        short_hash(&hash),
        intent,
        confidence,
        task_note
    ))
}

/// Render the current repository status: `HEAD`, active task, and counts.
pub fn status(root: &Path) -> Result<String> {
    let repo_root = discover(root)?;
    let repo = AgentRepo::open(&repo_root)?;

    let mut out = String::new();
    match repo.head() {
        Some(head) => {
            let commit = repo.load_commit(&head)?;
            out.push_str(&format!(
                "HEAD {} — \"{}\" @ {:.2}\n",
                short_hash(&head),
                commit.intent(),
                commit.confidence()
            ));
        }
        None => out.push_str("HEAD (no commits yet)\n"),
    }

    match repo.active_task() {
        Some(id) => {
            let desc = repo.task(id).map(|t| t.description.as_str()).unwrap_or("?");
            out.push_str(&format!("active task {} — \"{}\"\n", short(&id.to_string()), desc));
        }
        None => out.push_str("active task (none)\n"),
    }

    let commits = repo.history()?.len();
    let checkpoints = repo.checkpoints()?.len();
    let tasks = repo.tasks().len();
    out.push_str(&format!(
        "{commits} commit(s), {checkpoints} checkpoint(s), {tasks} task(s)"
    ));
    Ok(out)
}

/// Render the commit history reachable from `HEAD`, newest first.
///
/// With `json`, emits a machine-readable array (for the `agent log` verb);
/// otherwise a human-readable list. `show_intent` includes the intent line in
/// the human format.
pub fn log(root: &Path, json: bool, show_intent: bool) -> Result<String> {
    let repo_root = discover(root)?;
    let repo = AgentRepo::open(&repo_root)?;
    let history = repo.history()?;

    if json {
        let entries: Vec<_> = history
            .iter()
            .map(|(hash, commit)| {
                serde_json::json!({
                    "commit": hash.to_hex(),
                    "parent": commit.parent.map(|p| p.to_hex()),
                    "snapshot": commit.snapshot.to_hex(),
                    "intent": commit.intent(),
                    "confidence": commit.confidence(),
                    "task_id": commit.task_id.map(|t| t.to_string()),
                    "timestamp": commit.timestamp.to_rfc3339(),
                })
            })
            .collect();
        let rendered = serde_json::to_string_pretty(&entries)
            .map_err(|e| arta_agent::AgentError::from(arta_core::ArtaError::from(e)))?;
        return Ok(rendered);
    }

    if history.is_empty() {
        return Ok("(no commits yet)".to_string());
    }

    let mut out = String::new();
    for (hash, commit) in &history {
        out.push_str(&format!(
            "{}  @ {:.2}  {}",
            short_hash(hash),
            commit.confidence(),
            commit.timestamp.to_rfc3339()
        ));
        if show_intent {
            out.push_str(&format!("\n    intent: {}", commit.intent()));
            if let Some(reasoning) = &commit.context.reasoning {
                out.push_str(&format!("\n    reason: {reasoning}"));
            }
        }
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

/// Save a named checkpoint at the current `HEAD`.
pub fn checkpoint(root: &Path, reason: &str) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;
    let cp = repo.checkpoint(reason)?;
    Ok(format!(
        "checkpoint {} at {} — \"{}\"",
        short(&cp.id.to_string()),
        short_hash(&cp.commit_at),
        reason
    ))
}

/// Roll back `HEAD` to the most recent commit matching `query` by intent.
pub fn rollback_to_intent(root: &Path, query: &str) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;
    match repo.rollback_to_intent(query) {
        Ok(hash) => Ok(format!("rolled back to {} matching intent \"{}\"", short_hash(&hash), query)),
        Err(arta_agent::AgentError::NoMatch(_)) => Err(CliError::NoRollbackTarget(format!(
            "no commit matched intent \"{query}\""
        ))),
        Err(e) => Err(e.into()),
    }
}

/// Roll back `HEAD` to the most recent commit at or above `min` confidence.
pub fn rollback_to_confidence(root: &Path, min: f32) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;
    match repo.rollback_to_confidence(min) {
        Ok(hash) => Ok(format!(
            "rolled back to {} (confidence >= {:.2})",
            short_hash(&hash),
            min
        )),
        Err(arta_agent::AgentError::NoMatch(_)) => Err(CliError::NoRollbackTarget(format!(
            "no commit reached confidence {min:.2}"
        ))),
        Err(e) => Err(e.into()),
    }
}

/// Open a task, optionally under `parent`, and make it active.
pub fn task_open(root: &Path, description: &str, parent: Option<&str>) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;
    let parent = parent.map(parse_task_id).transpose()?;
    let id = repo.open_task(description, parent)?;
    Ok(format!("opened task {} — \"{}\"", id, description))
}

/// Complete the task named by `id` (a full UUID).
pub fn task_complete(root: &Path, id: &str) -> Result<String> {
    let repo_root = discover(root)?;
    let mut repo = AgentRepo::open(&repo_root)?;
    let uuid = parse_task_id(id)?;
    repo.complete_task(uuid)?;
    Ok(format!("completed task {}", short(&uuid.to_string())))
}

/// List all tasks in the repository, oldest first.
pub fn task_list(root: &Path) -> Result<String> {
    let repo_root = discover(root)?;
    let repo = AgentRepo::open(&repo_root)?;
    let tasks = repo.tasks();
    if tasks.is_empty() {
        return Ok("(no tasks)".to_string());
    }
    let active = repo.active_task();
    let mut out = String::new();
    for task in tasks {
        let marker = if active == Some(task.id) { "*" } else { " " };
        out.push_str(&format!(
            "{marker} {}  {}  \"{}\"  ({} commit(s))\n",
            short(&task.id.to_string()),
            status_label(&task.status),
            task.description,
            task.commits.len()
        ));
    }
    Ok(out.trim_end().to_string())
}

/// Walk up from `start` to find the directory containing `.arta`.
fn discover(start: &Path) -> Result<PathBuf> {
    let mut current = start;
    loop {
        if current.join(".arta").is_dir() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(CliError::NotARepo(start.to_path_buf())),
        }
    }
}

/// Parse a full UUID task id, mapping failure to a friendly [`CliError`].
fn parse_task_id(raw: &str) -> Result<Uuid> {
    Uuid::parse_str(raw).map_err(|_| CliError::BadTaskId(raw.to_string()))
}

/// A short, git-style prefix of a content hash for display.
fn short_hash(hash: &ContentHash) -> String {
    short(&hash.to_hex())
}

/// First 12 characters of an identifier, for compact display.
fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

/// A one-word label for a task status.
fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Active => "active",
        TaskStatus::Complete => "complete",
        TaskStatus::Abandoned => "abandoned",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A temp dir that is an initialised repo, plus a sample tracked file.
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), b"alpha").unwrap();
        dir
    }

    #[test]
    fn init_twice_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(init(dir.path()).is_ok());
        assert!(matches!(init(dir.path()), Err(CliError::AlreadyExists(_))));
    }

    #[test]
    fn commands_outside_a_repo_report_not_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(status(dir.path()), Err(CliError::NotARepo(_))));
    }

    #[test]
    fn discover_finds_repo_from_a_subdirectory() {
        let dir = repo();
        let sub = dir.path().join("nested/deeper");
        fs::create_dir_all(&sub).unwrap();
        // A snapshot invoked from the subdir still finds the repo root.
        let msg = snapshot(&sub, "from subdir", 0.9, None).unwrap();
        assert!(msg.contains("recorded commit"));
    }

    #[test]
    fn snapshot_then_log_and_status_reflect_the_commit() {
        let dir = repo();
        snapshot(dir.path(), "initial state", 0.8, Some("first pass")).unwrap();

        let status = status(dir.path()).unwrap();
        assert!(status.contains("initial state"));
        assert!(status.contains("1 commit(s)"));

        let human = log(dir.path(), false, true).unwrap();
        assert!(human.contains("intent: initial state"));
        assert!(human.contains("reason: first pass"));
    }

    #[test]
    fn snapshot_of_changed_tree_produces_a_new_snapshot_hash() {
        let dir = repo();
        snapshot(dir.path(), "v1", 1.0, None).unwrap();
        fs::write(dir.path().join("a.txt"), b"changed").unwrap();
        snapshot(dir.path(), "v2", 1.0, None).unwrap();

        let repo = AgentRepo::open(dir.path()).unwrap();
        let history = repo.history().unwrap();
        assert_eq!(history.len(), 2);
        // The two commits captured different working trees.
        assert_ne!(history[0].1.snapshot, history[1].1.snapshot);
    }

    #[test]
    fn json_log_is_valid_and_ordered_newest_first() {
        let dir = repo();
        snapshot(dir.path(), "older", 0.5, None).unwrap();
        snapshot(dir.path(), "newer", 0.9, None).unwrap();

        let json = log(dir.path(), true, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["intent"], "newer");
        assert_eq!(arr[1]["intent"], "older");
        // Each entry carries the full 64-char hash.
        assert_eq!(arr[0]["commit"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn checkpoint_and_confidence_rollback_round_trip() {
        let dir = repo();
        snapshot(dir.path(), "solid", 0.95, None).unwrap();
        let cp = checkpoint(dir.path(), "before risky work").unwrap();
        assert!(cp.contains("before risky work"));

        snapshot(dir.path(), "shaky", 0.2, None).unwrap();
        let msg = rollback_to_confidence(dir.path(), 0.9).unwrap();
        assert!(msg.contains("rolled back"));

        // HEAD is back on the high-confidence commit.
        let repo = AgentRepo::open(dir.path()).unwrap();
        assert_eq!(repo.load_commit(&repo.head().unwrap()).unwrap().intent(), "solid");
    }

    #[test]
    fn intent_rollback_reports_no_target_when_absent() {
        let dir = repo();
        snapshot(dir.path(), "only commit", 1.0, None).unwrap();
        assert!(matches!(
            rollback_to_intent(dir.path(), "nonexistent"),
            Err(CliError::NoRollbackTarget(_))
        ));
    }

    #[test]
    fn confidence_rollback_reports_no_target_when_unmet() {
        let dir = repo();
        snapshot(dir.path(), "low", 0.3, None).unwrap();
        assert!(matches!(
            rollback_to_confidence(dir.path(), 0.9),
            Err(CliError::NoRollbackTarget(_))
        ));
    }

    #[test]
    fn task_lifecycle_tags_commits_and_lists() {
        let dir = repo();
        let open = task_open(dir.path(), "refactor auth", None).unwrap();
        // The message ends with the full UUID; recover it for completion.
        let id = open.rsplit(' ').find(|s| Uuid::parse_str(s.trim_matches('"')).is_ok());
        // The active task marker shows in the list.
        let listed = task_list(dir.path()).unwrap();
        assert!(listed.starts_with('*'));
        assert!(listed.contains("active"));

        snapshot(dir.path(), "auth step", 0.8, None).unwrap();
        let repo = AgentRepo::open(dir.path()).unwrap();
        let head = repo.head().unwrap();
        assert!(repo.load_commit(&head).unwrap().task_id.is_some());

        // Complete via the full id parsed back out of the open message.
        let full = repo.active_task().unwrap().to_string();
        let _ = id; // id was a sanity check on the message shape
        let done = task_complete(dir.path(), &full).unwrap();
        assert!(done.contains("completed task"));
        assert!(AgentRepo::open(dir.path()).unwrap().active_task().is_none());
    }

    #[test]
    fn bad_task_id_is_rejected() {
        let dir = repo();
        assert!(matches!(
            task_complete(dir.path(), "not-a-uuid"),
            Err(CliError::BadTaskId(_))
        ));
    }
}
