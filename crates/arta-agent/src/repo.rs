//! The agent-facing repository.
//!
//! [`AgentRepo`] ties the object store together with the mutable state an agent
//! reasons about: the current commit (`HEAD`), the task graph, and saved
//! checkpoints. It is the entry point for the APIs that make arta different
//! from git — committing with intent, checkpointing, and rolling back by intent
//! or confidence rather than by hash.
//!
//! A repository is just a directory. Immutable objects (commits, contexts,
//! trees, blobs) live in a content-addressed [`BlobStore`]; mutable metadata
//! (`HEAD`, the active task, and the task/checkpoint records) live in small
//! files beside it. Nothing here is global: opening the same directory twice
//! yields two handles onto the same state.

use std::fs;
use std::path::PathBuf;

use arta_core::{AgentContext, BlobStore, ContentHash};
use uuid::Uuid;

use crate::checkpoint::Checkpoint;
use crate::commit::AgentCommit;
use crate::error::{AgentError, Result};
use crate::task::{TaskNode, TaskStatus};

/// A handle to an on-disk arta agent repository.
#[derive(Debug, Clone)]
pub struct AgentRepo {
    root: PathBuf,
    store: BlobStore,
}

impl AgentRepo {
    /// Open (creating if necessary) an agent repository rooted at `root`.
    ///
    /// The object store and metadata directories are created if absent, so this
    /// doubles as initialization.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let store = BlobStore::open(root.join("objects"))?;
        for sub in ["tasks", "checkpoints"] {
            let dir = root.join(sub);
            fs::create_dir_all(&dir).map_err(|e| AgentError::io(&dir, e))?;
        }
        Ok(AgentRepo { root, store })
    }

    /// The underlying content-addressed object store.
    pub fn store(&self) -> &BlobStore {
        &self.store
    }

    // --- HEAD -------------------------------------------------------------

    fn head_path(&self) -> PathBuf {
        self.root.join("HEAD")
    }

    /// The commit `HEAD` currently points at, or `None` if there are no commits.
    pub fn head(&self) -> Result<Option<ContentHash>> {
        let path = self.head_path();
        match fs::read_to_string(&path) {
            Ok(s) => {
                let s = s.trim();
                if s.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(ContentHash::from_hex(s)?))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AgentError::io(&path, e)),
        }
    }

    fn set_head(&self, hash: &ContentHash) -> Result<()> {
        let path = self.head_path();
        fs::write(&path, hash.to_hex()).map_err(|e| AgentError::io(&path, e))
    }

    // --- active task ------------------------------------------------------

    fn active_task_path(&self) -> PathBuf {
        self.root.join("active_task")
    }

    /// The id of the task commits are currently attributed to, if any.
    pub fn active_task(&self) -> Result<Option<Uuid>> {
        let path = self.active_task_path();
        match fs::read_to_string(&path) {
            Ok(s) => {
                let s = s.trim();
                if s.is_empty() {
                    Ok(None)
                } else {
                    Uuid::parse_str(s)
                        .map(Some)
                        // A malformed marker means "no active task" rather than
                        // a hard failure; the task graph itself is unaffected.
                        .or(Ok(None))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AgentError::io(&path, e)),
        }
    }

    fn set_active_task(&self, id: Option<Uuid>) -> Result<()> {
        let path = self.active_task_path();
        let body = id.map(|i| i.to_string()).unwrap_or_default();
        fs::write(&path, body).map_err(|e| AgentError::io(&path, e))
    }

    // --- commits ----------------------------------------------------------

    /// Record a commit for `snapshot` with the given agent `context`.
    ///
    /// The new commit's parent is the current `HEAD`, and `HEAD` advances to
    /// the new commit. If a task is active, the commit is attributed to it and
    /// appended to that task's commit list.
    ///
    /// `snapshot` is a tree root hash produced by `arta-core`; this layer does
    /// not itself walk the working tree.
    pub fn commit(&self, snapshot: ContentHash, context: AgentContext) -> Result<ContentHash> {
        let parent = self.head()?;
        let task_id = self.active_task()?;
        let commit = AgentCommit::new(snapshot, parent, task_id, context);
        let hash = commit.store(&self.store)?;
        self.set_head(&hash)?;

        if let Some(id) = task_id {
            // Attribute the commit to the active task. A missing task record at
            // this point would be a corrupted marker; surface it.
            let mut task = self.load_task(id)?;
            task.record_commit(hash);
            self.save_task(&task)?;
        }
        Ok(hash)
    }

    /// The commit history reachable from `HEAD`, most recent first.
    ///
    /// Each element pairs a commit's content hash with the commit itself.
    /// Because commits are content-addressed over their parent link, the chain
    /// is acyclic and terminates at the root commit.
    pub fn history(&self) -> Result<Vec<(ContentHash, AgentCommit)>> {
        let mut out = Vec::new();
        let mut cursor = self.head()?;
        while let Some(hash) = cursor {
            let commit = AgentCommit::load(&self.store, &hash)?;
            cursor = commit.parent;
            out.push((hash, commit));
        }
        Ok(out)
    }

    // --- checkpoints ------------------------------------------------------

    fn checkpoint_path(&self, id: Uuid) -> PathBuf {
        self.root.join("checkpoints").join(format!("{id}.json"))
    }

    /// Save a named rollback point at the current `HEAD`.
    ///
    /// Returns [`AgentError::EmptyHistory`] if the repository has no commits to
    /// checkpoint yet.
    pub fn checkpoint(&self, reason: impl Into<String>) -> Result<Checkpoint> {
        let head = self.head()?.ok_or(AgentError::EmptyHistory)?;
        let commit = AgentCommit::load(&self.store, &head)?;
        let cp = Checkpoint::new(commit.snapshot, head, reason);
        let path = self.checkpoint_path(cp.id);
        let bytes = serde_json::to_vec_pretty(&cp).map_err(arta_core::ArtaError::from)?;
        fs::write(&path, bytes).map_err(|e| AgentError::io(&path, e))?;
        Ok(cp)
    }

    /// All saved checkpoints, in no particular order.
    pub fn checkpoints(&self) -> Result<Vec<Checkpoint>> {
        let dir = self.root.join("checkpoints");
        let mut out = Vec::new();
        let read_dir = fs::read_dir(&dir).map_err(|e| AgentError::io(&dir, e))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| AgentError::io(&dir, e))?;
            let bytes = fs::read(entry.path()).map_err(|e| AgentError::io(entry.path(), e))?;
            let cp: Checkpoint =
                serde_json::from_slice(&bytes).map_err(arta_core::ArtaError::from)?;
            out.push(cp);
        }
        Ok(out)
    }

    // --- rollback ---------------------------------------------------------

    /// Move `HEAD` back to the most recent commit whose intent matches `query`.
    ///
    /// Matching is exact first: if any commit's intent equals `query`, the most
    /// recent such commit is chosen. Otherwise a case-insensitive substring
    /// match is used as a light fuzzy fallback (richer similarity search is
    /// Phase 5). Returns the target commit's hash, or [`AgentError::NoMatch`] if
    /// nothing matches.
    ///
    /// Rollback only repoints `HEAD`; materializing the target snapshot into the
    /// working tree is the caller's responsibility.
    pub fn rollback_to_intent(&self, query: &str) -> Result<ContentHash> {
        let history = self.history()?;
        let exact = history.iter().find(|(_, c)| c.intent == query);
        if let Some((hash, _)) = exact {
            self.set_head(hash)?;
            return Ok(*hash);
        }
        let needle = query.to_lowercase();
        let fuzzy = history
            .iter()
            .find(|(_, c)| c.intent.to_lowercase().contains(&needle));
        match fuzzy {
            Some((hash, _)) => {
                self.set_head(hash)?;
                Ok(*hash)
            }
            None => Err(AgentError::NoMatch(query.to_string())),
        }
    }

    /// Move `HEAD` back to the most recent commit with confidence `>= min`.
    ///
    /// This answers "go back to the last point I was sure about". Returns the
    /// target commit's hash, or [`AgentError::NoMatch`] if no commit in history
    /// clears the threshold.
    pub fn rollback_to_confidence(&self, min: f32) -> Result<ContentHash> {
        let history = self.history()?;
        match history.iter().find(|(_, c)| c.confidence >= min) {
            Some((hash, _)) => {
                self.set_head(hash)?;
                Ok(*hash)
            }
            None => Err(AgentError::NoMatch(format!("confidence >= {min}"))),
        }
    }

    // --- task graph -------------------------------------------------------

    fn task_path(&self, id: Uuid) -> PathBuf {
        self.root.join("tasks").join(format!("{id}.json"))
    }

    fn save_task(&self, task: &TaskNode) -> Result<()> {
        let path = self.task_path(task.id);
        let bytes = serde_json::to_vec_pretty(task).map_err(arta_core::ArtaError::from)?;
        fs::write(&path, bytes).map_err(|e| AgentError::io(&path, e))
    }

    fn load_task(&self, id: Uuid) -> Result<TaskNode> {
        let path = self.task_path(id);
        match fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).map_err(arta_core::ArtaError::from)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AgentError::NoSuchTask(id)),
            Err(e) => Err(AgentError::io(&path, e)),
        }
    }

    /// Look up a task by id.
    pub fn task(&self, id: Uuid) -> Result<TaskNode> {
        self.load_task(id)
    }

    /// Open a new task and make it the active one.
    ///
    /// Commits made after this call are attributed to the task until it is
    /// completed or abandoned — arta's stand-in for git's branch-per-goal, kept
    /// at the task level so the graph records why work was grouped. Returns the
    /// new task's id.
    pub fn open_task(
        &self,
        description: impl Into<String>,
        parent: Option<Uuid>,
    ) -> Result<Uuid> {
        // Validate the parent exists so the graph never dangles.
        if let Some(pid) = parent {
            self.load_task(pid)?;
        }
        let task = TaskNode::open(description, parent);
        let id = task.id;
        self.save_task(&task)?;
        self.set_active_task(Some(id))?;
        Ok(id)
    }

    /// Close a task, marking it [`Complete`](TaskStatus::Complete).
    ///
    /// Because commits are recorded linearly onto `HEAD`, completing a task is
    /// the merge: its commits are already on the main history. If the completed
    /// task was the active one, no task is active afterward.
    pub fn complete_task(&self, id: Uuid) -> Result<()> {
        self.finish_task(id, TaskStatus::Complete)
    }

    /// Close a task, marking it [`Abandoned`](TaskStatus::Abandoned).
    ///
    /// Its commits are kept for history but the task is recorded as given up on.
    pub fn abandon_task(&self, id: Uuid) -> Result<()> {
        self.finish_task(id, TaskStatus::Abandoned)
    }

    fn finish_task(&self, id: Uuid, status: TaskStatus) -> Result<()> {
        let mut task = self.load_task(id)?;
        task.status = status;
        self.save_task(&task)?;
        if self.active_task()? == Some(id) {
            self.set_active_task(None)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> (tempfile::TempDir, AgentRepo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = AgentRepo::open(dir.path().join(".arta")).unwrap();
        (dir, repo)
    }

    fn snap(tag: &[u8]) -> ContentHash {
        ContentHash::of(tag)
    }

    #[test]
    fn head_starts_empty_then_advances() {
        let (_d, repo) = repo();
        assert_eq!(repo.head().unwrap(), None);
        let h = repo
            .commit(snap(b"t1"), AgentContext::new("first", 1.0))
            .unwrap();
        assert_eq!(repo.head().unwrap(), Some(h));
    }

    #[test]
    fn commits_chain_through_parents() {
        let (_d, repo) = repo();
        let a = repo
            .commit(snap(b"a"), AgentContext::new("a", 0.9))
            .unwrap();
        let b = repo
            .commit(snap(b"b"), AgentContext::new("b", 0.9))
            .unwrap();
        let history = repo.history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, b, "most recent first");
        assert_eq!(history[0].1.parent, Some(a));
        assert_eq!(history[1].1.parent, None);
    }

    #[test]
    fn checkpoint_requires_a_commit() {
        let (_d, repo) = repo();
        assert!(matches!(
            repo.checkpoint("nope").unwrap_err(),
            AgentError::EmptyHistory
        ));
        repo.commit(snap(b"t"), AgentContext::new("t", 1.0))
            .unwrap();
        let cp = repo.checkpoint("before_refactor").unwrap();
        assert_eq!(cp.reason, "before_refactor");
        assert_eq!(repo.checkpoints().unwrap().len(), 1);
    }

    #[test]
    fn rollback_to_intent_exact_beats_substring() {
        let (_d, repo) = repo();
        repo.commit(snap(b"1"), AgentContext::new("working auth flow", 0.8))
            .unwrap();
        let exact = repo
            .commit(snap(b"2"), AgentContext::new("working auth", 0.8))
            .unwrap();
        repo.commit(snap(b"3"), AgentContext::new("broke auth", 0.3))
            .unwrap();
        let target = repo.rollback_to_intent("working auth").unwrap();
        assert_eq!(target, exact, "exact match wins over the substring match");
        assert_eq!(repo.head().unwrap(), Some(exact));
    }

    #[test]
    fn rollback_to_intent_falls_back_to_substring() {
        let (_d, repo) = repo();
        let base = repo
            .commit(snap(b"1"), AgentContext::new("stabilize the parser", 0.9))
            .unwrap();
        repo.commit(snap(b"2"), AgentContext::new("experiment", 0.2))
            .unwrap();
        let target = repo.rollback_to_intent("PARSER").unwrap();
        assert_eq!(target, base);
    }

    #[test]
    fn rollback_to_intent_reports_no_match() {
        let (_d, repo) = repo();
        repo.commit(snap(b"1"), AgentContext::new("only intent", 1.0))
            .unwrap();
        assert!(matches!(
            repo.rollback_to_intent("absent").unwrap_err(),
            AgentError::NoMatch(_)
        ));
    }

    #[test]
    fn rollback_to_confidence_picks_most_recent_above_threshold() {
        let (_d, repo) = repo();
        let sure = repo
            .commit(snap(b"1"), AgentContext::new("sure thing", 0.95))
            .unwrap();
        repo.commit(snap(b"2"), AgentContext::new("a guess", 0.4))
            .unwrap();
        repo.commit(snap(b"3"), AgentContext::new("another guess", 0.5))
            .unwrap();
        let target = repo.rollback_to_confidence(0.9).unwrap();
        assert_eq!(target, sure);
        assert_eq!(repo.head().unwrap(), Some(sure));
    }

    #[test]
    fn active_task_attributes_commits() {
        let (_d, repo) = repo();
        let task = repo.open_task("refactor auth", None).unwrap();
        assert_eq!(repo.active_task().unwrap(), Some(task));
        let c = repo
            .commit(snap(b"1"), AgentContext::new("split module", 0.8))
            .unwrap();
        let loaded = repo.task(task).unwrap();
        assert_eq!(loaded.commits, vec![c]);
        assert_eq!(loaded.status, TaskStatus::Active);
        // The commit itself records the task id.
        assert_eq!(repo.history().unwrap()[0].1.task_id, Some(task));
    }

    #[test]
    fn completing_the_active_task_clears_it() {
        let (_d, repo) = repo();
        let task = repo.open_task("t", None).unwrap();
        repo.complete_task(task).unwrap();
        assert_eq!(repo.active_task().unwrap(), None);
        assert_eq!(repo.task(task).unwrap().status, TaskStatus::Complete);
    }

    #[test]
    fn sub_task_requires_an_existing_parent() {
        let (_d, repo) = repo();
        let missing = Uuid::new_v4();
        assert!(matches!(
            repo.open_task("child", Some(missing)).unwrap_err(),
            AgentError::NoSuchTask(_)
        ));
        let parent = repo.open_task("parent", None).unwrap();
        let child = repo.open_task("child", Some(parent)).unwrap();
        assert_eq!(repo.task(child).unwrap().parent_task, Some(parent));
    }

    #[test]
    fn state_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".arta");
        let head = {
            let repo = AgentRepo::open(&path).unwrap();
            repo.commit(snap(b"persist"), AgentContext::new("persist me", 1.0))
                .unwrap()
        };
        // A fresh handle onto the same directory sees the same HEAD.
        let reopened = AgentRepo::open(&path).unwrap();
        assert_eq!(reopened.head().unwrap(), Some(head));
        assert_eq!(reopened.history().unwrap().len(), 1);
    }
}
