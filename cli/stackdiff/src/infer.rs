//! Infer entrypoints: exported functions whose expanded call trees differ,
//! plus any explicitly requested entries.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

use anyhow::{bail, Result};

use crate::calltree::{build_call_tree, lookup_callable, resolve_entry};
use crate::diff::{diff_trees, tree_has_changes};
use crate::extract::FunctionIndex;
use crate::types::{CallMeta, CallNode, CallStep, DiffNode, DiffStatus, NodeKind};

/// Structural signature of a key's expanded call tree — the same shape
/// `build_call_tree` produces, hashed instead of built, memoized per
/// (key, depth). Subtrees that hit a recursion cycle skip the memo so the
/// cycle marker stays context-correct.
struct SigCache<'i> {
    index: &'i FunctionIndex,
    max_depth: usize,
    memo: HashMap<(String, usize), u64>,
}

impl<'i> SigCache<'i> {
    fn new(index: &'i FunctionIndex, max_depth: usize) -> Self {
        SigCache {
            index,
            max_depth,
            memo: HashMap::new(),
        }
    }

    fn tree_sig(&mut self, key: &str) -> u64 {
        let mut visiting = HashSet::new();
        self.call_sig(key, 0, &mut visiting).0
    }

    fn call_sig(&mut self, key: &str, depth: usize, visiting: &mut HashSet<String>) -> (u64, bool) {
        if let Some(sig) = self.memo.get(&(key.to_string(), depth)) {
            return (*sig, false);
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let mut cyclic = false;
        if depth < self.max_depth {
            if let Some(info) = lookup_callable(key, self.index) {
                if visiting.contains(&info.key) {
                    "⇄".hash(&mut hasher);
                    return (hasher.finish(), true);
                }
                let info_key = info.key.clone();
                let steps = info.steps.clone();
                visiting.insert(info_key.clone());
                cyclic = self.steps_sig(&steps, depth, visiting, &mut hasher);
                visiting.remove(&info_key);
            }
        }
        let sig = hasher.finish();
        if !cyclic {
            self.memo.insert((key.to_string(), depth), sig);
        }
        (sig, cyclic)
    }

    fn steps_sig(
        &mut self,
        steps: &[CallStep],
        depth: usize,
        visiting: &mut HashSet<String>,
        hasher: &mut std::collections::hash_map::DefaultHasher,
    ) -> bool {
        let mut cyclic = false;
        for step in steps {
            match step {
                CallStep::Branch { key, children, .. } => {
                    key.hash(hasher);
                    cyclic |= self.steps_sig(children, depth, visiting, hasher);
                }
                CallStep::Call { key, .. } => {
                    let (sig, hit) = self.call_sig(key, depth + 1, visiting);
                    sig.hash(hasher);
                    cyclic |= hit;
                }
            }
        }
        cyclic
    }
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
    let mut before_sigs = SigCache::new(before, max_depth);
    let mut after_sigs = SigCache::new(after, max_depth);
    let mut changed = |key: &str| {
        let b = if before.contains_key(key) {
            before_sigs.tree_sig(key)
        } else {
            0
        };
        let a = if after.contains_key(key) {
            after_sigs.tree_sig(key)
        } else {
            0
        };
        b != a
    };
    let mut candidates: Vec<String> = Vec::new();

    for key in &keys {
        // Skip synthetic `new X` aliases for inference listing (still resolvable)
        if key.starts_with("new ") {
            continue;
        }

        // Prefer exported / public-ish roots
        let interesting = before.get(*key).map(|f| f.exported).unwrap_or(false)
            || after.get(*key).map(|f| f.exported).unwrap_or(false);
        if !interesting {
            continue;
        }

        if changed(key) {
            candidates.push((*key).clone());
        }
    }

    // If nothing exported changed, fall back to any function with a differing tree
    if candidates.is_empty() {
        for key in &keys {
            if key.starts_with("new ") {
                continue;
            }
            if changed(key) {
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
            location: None,
            doc: None,
            returns: None,
            meta: CallMeta::default(),
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
            location: None,
            doc: None,
            returns: None,
            meta: CallMeta::default(),
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
