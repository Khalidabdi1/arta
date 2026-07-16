//! The agent-facing repository.
//!
//! [`AgentRepo`] is the entry point for the agent API described in
//! `CLAUDE.md`. It owns a content-addressed [`BlobStore`] plus a small amount of
//! mutable state — the current commit (`HEAD`), the checkpoint list, and the
//! task graph — persisted as JSON under an `.arta` directory so a repository
//! survives across process runs.
//!
//! History is a chain of [`AgentCommit`]s linked by `parent`; the higher-level
//! rollback queries (`rollback_to_intent`, `rollback_to_confidence`) walk that
//! chain from `HEAD` and move `HEAD` to the matching commit.

use std::fs;
use std::path::{Path, PathBuf};

use arta_core::{AgentContext, BlobStore, ContentHash};
use uuid::Uuid;

use crate::checkpoint::Checkpoint;
use crate::commit::AgentCommit;
use crate::error::{AgentError, Result};
use crate::intent::match_strength;
use crate::task::TaskNode;

/// The name of the metadata directory inside a repository root.
const META_DIR: &str = ".arta";

/// An agent-native repository over a content-addressed object store.
#[derive(Debug)]
pub struct AgentRepo {
    store: BlobStore,
    meta: PathBuf,
    head: Option<ContentHash>,
    checkpoints: Vec<Checkpoint>,
    tasks: Vec<TaskNode>,
    active_task: Option<Uuid>,
}

impl AgentRepo {
    /// Open (creating if necessary) a repository rooted at `root`.
    ///
    /// Metadata lives under `root/.arta`; the object store under
    /// `root/.arta/objects`. Re-opening an existing repository restores its
    /// `HEAD`, checkpoints, and task graph.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let meta = root.as_ref().join(META_DIR);
        fs::create_dir_all(&meta).map_err(|e| AgentError::io(&meta, e))?;
        let store = BlobStore::open(meta.join("objects"))?;

        let head = match Self::read_string(&meta, "HEAD")? {
            Some(hex) if !hex.trim().is_empty() => Some(ContentHash::from_hex(hex.trim())?),
            _ => None,
        };
        let checkpoints = Self::read_json(&meta, "checkpoints.json")?.unwrap_or_default();
        let tasks = Self::read_json(&meta, "tasks.json")?.unwrap_or_default();
        let active_task = match Self::read_string(&meta, "active_task")? {
            Some(s) if !s.trim().is_empty() => Some(
                Uuid::parse_str(s.trim())
                    .map_err(|_| AgentError::NoMatch(format!("invalid active task id: {s}"))),
            )
            .transpose()?,
            _ => None,
        };

        Ok(AgentRepo {
            store,
            meta,
            head,
            checkpoints,
            tasks,
            active_task,
        })
    }

    /// The underlying object store, for callers that need to write snapshots or
    /// blobs directly (e.g. the CLI hashing a working tree).
    pub fn store(&self) -> &BlobStore {
        &self.store
    }

    /// The current commit hash, or `None` if the repository has no commits.
    pub fn head(&self) -> Option<ContentHash> {
        self.head
    }

    /// Create an intent-aware commit over `snapshot`, advancing `HEAD`.
    ///
    /// The new commit's parent is the current `HEAD`. If a task is currently
    /// active the commit is recorded against it. Returns the new commit's hash.
    pub fn commit(&mut self, context: AgentContext, snapshot: ContentHash) -> Result<ContentHash> {
        let commit = AgentCommit::new(snapshot, self.head, context, self.active_task);
        let hash = commit.store(&self.store)?;
        self.head = Some(hash);
        self.write_head()?;

        if let Some(task_id) = self.active_task {
            if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                task.record_commit(hash);
                self.write_tasks()?;
            }
        }
        tracing::info!(commit = %hash, "recorded agent commit");
        Ok(hash)
    }

    /// Load a commit by hash.
    pub fn load_commit(&self, hash: &ContentHash) -> Result<AgentCommit> {
        AgentCommit::load(&self.store, hash)
    }

    /// Walk history from `HEAD` back to the root, newest commit first.
    ///
    /// Each element is the commit's hash paired with the loaded commit.
    pub fn history(&self) -> Result<Vec<(ContentHash, AgentCommit)>> {
        let mut out = Vec::new();
        let mut cursor = self.head;
        while let Some(hash) = cursor {
            let commit = self.load_commit(&hash)?;
            cursor = commit.parent;
            out.push((hash, commit));
        }
        Ok(out)
    }

    /// Save a named checkpoint pinning the current commit and its snapshot.
    ///
    /// Fails with [`AgentError::Empty`] if there is nothing to check point.
    pub fn checkpoint(&mut self, reason: impl Into<String>) -> Result<Checkpoint> {
        let head = self.head.ok_or(AgentError::Empty)?;
        let commit = self.load_commit(&head)?;
        let cp = Checkpoint::new(reason, head, commit.snapshot);
        self.checkpoints.push(cp.clone());
        self.write_checkpoints()?;
        tracing::info!(checkpoint = %cp.id, "saved checkpoint");
        Ok(cp)
    }

    /// All saved checkpoints, oldest first.
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Roll back `HEAD` to the most recent commit whose intent best matches
    /// `query`, and return that commit's hash.
    ///
    /// Matching prefers an exact (normalized) match, then a substring match,
    /// then trigram similarity. Among equally strong matches the most recent
    /// wins. Fails with [`AgentError::NoMatch`] if nothing matches.
    pub fn rollback_to_intent(&mut self, query: &str) -> Result<ContentHash> {
        let history = self.history()?;
        // History is newest-first, so the first max_by keeps the most recent on
        // ties, as long as we use a strictly-greater comparison downstream.
        let best = history
            .iter()
            .map(|(hash, commit)| (hash, match_strength(query, &commit.intent).score()))
            .filter(|(_, score)| *score >= 0.0)
            .fold(None, |acc: Option<(&ContentHash, f32)>, (hash, score)| {
                match acc {
                    // Strictly greater keeps the earliest-seen (most recent)
                    // commit when scores tie.
                    Some((_, best)) if score > best => Some((hash, score)),
                    None => Some((hash, score)),
                    other => other,
                }
            });

        match best {
            Some((hash, _)) => {
                self.head = Some(*hash);
                self.write_head()?;
                tracing::info!(target = %hash, query, "rolled back to intent");
                Ok(*hash)
            }
            None => Err(AgentError::NoMatch(query.to_string())),
        }
    }

    /// Roll back `HEAD` to the most recent commit whose confidence is at least
    /// `min`, and return that commit's hash.
    ///
    /// This answers "go back to the last point I was sure about". Fails with
    /// [`AgentError::NoMatch`] if no commit meets the threshold.
    pub fn rollback_to_confidence(&mut self, min: f32) -> Result<ContentHash> {
        let history = self.history()?;
        let found = history.iter().find(|(_, c)| c.confidence >= min);
        match found {
            Some((hash, _)) => {
                self.head = Some(*hash);
                self.write_head()?;
                tracing::info!(target = %hash, min, "rolled back to confidence");
                Ok(*hash)
            }
            None => Err(AgentError::NoMatch(format!("confidence >= {min}"))),
        }
    }

    /// Open a new task and make it the active task. Commits made afterward are
    /// recorded against it until it is completed or abandoned.
    ///
    /// `parent` optionally nests this task under another, forming the task DAG.
    pub fn open_task(&mut self, description: impl Into<String>, parent: Option<Uuid>) -> Result<Uuid> {
        if let Some(parent_id) = parent {
            if !self.tasks.iter().any(|t| t.id == parent_id) {
                return Err(AgentError::TaskNotFound(parent_id));
            }
        }
        let task = TaskNode::open(description, parent);
        let id = task.id;
        self.tasks.push(task);
        self.active_task = Some(id);
        self.write_tasks()?;
        self.write_active_task()?;
        tracing::info!(task = %id, "opened task");
        Ok(id)
    }

    /// Mark a task complete. If it was the active task, clears the active task.
    pub fn complete_task(&mut self, id: Uuid) -> Result<()> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(AgentError::TaskNotFound(id))?;
        task.complete();
        if self.active_task == Some(id) {
            self.active_task = None;
            self.write_active_task()?;
        }
        self.write_tasks()?;
        tracing::info!(task = %id, "completed task");
        Ok(())
    }

    /// Look up a task by id.
    pub fn task(&self, id: Uuid) -> Option<&TaskNode> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// All tasks in the graph, in creation order.
    pub fn tasks(&self) -> &[TaskNode] {
        &self.tasks
    }

    /// The currently active task, if any.
    pub fn active_task(&self) -> Option<Uuid> {
        self.active_task
    }

    // --- persistence helpers ---

    fn write_head(&self) -> Result<()> {
        let value = self.head.map(|h| h.to_hex()).unwrap_or_default();
        self.write_string("HEAD", &value)
    }

    fn write_checkpoints(&self) -> Result<()> {
        self.write_json("checkpoints.json", &self.checkpoints)
    }

    fn write_tasks(&self) -> Result<()> {
        self.write_json("tasks.json", &self.tasks)
    }

    fn write_active_task(&self) -> Result<()> {
        let value = self.active_task.map(|id| id.to_string()).unwrap_or_default();
        self.write_string("active_task", &value)
    }

    fn write_string(&self, name: &str, value: &str) -> Result<()> {
        let path = self.meta.join(name);
        // temp-then-rename so a concurrent reader never sees a half-written file
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, value).map_err(|e| AgentError::io(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| AgentError::io(&path, e))
    }

    fn write_json<T: serde::Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value)?;
        let s = String::from_utf8(bytes).expect("serde_json emits valid utf-8");
        self.write_string(name, &s)
    }

    fn read_string(meta: &Path, name: &str) -> Result<Option<String>> {
        let path = meta.join(name);
        match fs::read_to_string(&path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AgentError::io(&path, e)),
        }
    }

    fn read_json<T: serde::de::DeserializeOwned>(meta: &Path, name: &str) -> Result<Option<T>> {
        match Self::read_string(meta, name)? {
            Some(s) if !s.trim().is_empty() => Ok(Some(serde_json::from_str(&s)?)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(store: &BlobStore, bytes: &[u8]) -> ContentHash {
        store.put(bytes).unwrap()
    }

    fn repo() -> (tempfile::TempDir, AgentRepo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = AgentRepo::open(dir.path()).unwrap();
        (dir, repo)
    }

    #[test]
    fn commit_advances_head_and_links_parent() {
        let (_dir, mut repo) = repo();
        let s1 = snap(repo.store(), b"tree-1");
        let c1 = repo.commit(AgentContext::new("first", 0.9), s1).unwrap();
        assert_eq!(repo.head(), Some(c1));

        let s2 = snap(repo.store(), b"tree-2");
        let c2 = repo.commit(AgentContext::new("second", 0.9), s2).unwrap();
        assert_eq!(repo.head(), Some(c2));
        assert_eq!(repo.load_commit(&c2).unwrap().parent, Some(c1));

        let history = repo.history().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, c2); // newest first
    }

    #[test]
    fn state_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let head;
        {
            let mut repo = AgentRepo::open(dir.path()).unwrap();
            let s = snap(repo.store(), b"tree");
            head = repo.commit(AgentContext::new("persist me", 1.0), s).unwrap();
            repo.checkpoint("cp").unwrap();
        }
        let repo = AgentRepo::open(dir.path()).unwrap();
        assert_eq!(repo.head(), Some(head));
        assert_eq!(repo.checkpoints().len(), 1);
    }

    #[test]
    fn checkpoint_pins_current_commit() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"tree");
        let c = repo.commit(AgentContext::new("work", 0.8), s).unwrap();
        let cp = repo.checkpoint("before refactor").unwrap();
        assert_eq!(cp.commit_at, c);
        assert_eq!(cp.snapshot, s);
    }

    #[test]
    fn checkpoint_on_empty_repo_errors() {
        let (_dir, mut repo) = repo();
        assert!(matches!(repo.checkpoint("x"), Err(AgentError::Empty)));
    }

    #[test]
    fn rollback_to_intent_moves_head_to_match() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        let good = repo
            .commit(AgentContext::new("working auth flow", 0.9), s)
            .unwrap();
        repo.commit(AgentContext::new("broke the parser", 0.4), s)
            .unwrap();

        let target = repo.rollback_to_intent("working auth").unwrap();
        assert_eq!(target, good);
        assert_eq!(repo.head(), Some(good));
    }

    #[test]
    fn rollback_to_intent_prefers_most_recent_on_tie() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        repo.commit(AgentContext::new("fix auth", 0.9), s).unwrap();
        let newer = repo.commit(AgentContext::new("fix auth", 0.9), s).unwrap();
        // Two exact matches; the more recent one should be chosen.
        assert_eq!(repo.rollback_to_intent("fix auth").unwrap(), newer);
    }

    #[test]
    fn rollback_to_intent_with_no_match_errors() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        repo.commit(AgentContext::new("something", 0.5), s).unwrap();
        assert!(matches!(
            repo.rollback_to_intent("totally unrelated packfile"),
            Err(AgentError::NoMatch(_))
        ));
    }

    #[test]
    fn rollback_to_confidence_finds_last_sure_commit() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        let sure = repo.commit(AgentContext::new("solid", 0.95), s).unwrap();
        repo.commit(AgentContext::new("shaky", 0.3), s).unwrap();

        let target = repo.rollback_to_confidence(0.9).unwrap();
        assert_eq!(target, sure);
        assert_eq!(repo.head(), Some(sure));
    }

    #[test]
    fn rollback_to_confidence_unmet_threshold_errors() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        repo.commit(AgentContext::new("meh", 0.5), s).unwrap();
        assert!(matches!(
            repo.rollback_to_confidence(0.99),
            Err(AgentError::NoMatch(_))
        ));
    }

    #[test]
    fn commits_are_recorded_against_active_task() {
        let (_dir, mut repo) = repo();
        let s = snap(repo.store(), b"t");
        let task = repo.open_task("refactor auth", None).unwrap();
        let c = repo.commit(AgentContext::new("step", 0.7), s).unwrap();
        assert_eq!(repo.load_commit(&c).unwrap().task_id, Some(task));
        assert_eq!(repo.task(task).unwrap().commits, vec![c]);
    }

    #[test]
    fn completing_active_task_clears_it() {
        let (_dir, mut repo) = repo();
        let task = repo.open_task("t", None).unwrap();
        assert_eq!(repo.active_task(), Some(task));
        repo.complete_task(task).unwrap();
        assert_eq!(repo.active_task(), None);
        assert!(!repo.task(task).unwrap().is_open());
    }

    #[test]
    fn sub_task_requires_existing_parent() {
        let (_dir, mut repo) = repo();
        let missing = Uuid::new_v4();
        assert!(matches!(
            repo.open_task("child", Some(missing)),
            Err(AgentError::TaskNotFound(_))
        ));
    }
}
