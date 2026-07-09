//! The agent repository.
//!
//! [`AgentRepo`] is the entry point an agent drives: it owns a content-addressed
//! [`BlobStore`] of immutable objects (commits, checkpoints) plus a small file
//! of *mutable* pointers — the current `HEAD`, the open tasks, and the saved
//! checkpoints. This mirrors git's split between the object database and refs:
//! objects never change, pointers move.
//!
//! The distinctive operations live here:
//!
//! - [`commit`](AgentRepo::commit) chains an [`AgentCommit`] onto `HEAD`.
//! - [`checkpoint`](AgentRepo::checkpoint) drops a named rollback marker.
//! - [`rollback_to_intent`](AgentRepo::rollback_to_intent) and
//!   [`rollback_to_confidence`](AgentRepo::rollback_to_confidence) move `HEAD`
//!   back to a commit matched by *what the agent meant* or *how sure it was* —
//!   queries git cannot express.
//! - [`open_task`](AgentRepo::open_task) / [`complete_task`](AgentRepo::complete_task)
//!   track the goal a run of commits belongs to.

use std::fs;
use std::path::{Path, PathBuf};

use arta_core::{AgentContext, BlobStore, ContentHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AgentCommit, AgentError, Checkpoint, Result, TaskNode};

/// The mutable pointer state of a repository, persisted as JSON.
///
/// Everything the object store cannot hold because it changes over time: the
/// current commit, the live task registry, and the list of saved checkpoints.
#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    head: Option<ContentHash>,
    active_task: Option<Uuid>,
    tasks: Vec<TaskNode>,
    checkpoints: Vec<ContentHash>,
}

/// An agent-native repository: an object store plus moveable `HEAD` and tasks.
#[derive(Debug)]
pub struct AgentRepo {
    meta: PathBuf,
    store: BlobStore,
    state: State,
}

impl AgentRepo {
    /// Initialise a fresh repository under `path`.
    ///
    /// Creates a `.arta` metadata directory containing the object store and an
    /// empty state. If a repository already exists at `path` its state is left
    /// intact and reopened.
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let meta = path.as_ref().join(".arta");
        fs::create_dir_all(&meta).map_err(|e| io(&meta, e))?;
        let store = BlobStore::open(meta.join("objects"))?;
        let state_path = meta.join("state.json");
        let state = if state_path.exists() {
            load_state(&state_path)?
        } else {
            State::default()
        };
        let repo = AgentRepo { meta, store, state };
        repo.save()?;
        Ok(repo)
    }

    /// Open an existing repository under `path`.
    ///
    /// Returns [`AgentError::Io`] if no `.arta` state is present.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let meta = path.as_ref().join(".arta");
        let store = BlobStore::open(meta.join("objects"))?;
        let state = load_state(&meta.join("state.json"))?;
        Ok(AgentRepo { meta, store, state })
    }

    /// The underlying object store.
    pub fn store(&self) -> &BlobStore {
        &self.store
    }

    /// The current `HEAD` commit hash, or `None` if the repo has no commits.
    pub fn head(&self) -> Option<ContentHash> {
        self.state.head
    }

    /// Record a commit of `snapshot` under `context`, chained onto the current
    /// `HEAD`, and advance `HEAD` to it.
    ///
    /// If a task is open, the commit is tagged with that task's id and appended
    /// to the task's commit list.
    pub fn commit(&mut self, context: AgentContext, snapshot: ContentHash) -> Result<ContentHash> {
        let mut commit = AgentCommit::new(context, snapshot, self.state.head);
        if let Some(task_id) = self.state.active_task {
            commit.task_id = Some(task_id);
        }
        let hash = commit.store(&self.store)?;

        if let Some(task_id) = self.state.active_task {
            // The active task always exists in the registry; guard defensively.
            if let Some(task) = self.state.tasks.iter_mut().find(|t| t.id == task_id) {
                task.record_commit(hash);
            }
        }
        self.state.head = Some(hash);
        self.save()?;
        tracing::info!(commit = %hash, "recorded agent commit");
        Ok(hash)
    }

    /// Load the [`AgentCommit`] identified by `hash`.
    pub fn load_commit(&self, hash: &ContentHash) -> Result<AgentCommit> {
        AgentCommit::load(&self.store, hash)
    }

    /// The history reachable from `HEAD`, newest commit first.
    ///
    /// Each element pairs a commit's hash with the loaded commit. The walk
    /// follows `parent` links until it reaches a root commit.
    pub fn history(&self) -> Result<Vec<(ContentHash, AgentCommit)>> {
        let mut out = Vec::new();
        let mut cursor = self.state.head;
        while let Some(hash) = cursor {
            let commit = self.load_commit(&hash)?;
            cursor = commit.parent;
            out.push((hash, commit));
        }
        Ok(out)
    }

    /// Drop a named checkpoint at the current `HEAD`.
    ///
    /// Returns [`AgentError::EmptyHistory`] if there is nothing to check point.
    pub fn checkpoint(&mut self, reason: impl Into<String>) -> Result<Checkpoint> {
        let head = self.state.head.ok_or(AgentError::EmptyHistory)?;
        let commit = self.load_commit(&head)?;
        let checkpoint = Checkpoint::new(reason, head, commit.snapshot);
        let hash = checkpoint.store(&self.store)?;
        self.state.checkpoints.push(hash);
        self.save()?;
        tracing::info!(checkpoint = %checkpoint.id, "saved checkpoint");
        Ok(checkpoint)
    }

    /// All saved checkpoints, oldest first.
    pub fn checkpoints(&self) -> Result<Vec<Checkpoint>> {
        self.state
            .checkpoints
            .iter()
            .map(|h| Checkpoint::load(&self.store, h))
            .collect()
    }

    /// Move `HEAD` to `target`, which must be a stored commit.
    ///
    /// Committing after a rollback forks history at `target`.
    pub fn rollback_to(&mut self, target: ContentHash) -> Result<()> {
        // Validate the target is a loadable commit before moving HEAD.
        self.load_commit(&target)?;
        self.state.head = Some(target);
        self.save()?;
        tracing::info!(target = %target, "rolled back HEAD");
        Ok(())
    }

    /// Roll back to the most recent reachable commit whose intent matches
    /// `query`.
    ///
    /// An exact (case-insensitive) intent match anywhere in reachable history
    /// wins; failing that, the most recent commit whose intent *contains*
    /// `query` (case-insensitive) is used. Returns the commit hash moved to, or
    /// [`AgentError::NoMatch`] if nothing matches.
    pub fn rollback_to_intent(&mut self, query: &str) -> Result<ContentHash> {
        let needle = query.to_lowercase();
        let history = self.history()?;

        let exact = history
            .iter()
            .find(|(_, c)| c.intent().to_lowercase() == needle);
        let chosen = exact.or_else(|| {
            history
                .iter()
                .find(|(_, c)| c.intent().to_lowercase().contains(&needle))
        });

        match chosen {
            Some((hash, _)) => {
                let hash = *hash;
                self.rollback_to(hash)?;
                Ok(hash)
            }
            None => Err(AgentError::NoMatch(query.to_string())),
        }
    }

    /// Roll back to the most recent reachable commit whose confidence is at
    /// least `min`.
    ///
    /// This answers "go back to the last point I was sure about". Returns the
    /// commit hash moved to, or [`AgentError::NoMatch`] if no reachable commit
    /// clears the threshold.
    pub fn rollback_to_confidence(&mut self, min: f32) -> Result<ContentHash> {
        let history = self.history()?;
        match history.iter().find(|(_, c)| c.confidence() >= min) {
            Some((hash, _)) => {
                let hash = *hash;
                self.rollback_to(hash)?;
                Ok(hash)
            }
            None => Err(AgentError::NoMatch(format!("confidence >= {min}"))),
        }
    }

    /// Open a task with `description` under an optional `parent`, make it the
    /// active task, and return its id.
    ///
    /// Subsequent commits are tagged with and recorded on this task until
    /// another task is opened or it is completed. (Full auto-branching lands
    /// with the branch model in a later phase; today a task groups commits and
    /// carries intent.)
    pub fn open_task(&mut self, description: impl Into<String>, parent: Option<Uuid>) -> Result<Uuid> {
        let task = TaskNode::open(description, parent);
        let id = task.id;
        self.state.tasks.push(task);
        self.state.active_task = Some(id);
        self.save()?;
        tracing::info!(task = %id, "opened task");
        Ok(id)
    }

    /// The currently active task's id, if any.
    pub fn active_task(&self) -> Option<Uuid> {
        self.state.active_task
    }

    /// Borrow a task by id.
    pub fn task(&self, id: Uuid) -> Option<&TaskNode> {
        self.state.tasks.iter().find(|t| t.id == id)
    }

    /// All tasks in the repository, in the order they were opened.
    pub fn tasks(&self) -> &[TaskNode] {
        &self.state.tasks
    }

    /// Complete the task `id`, closing it and clearing it as the active task if
    /// it was active.
    ///
    /// Returns [`AgentError::UnknownTask`] if no such task exists.
    pub fn complete_task(&mut self, id: Uuid) -> Result<()> {
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(AgentError::UnknownTask(id))?;
        task.complete();
        if self.state.active_task == Some(id) {
            self.state.active_task = None;
        }
        self.save()?;
        tracing::info!(task = %id, "completed task");
        Ok(())
    }

    /// Persist the mutable state to disk.
    fn save(&self) -> Result<()> {
        let path = self.meta.join("state.json");
        let bytes = serde_json::to_vec_pretty(&self.state).map_err(arta_core::ArtaError::from)?;
        // Write-then-rename so a crash never leaves a half-written state file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).map_err(|e| io(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| io(&path, e))?;
        Ok(())
    }
}

/// Load and deserialize the state file at `path`.
fn load_state(path: &Path) -> Result<State> {
    let bytes = fs::read(path).map_err(|e| io(path, e))?;
    Ok(serde_json::from_slice(&bytes).map_err(arta_core::ArtaError::from)?)
}

/// Build an [`AgentError::Io`] carrying the offending path.
fn io(path: impl Into<PathBuf>, source: std::io::Error) -> AgentError {
    AgentError::Io {
        path: path.into(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> (tempfile::TempDir, AgentRepo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = AgentRepo::init(dir.path()).unwrap();
        (dir, repo)
    }

    fn commit(repo: &mut AgentRepo, intent: &str, confidence: f32) -> ContentHash {
        let ctx = AgentContext::new(intent, confidence);
        let snap = ContentHash::of(intent.as_bytes());
        repo.commit(ctx, snap).unwrap()
    }

    #[test]
    fn commits_chain_and_history_is_newest_first() {
        let (_dir, mut repo) = repo();
        assert!(repo.head().is_none());
        let c1 = commit(&mut repo, "first", 0.5);
        let c2 = commit(&mut repo, "second", 0.9);
        assert_eq!(repo.head(), Some(c2));

        let history = repo.history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, c2);
        assert_eq!(history[1].0, c1);
        assert_eq!(history[1].1.parent, None);
        assert_eq!(history[0].1.parent, Some(c1));
    }

    #[test]
    fn state_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let head = {
            let mut repo = AgentRepo::init(dir.path()).unwrap();
            commit(&mut repo, "persisted", 1.0)
        };
        let reopened = AgentRepo::open(dir.path()).unwrap();
        assert_eq!(reopened.head(), Some(head));
        assert_eq!(reopened.history().unwrap().len(), 1);
    }

    #[test]
    fn checkpoint_records_current_head() {
        let (_dir, mut repo) = repo();
        let c1 = commit(&mut repo, "work", 0.8);
        let cp = repo.checkpoint("before_refactor").unwrap();
        assert_eq!(cp.commit_at, c1);
        assert_eq!(cp.reason, "before_refactor");
        assert_eq!(repo.checkpoints().unwrap().len(), 1);
    }

    #[test]
    fn checkpoint_on_empty_history_errors() {
        let (_dir, mut repo) = repo();
        assert!(matches!(
            repo.checkpoint("nope"),
            Err(AgentError::EmptyHistory)
        ));
    }

    #[test]
    fn rollback_to_confidence_finds_last_sure_commit() {
        let (_dir, mut repo) = repo();
        let sure = commit(&mut repo, "solid", 0.95);
        let _shaky = commit(&mut repo, "guess", 0.2);
        let target = repo.rollback_to_confidence(0.9).unwrap();
        assert_eq!(target, sure);
        assert_eq!(repo.head(), Some(sure));
    }

    #[test]
    fn rollback_to_confidence_unmet_errors() {
        let (_dir, mut repo) = repo();
        commit(&mut repo, "guess", 0.3);
        assert!(matches!(
            repo.rollback_to_confidence(0.9),
            Err(AgentError::NoMatch(_))
        ));
    }

    #[test]
    fn rollback_to_intent_prefers_exact_then_substring() {
        let (_dir, mut repo) = repo();
        let auth = commit(&mut repo, "working auth", 0.7);
        let _other = commit(&mut repo, "tidy imports", 0.7);

        // Substring, case-insensitive.
        let hit = repo.rollback_to_intent("AUTH").unwrap();
        assert_eq!(hit, auth);

        // Exact match wins even when a later commit also contains the words.
        let _more = commit(&mut repo, "working auth refactor", 0.7);
        // history now: [working auth refactor, working auth]
        let exact = repo.rollback_to_intent("working auth").unwrap();
        assert_eq!(exact, auth);
    }

    #[test]
    fn rollback_to_intent_no_match_errors() {
        let (_dir, mut repo) = repo();
        commit(&mut repo, "something", 1.0);
        assert!(matches!(
            repo.rollback_to_intent("nonexistent"),
            Err(AgentError::NoMatch(_))
        ));
    }

    #[test]
    fn committing_after_rollback_forks_history() {
        let (_dir, mut repo) = repo();
        let base = commit(&mut repo, "base", 0.9);
        let _discarded = commit(&mut repo, "discarded", 0.9);
        repo.rollback_to(base).unwrap();
        let forked = commit(&mut repo, "forked", 0.9);

        let history = repo.history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, forked);
        assert_eq!(history[0].1.parent, Some(base));
        assert_eq!(history[1].0, base);
    }

    #[test]
    fn active_task_tags_and_collects_commits() {
        let (_dir, mut repo) = repo();
        let task = repo.open_task("refactor auth module", None).unwrap();
        assert_eq!(repo.active_task(), Some(task));

        let c1 = commit(&mut repo, "step one", 0.8);
        let c2 = commit(&mut repo, "step two", 0.8);

        // Both commits carry the task id and are collected on the task.
        assert_eq!(repo.load_commit(&c1).unwrap().task_id, Some(task));
        assert_eq!(repo.load_commit(&c2).unwrap().task_id, Some(task));
        assert_eq!(repo.task(task).unwrap().commits, vec![c1, c2]);

        repo.complete_task(task).unwrap();
        assert_eq!(repo.active_task(), None);
        assert_eq!(
            repo.task(task).unwrap().status,
            crate::TaskStatus::Complete
        );
    }

    #[test]
    fn sub_tasks_link_to_their_parent() {
        let (_dir, mut repo) = repo();
        let parent = repo.open_task("parent goal", None).unwrap();
        let child = repo.open_task("sub goal", Some(parent)).unwrap();
        assert_eq!(repo.task(child).unwrap().parent_task, Some(parent));
        // Opening the child made it the active task.
        assert_eq!(repo.active_task(), Some(child));
    }

    #[test]
    fn completing_unknown_task_errors() {
        let (_dir, mut repo) = repo();
        assert!(matches!(
            repo.complete_task(Uuid::from_u128(1)),
            Err(AgentError::UnknownTask(_))
        ));
    }

    #[test]
    fn commit_without_active_task_has_no_task_id() {
        let (_dir, mut repo) = repo();
        let c = commit(&mut repo, "loose commit", 1.0);
        assert_eq!(repo.load_commit(&c).unwrap().task_id, None);
    }
}
