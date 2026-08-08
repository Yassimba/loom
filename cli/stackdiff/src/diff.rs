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
            status: DiffStatus::Same,
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
        children: node.children.iter().map(|c| mark_tree(c, status)).collect(),
    }
}

fn diff_children(before: &[CallNode], after: &[CallNode]) -> Vec<DiffNode> {
    let n = before.len();
    let m = after.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if before[i].key == after[j].key {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut result = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if before[i].key == after[j].key {
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
