//! Infer entrypoints: exported functions whose expanded call trees differ,
//! plus any explicitly requested entries.

use std::collections::BTreeSet;

use anyhow::{bail, Result};

use crate::calltree::{build_call_tree, resolve_entry};
use crate::diff::{diff_trees, tree_has_changes};
use crate::extract::FunctionIndex;
use crate::types::{CallNode, DiffNode, DiffStatus, NodeKind};

fn callee_set(index: &FunctionIndex, key: &str, max_depth: usize) -> String {
    let tree = build_call_tree(key, index, max_depth);
    let mut parts: Vec<String> = Vec::new();
    fn walk(node: &CallNode, depth: usize, parts: &mut Vec<String>) {
        parts.push(format!("{}{}", "  ".repeat(depth), node.key));
        for child in &node.children {
            walk(child, depth + 1, parts);
        }
    }
    walk(&tree, 0, &mut parts);
    parts.join("\n")
}

pub fn infer_entries(
    before: &FunctionIndex,
    after: &FunctionIndex,
    explicit: &[String],
    max_depth: usize,
) -> Result<Vec<String>> {
    if !explicit.is_empty() {
        let mut resolved: Vec<String> = Vec::new();
        for entry in explicit {
            let from_before = resolve_entry(entry, before);
            let from_after = resolve_entry(entry, after);
            let Some(key) = from_after.or(from_before) else {
                bail!("Entrypoint not found: {entry}");
            };
            if !resolved.contains(&key) {
                resolved.push(key);
            }
        }
        return Ok(resolved);
    }

    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let mut candidates: Vec<String> = Vec::new();

    for key in &keys {
        // Skip synthetic `new X` aliases for inference listing (still resolvable)
        if key.starts_with("new ") {
            continue;
        }

        let b = before.get(*key);
        let a = after.get(*key);

        // Prefer exported / public-ish roots
        let interesting =
            b.map(|f| f.exported).unwrap_or(false) || a.map(|f| f.exported).unwrap_or(false);
        if !interesting {
            continue;
        }

        let before_tree = b
            .map(|_| callee_set(before, key, max_depth))
            .unwrap_or_default();
        let after_tree = a
            .map(|_| callee_set(after, key, max_depth))
            .unwrap_or_default();

        if before_tree != after_tree {
            candidates.push((*key).clone());
        }
    }

    // If nothing exported changed, fall back to any function with a differing tree
    if candidates.is_empty() {
        for key in &keys {
            if key.starts_with("new ") {
                continue;
            }
            let before_tree = if before.contains_key(*key) {
                callee_set(before, key, max_depth)
            } else {
                String::new()
            };
            let after_tree = if after.contains_key(*key) {
                callee_set(after, key, max_depth)
            } else {
                String::new()
            };
            if before_tree != after_tree {
                candidates.push((*key).clone());
            }
        }
    }

    // Prefer stable, case-insensitive-ish ordering (localeCompare equivalent)
    candidates.sort_by(|a, b| {
        let al = a.to_lowercase();
        let bl = b.to_lowercase();
        al.cmp(&bl).then_with(|| a.cmp(b))
    });
    Ok(candidates)
}

pub fn diff_entry(
    key: &str,
    before: &FunctionIndex,
    after: &FunctionIndex,
    max_depth: usize,
) -> Option<DiffNode> {
    let before_key = resolve_entry(key, before).unwrap_or_else(|| key.to_string());
    let after_key = resolve_entry(key, after).unwrap_or_else(|| key.to_string());

    let has_before = before.contains_key(&before_key);
    let has_after = after.contains_key(&after_key);

    if !has_before && !has_after {
        return None;
    }

    if !has_before && has_after {
        let after_tree = build_call_tree(&after_key, after, max_depth);
        let empty = CallNode {
            key: after_key.clone(),
            label: after_tree.label.clone(),
            kind: NodeKind::Call,
            children: Vec::new(),
        };
        let mut diff = diff_trees(&empty, &after_tree);
        diff.status = DiffStatus::Added;
        return Some(diff);
    }

    if has_before && !has_after {
        let before_tree = build_call_tree(&before_key, before, max_depth);
        let empty = CallNode {
            key: before_key.clone(),
            label: before_tree.label.clone(),
            kind: NodeKind::Call,
            children: Vec::new(),
        };
        let mut diff = diff_trees(&before_tree, &empty);
        diff.status = DiffStatus::Removed;
        return Some(diff);
    }

    let before_tree = build_call_tree(&before_key, before, max_depth);
    let after_tree = build_call_tree(&after_key, after, max_depth);
    let diff = diff_trees(&before_tree, &after_tree);
    if !tree_has_changes(&diff) {
        return None;
    }
    Some(diff)
}
