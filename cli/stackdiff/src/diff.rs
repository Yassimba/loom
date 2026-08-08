//! Structural tree diff keyed by call identity.
//! Children are aligned with LCS so output order stays close to the "after" tree.

use crate::types::{CallNode, DiffNode, DiffStatus};

pub fn diff_trees(before: &CallNode, after: &CallNode) -> DiffNode {
    diff_node(Some(before), Some(after))
}

fn diff_node(before: Option<&CallNode>, after: Option<&CallNode>) -> DiffNode {
    match (before, after) {
        (Some(before), Some(after)) => DiffNode {
            key: after.key.clone(),
            label: after.label.clone(),
            kind: after.kind,
            status: if before.meta == after.meta {
                DiffStatus::Same
            } else {
                DiffStatus::Changed
            },
            location: after.location.clone(),
            doc: after.doc.clone(),
            returns: after.returns.clone(),
            signature: after.signature.clone(),
            meta: after.meta.clone(),
            children: diff_children(&before.children, &after.children),
        },
        (None, Some(after)) => mark_tree(after, DiffStatus::Added),
        (Some(before), None) => mark_tree(before, DiffStatus::Removed),
        (None, None) => unreachable!("diff_node called with no trees"),
    }
}

fn mark_tree(node: &CallNode, status: DiffStatus) -> DiffNode {
    DiffNode {
        key: node.key.clone(),
        label: node.label.clone(),
        kind: node.kind,
        status,
        location: node.location.clone(),
        doc: node.doc.clone(),
        returns: node.returns.clone(),
        signature: node.signature.clone(),
        meta: node.meta.clone(),
        children: node.children.iter().map(|c| mark_tree(c, status)).collect(),
    }
}

fn diff_children(before: &[CallNode], after: &[CallNode]) -> Vec<DiffNode> {
    let n = before.len();
    let m = after.len();
    // Weighted LCS: matching keys pair (weight 2), and identical call-site
    // meta upgrades the pair (weight 3) — so among same-key twins the
    // occurrence with matching arguments wins over the merely positional one.
    let score = |i: usize, j: usize| -> usize {
        if before[i].key != after[j].key {
            0
        } else if before[i].meta == after[j].meta {
            3
        } else {
            2
        }
    };
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let pair = match score(i, j) {
                0 => 0,
                s => dp[i + 1][j + 1] + s,
            };
            dp[i][j] = pair.max(dp[i + 1][j]).max(dp[i][j + 1]);
        }
    }

    let mut result = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        let s = score(i, j);
        if s > 0 && dp[i][j] == dp[i + 1][j + 1] + s {
            result.push(diff_node(Some(&before[i]), Some(&after[j])));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            result.push(diff_node(Some(&before[i]), None));
            i += 1;
        } else {
            result.push(diff_node(None, Some(&after[j])));
            j += 1;
        }
    }
    while i < n {
        result.push(diff_node(Some(&before[i]), None));
        i += 1;
    }
    while j < m {
        result.push(diff_node(None, Some(&after[j])));
        j += 1;
    }
    result
}

pub fn tree_has_changes(node: &DiffNode) -> bool {
    node.status != DiffStatus::Same || node.children.iter().any(tree_has_changes)
}

/// Keep only changed limbs plus `context` unchanged siblings around each;
/// elided runs collapse to one "…" node. Ancestors of changes always stay.
pub fn prune_unchanged(node: &DiffNode, context: usize) -> DiffNode {
    let keep: Vec<bool> = node.children.iter().map(tree_has_changes).collect();
    let near: Vec<bool> = (0..node.children.len())
        .map(|i| {
            let lo = i.saturating_sub(context);
            let hi = (i + context).min(keep.len().saturating_sub(1));
            (lo..=hi).any(|j| keep[j])
        })
        .collect();
    let mut children = Vec::new();
    let mut elided = false;
    for (index, child) in node.children.iter().enumerate() {
        if near.get(index).copied().unwrap_or(false) {
            children.push(prune_unchanged(child, context));
            elided = false;
        } else if !elided {
            children.push(DiffNode {
                key: "…".into(),
                label: "…".into(),
                kind: crate::types::NodeKind::Call,
                status: DiffStatus::Same,
                location: None,
                doc: None,
                returns: None,
                signature: None,
                meta: crate::types::CallMeta::default(),
                children: Vec::new(),
            });
            elided = true;
        }
    }
    DiffNode {
        children,
        ..DiffNode {
            key: node.key.clone(),
            label: node.label.clone(),
            kind: node.kind,
            status: node.status,
            location: node.location.clone(),
            doc: node.doc.clone(),
            returns: node.returns.clone(),
            signature: node.signature.clone(),
            meta: node.meta.clone(),
            children: Vec::new(),
        }
    }
}

/// Expand-once: after a subtree has been shown in full, later occurrences
/// of the same key collapse to a single "▸ shown above" stub. Runs across
/// a whole entry list in display order.
pub fn dedupe_subtrees(trees: &mut [DiffNode]) {
    let mut seen = std::collections::HashSet::new();
    for tree in trees {
        dedupe_node(tree, &mut seen);
    }
}

fn dedupe_node(node: &mut DiffNode, seen: &mut std::collections::HashSet<String>) {
    if node.children.is_empty() {
        return;
    }
    if !seen.insert(node.key.clone()) {
        node.children = vec![DiffNode {
            key: "▸".into(),
            label: "▸ shown above".into(),
            kind: crate::types::NodeKind::Call,
            status: DiffStatus::Same,
            location: None,
            doc: None,
            returns: None,
            signature: None,
            meta: crate::types::CallMeta::default(),
            children: Vec::new(),
        }];
        return;
    }
    for child in &mut node.children {
        dedupe_node(child, seen);
    }
}

fn is_data_node(node: &DiffNode) -> bool {
    node.location.is_none()
        && node
            .key
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
}

/// Lineage granularity: keep resolved functions and data constructors;
/// branch arms flatten away and unresolved plumbing drops out, whatever
/// their diff status. The default tree altitude (--flow restores all).
pub fn lineage_prune(node: &DiffNode) -> DiffNode {
    DiffNode {
        children: lineage_children(node),
        ..DiffNode {
            key: node.key.clone(),
            label: node.label.clone(),
            kind: node.kind,
            status: node.status,
            location: node.location.clone(),
            doc: node.doc.clone(),
            returns: node.returns.clone(),
            signature: node.signature.clone(),
            meta: node.meta.clone(),
            children: Vec::new(),
        }
    }
}

fn lineage_children(node: &DiffNode) -> Vec<DiffNode> {
    let mut out = Vec::new();
    for child in &node.children {
        let marker = child.key == "…" || child.key == "▸";
        let keep = marker || child.location.is_some() || is_data_node(child);
        if child.kind == crate::types::NodeKind::Branch {
            out.extend(lineage_children(child));
        } else if keep {
            out.push(lineage_prune(child));
        } else {
            out.extend(lineage_children(child));
        }
    }
    // Flattening can leave duplicate elision markers side by side.
    out.dedup_by(|a, b| a.key == "…" && b.key == "…");
    out
}
