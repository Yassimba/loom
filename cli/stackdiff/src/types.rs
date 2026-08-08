#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Call,
    Branch,
}

#[derive(Debug, Clone)]
pub struct CallNode {
    /// Stable identity used for matching across versions, e.g. "PiService.createAgentSession"
    pub key: String,
    /// Display label, e.g. "PiService.createAgentSession(options)" or "if (!options.sessionId)"
    pub label: String,
    /// Branches omit the continuing │ rail so arms read as alternate paths
    pub kind: NodeKind,
    pub children: Vec<CallNode>,
}

/// One step in a function body: a call, or a conditional branch with nested steps.
#[derive(Debug, Clone)]
pub enum CallStep {
    Call {
        key: String,
    },
    Branch {
        key: String,
        label: String,
        children: Vec<CallStep>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    Same,
    Added,
    Removed,
}

#[derive(Debug, Clone)]
pub struct DiffNode {
    pub key: String,
    pub label: String,
    pub status: DiffStatus,
    pub kind: NodeKind,
    pub children: Vec<DiffNode>,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Stable key: "foo" or "ClassName.method" or "ClassName.constructor"
    pub key: String,
    pub label: String,
    #[allow(dead_code)]
    pub file: String,
    /// Ordered body steps (calls + if/else branches)
    pub steps: Vec<CallStep>,
    pub exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Snapshot {
    Commit(String),
    Worktree,
}

impl Snapshot {
    pub fn describe(&self) -> &str {
        match self {
            Snapshot::Commit(r) => r,
            Snapshot::Worktree => "working tree",
        }
    }
}
