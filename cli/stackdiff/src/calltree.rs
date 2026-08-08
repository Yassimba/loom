//! Expand a function into a nested call tree by following known definitions.

use std::collections::HashSet;

use crate::extract::FunctionIndex;
use std::collections::HashMap;

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
                signature: None,
                meta: CallMeta::default(),
                children: expand_steps(children, index, depth, max_depth, visiting),
            },
            CallStep::Call { key, meta, count } => {
                let mut node = expand_call(key, index, depth, max_depth, visiting);
                node.meta = meta.clone();
                if *count > 1 {
                    node.label.push_str(&format!(" ×{count}"));
                }
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
        signature: info.and_then(|info| info.signature.clone()),
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

/// callee key → (caller key, call-site meta) edges, for --callers.
pub fn reverse_index(index: &FunctionIndex) -> HashMap<String, Vec<(String, CallMeta)>> {
    let mut reverse: HashMap<String, Vec<(String, CallMeta)>> = HashMap::new();
    fn walk(
        steps: &[CallStep],
        caller: &str,
        index: &FunctionIndex,
        reverse: &mut HashMap<String, Vec<(String, CallMeta)>>,
    ) {
        for step in steps {
            match step {
                CallStep::Call { key, meta, .. } => {
                    if let Some(info) = lookup_callable(key, index) {
                        reverse
                            .entry(info.key.clone())
                            .or_default()
                            .push((caller.to_string(), meta.clone()));
                    }
                }
                CallStep::Branch { children, .. } => walk(children, caller, index, reverse),
            }
        }
    }
    for info in index.values() {
        if info.key.starts_with("new ") {
            continue;
        }
        walk(&info.steps, &info.key, index, &mut reverse);
    }
    for edges in reverse.values_mut() {
        edges.sort_by(|a, b| a.0.cmp(&b.0));
        edges.dedup_by(|a, b| a.0 == b.0);
    }
    reverse
}

/// Who-calls-X tree: the root is `key`, children are its callers, and so on
/// upward. Each caller node carries the call-site meta of its edge down.
pub fn build_caller_tree(
    key: &str,
    index: &FunctionIndex,
    reverse: &HashMap<String, Vec<(String, CallMeta)>>,
    max_depth: usize,
) -> CallNode {
    let mut visiting = std::collections::HashSet::new();
    expand_callers(key, index, reverse, 0, max_depth, &mut visiting)
}

fn expand_callers(
    key: &str,
    index: &FunctionIndex,
    reverse: &HashMap<String, Vec<(String, CallMeta)>>,
    depth: usize,
    max_depth: usize,
    visiting: &mut std::collections::HashSet<String>,
) -> CallNode {
    let info = lookup_callable(key, index);
    let resolved_key = info
        .map(|i| i.key.clone())
        .unwrap_or_else(|| key.to_string());
    let mut node = CallNode {
        key: resolved_key.clone(),
        label: display_call_label(key, index),
        kind: NodeKind::Call,
        location: info.map(|i| format!("{}:{}", i.file, i.line)),
        doc: info.and_then(|i| i.doc.clone()),
        returns: info.and_then(|i| i.returns.clone()),
        signature: info.and_then(|i| i.signature.clone()),
        meta: CallMeta::default(),
        children: Vec::new(),
    };
    if depth >= max_depth || visiting.contains(&resolved_key) {
        return node;
    }
    visiting.insert(resolved_key.clone());
    if let Some(edges) = reverse.get(&resolved_key) {
        for (caller, meta) in edges {
            let mut child = expand_callers(caller, index, reverse, depth + 1, max_depth, visiting);
            child.meta = meta.clone();
            node.children.push(child);
        }
    }
    visiting.remove(&resolved_key);
    node
}

/// Closest entry keys to a miss, for "did you mean" suggestions.
pub fn suggest_entries(entry: &str, index: &FunctionIndex, limit: usize) -> Vec<String> {
    let needle = entry.to_lowercase();
    let mut scored: Vec<(i64, &String)> = index
        .keys()
        .filter(|key| !key.starts_with("new "))
        .filter_map(|key| {
            let hay = key.to_lowercase();
            let score = if hay == needle {
                1000
            } else if hay.contains(&needle) {
                600 - (hay.len() as i64 - needle.len() as i64)
            } else if needle.contains(&hay) && hay.len() >= 4 {
                300 + hay.len() as i64
            } else {
                let base = hay.rsplit(['.', ':']).next().unwrap_or(&hay);
                let dist = edit_distance(&needle, base);
                if dist <= 3 {
                    100 - dist as i64 * 20
                } else {
                    return None;
                }
            };
            Some((score, key))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, k)| k.clone())
        .collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current.push((prev[j] + cost).min(prev[j + 1] + 1).min(current[j] + 1));
        }
        prev = current;
    }
    prev[b.len()]
}

/// Score how much a function smells like a program entry point: nothing
/// calls it, and its name/path look like main/CLI/API surface.
pub fn entrypoint_score(
    key: &str,
    info: &FunctionInfo,
    reverse: &HashMap<String, Vec<(String, CallMeta)>>,
) -> i64 {
    if !info.exported || key.starts_with("new ") {
        return 0;
    }
    let mut score = 0;
    if !reverse.contains_key(key) {
        score += 4;
    }
    let name = key.rsplit(['.', ':']).next().unwrap_or(key).to_lowercase();
    if name == "main" {
        score += 4;
    }
    if name.starts_with("cmd") || name.starts_with("cli") || name.starts_with("handle") {
        score += 2;
    }
    if ["run", "serve", "execute", "start", "app"].contains(&name.as_str()) {
        score += 2;
    }
    let file = info.file.to_lowercase();
    for marker in [
        "/cli/",
        "/commands/",
        "/api/",
        "/routes/",
        "/endpoints/",
        "/presentation/",
        "/bin/",
    ] {
        if file.contains(marker) {
            score += 2;
            break;
        }
    }
    if file.ends_with("main.py") || file.ends_with("main.rs") || file.ends_with("main.go") {
        score += 2;
    }
    // A callable nobody calls with zero flavor is likely a library export.
    if score <= 4 && info.steps.is_empty() {
        return 0;
    }
    score
}

/// The functions a program starts from, best first.
pub fn detect_entrypoints(index: &FunctionIndex, limit: usize) -> Vec<String> {
    let reverse = reverse_index(index);
    let mut scored: Vec<(i64, &String)> = index
        .iter()
        .filter_map(|(key, info)| {
            let score = entrypoint_score(key, info, &reverse);
            (score >= 4).then_some((score, key))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, key)| key.clone())
        .collect()
}
