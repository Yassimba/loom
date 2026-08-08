use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    Call,
    Branch,
}

/// Call-site facts gathered at extraction: the binding that captures the
/// result, the argument texts as written, and which earlier bindings of the
/// same body this call consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CallMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consumes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallNode {
    /// Stable identity used for matching across versions, e.g. "PiService.createAgentSession"
    pub key: String,
    /// Display label, e.g. "PiService.createAgentSession(options)" or "if (!options.sessionId)"
    pub label: String,
    /// Branches omit the continuing │ rail so arms read as alternate paths
    pub kind: NodeKind,
    /// Where the callee is defined: "path:line" (resolved calls only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// First sentence of the callee's doc comment, as written in source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    /// The callee's declared return type, as written in source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
    /// The callee's typed parameter list, e.g. "(target: RunTarget)".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(flatten)]
    pub meta: CallMeta,
    pub children: Vec<CallNode>,
}

/// One step in a function body: a call, or a conditional branch with nested steps.
#[derive(Debug, Clone)]
pub enum CallStep {
    Call {
        key: String,
        meta: CallMeta,
        /// Consecutive identical call sites collapsed (`×N` in labels).
        count: usize,
    },
    Branch {
        key: String,
        label: String,
        children: Vec<CallStep>,
    },
}

impl CallStep {
    pub fn call(key: impl Into<String>) -> Self {
        CallStep::Call {
            key: key.into(),
            meta: CallMeta::default(),
            count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiffStatus {
    Same,
    /// Same call target, different call-site (binding or arguments).
    Changed,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffNode {
    pub key: String,
    pub label: String,
    pub status: DiffStatus,
    pub kind: NodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(flatten)]
    pub meta: CallMeta,
    pub children: Vec<DiffNode>,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Stable key: "foo" or "ClassName.method" or "ClassName.constructor"
    pub key: String,
    pub label: String,
    pub file: String,
    /// 1-based line of the definition.
    pub line: usize,
    /// First sentence of the doc comment, as written.
    pub doc: Option<String>,
    /// Declared return type, as written.
    pub returns: Option<String>,
    /// Typed parameter list as written, e.g. "(target: RunTarget, n: int)".
    pub signature: Option<String>,
    /// Ordered body steps (calls + if/else branches)
    pub steps: Vec<CallStep>,
    pub exported: bool,
}

/// A type definition, for the --er view: name plus declared fields.
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub fields: Vec<(String, Option<String>)>,
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

/// 1-based line number of a byte offset.
pub fn line_of(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

/// First sentence (or first line) of a doc block, whichever ends sooner.
pub fn first_sentence(doc: &str) -> String {
    let line = doc.lines().next().unwrap_or("").trim();
    match line.find(". ") {
        Some(dot) => line[..=dot].trim().to_string(),
        None => line.to_string(),
    }
}
