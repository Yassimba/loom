//! Expand a function into a nested call tree by following known definitions.

use std::collections::HashSet;

use crate::extract::FunctionIndex;
use crate::types::{CallMeta, CallNode, CallStep, FunctionInfo, NodeKind};

/// Look up a callable by key, falling back to its `new X` constructor alias —
/// languages without a `new` keyword (Python) emit plain `X` for instantiation.
pub fn lookup_callable<'a>(key: &str, index: &'a FunctionIndex) -> Option<&'a FunctionInfo> {
    index.get(key).or_else(|| index.get(&format!("new {key}")))
}

fn display_call_label(key: &str, index: &FunctionIndex) -> String {
    if let Some(info) = lookup_callable(key, index) {
        return info.label.clone();
    }
    if key.contains('(') {
        key.to_string()
    } else {
        format!("{key}()")
    }
}

fn expand_steps(
    steps: &[CallStep],
    index: &FunctionIndex,
    depth: usize,
    max_depth: usize,
    visiting: &mut HashSet<String>,
) -> Vec<CallNode> {
    steps
        .iter()
        .map(|step| match step {
            CallStep::Branch {
                key,
                label,
                children,
            } => CallNode {
                key: key.clone(),
                label: label.clone(),
                kind: NodeKind::Branch,
                location: None,
                doc: None,
                returns: None,
                meta: CallMeta::default(),
                children: expand_steps(children, index, depth, max_depth, visiting),
            },
            CallStep::Call { key, meta } => {
                let mut node = expand_call(key, index, depth, max_depth, visiting);
                node.meta = meta.clone();
                node
            }
        })
        .collect()
}

fn leaf(key: &str, label: String, info: Option<&FunctionInfo>) -> CallNode {
    CallNode {
        key: key.to_string(),
        label,
        kind: NodeKind::Call,
        location: info.map(|info| format!("{}:{}", info.file, info.line)),
        doc: info.and_then(|info| info.doc.clone()),
        returns: info.and_then(|info| info.returns.clone()),
        meta: CallMeta::default(),
        children: Vec::new(),
    }
}

fn expand_call(
    key: &str,
    index: &FunctionIndex,
    depth: usize,
    max_depth: usize,
    visiting: &mut HashSet<String>,
) -> CallNode {
    let label = display_call_label(key, index);
    let info = lookup_callable(key, index);

    if depth >= max_depth {
        return leaf(key, label, info);
    }

    let Some(info) = info else {
        return leaf(key, label, None);
    };

    if visiting.contains(&info.key) {
        return leaf(key, format!("{label} ⇄"), Some(info));
    }

    visiting.insert(info.key.clone());
    let children = expand_steps(&info.steps, index, depth + 1, max_depth, visiting);
    visiting.remove(&info.key);

    CallNode {
        children,
        ..leaf(key, label, Some(info))
    }
}

pub fn build_call_tree(entry_key: &str, index: &FunctionIndex, max_depth: usize) -> CallNode {
    let resolved = resolve_entry(entry_key, index).unwrap_or_else(|| entry_key.to_string());
    expand_call(&resolved, index, 0, max_depth, &mut HashSet::new())
}

pub fn resolve_entry(entry: &str, index: &FunctionIndex) -> Option<String> {
    if index.contains_key(entry) {
        return Some(entry.to_string());
    }

    let stripped = entry.strip_suffix("()").unwrap_or(entry);
    if index.contains_key(stripped) {
        return Some(stripped.to_string());
    }

    let mut matches: Vec<&String> = index
        .keys()
        .filter(|key| {
            key.as_str() == entry
                || key.ends_with(&format!(".{entry}"))
                || key.as_str() == format!("new {entry}")
        })
        .collect();

    if matches.len() == 1 {
        return Some(matches[0].clone());
    }
    if matches.len() > 1 {
        let exported: Vec<&&String> = matches
            .iter()
            .filter(|key| index.get(**key).map(|f| f.exported).unwrap_or(false))
            .collect();
        if exported.len() == 1 {
            return Some((*exported[0]).clone());
        }
        matches.sort();
        return Some(matches[0].clone());
    }

    None
}
