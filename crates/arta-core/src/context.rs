//! Agent context objects.
//!
//! An [`AgentContext`] is arta's first-class record of *why* a change was made:
//! the agent's intent, the tool calls it issued, its reasoning, and how
//! confident it was. It is stored as JSON so that it survives a round-trip
//! through the git compat layer (where it lives in the commit body) and remains
//! readable by non-arta tooling.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::hash::ContentHash;
use crate::store::BlobStore;

/// A single tool invocation recorded alongside a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// The name of the tool that was called (e.g. `"edit_file"`).
    pub name: String,
    /// The arguments passed to the tool, as free-form JSON.
    pub arguments: serde_json::Value,
}

/// Structured metadata describing the agent's intent behind a change.
///
/// This is the payload behind an `AgentCommit` in the agent layer; keeping it
/// in `arta-core` lets the object store address and deduplicate it like any
/// other object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentContext {
    /// A short natural-language statement of what the change is meant to do.
    pub intent: String,
    /// The tool calls the agent issued while producing the change.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Optional longer-form reasoning chain.
    #[serde(default)]
    pub reasoning: Option<String>,
    /// The agent's confidence: `0.0` = guessing, `1.0` = certain.
    pub confidence: f32,
}

impl AgentContext {
    /// Create a context from an intent and confidence, with no tool calls or
    /// reasoning attached.
    pub fn new(intent: impl Into<String>, confidence: f32) -> Self {
        AgentContext {
            intent: intent.into(),
            tool_calls: Vec::new(),
            reasoning: None,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Attach a reasoning chain, consuming and returning `self` for chaining.
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    /// Record a tool call, consuming and returning `self` for chaining.
    pub fn with_tool_call(mut self, call: ToolCall) -> Self {
        self.tool_calls.push(call);
        self
    }

    /// Serialize to the canonical JSON byte form used for storage and for
    /// embedding in git commit bodies.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    /// Deserialize from the JSON byte form produced by [`AgentContext::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Store this context in `store`, returning its content hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        store.put(&self.to_json()?)
    }

    /// Load a context from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self> {
        AgentContext::from_json(&store.get(hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips() {
        let ctx = AgentContext::new("add retry to token refresh", 0.85)
            .with_reasoning("the refresh path had no backoff")
            .with_tool_call(ToolCall {
                name: "edit_file".into(),
                arguments: serde_json::json!({ "path": "auth.rs" }),
            });
        let bytes = ctx.to_json().unwrap();
        assert_eq!(AgentContext::from_json(&bytes).unwrap(), ctx);
    }

    #[test]
    fn confidence_is_clamped() {
        assert_eq!(AgentContext::new("x", 5.0).confidence, 1.0);
        assert_eq!(AgentContext::new("x", -1.0).confidence, 0.0);
    }

    #[test]
    fn store_and_load_via_blob_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        let ctx = AgentContext::new("first commit", 1.0);
        let hash = ctx.store(&store).unwrap();
        assert_eq!(AgentContext::load(&store, &hash).unwrap(), ctx);
    }

    #[test]
    fn optional_fields_default_when_absent() {
        // A minimal payload (intent + confidence only) must still parse.
        let bytes = br#"{"intent":"minimal","confidence":0.5}"#;
        let ctx = AgentContext::from_json(bytes).unwrap();
        assert_eq!(ctx.intent, "minimal");
        assert!(ctx.tool_calls.is_empty());
        assert!(ctx.reasoning.is_none());
    }
}
