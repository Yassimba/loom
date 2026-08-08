//! Render a callstack diff (or a plain tree) as an ASCII tree with +/- markers.

use owo_colors::OwoColorize;

use crate::types::{DiffNode, DiffStatus, NodeKind};

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    /// When false, skip ANSI colors (useful for tests and pipes).
    pub color: bool,
}

fn paint(status: DiffStatus, text: &str, color: bool) -> String {
    if !color {
        return text.to_string();
    }
    match status {
        DiffStatus::Added => text.blue().to_string(),
        DiffStatus::Removed => text.magenta().to_string(),
        DiffStatus::Same => text.to_string(),
    }
}

fn status_prefix(status: DiffStatus, color: bool) -> String {
    let raw = match status {
        DiffStatus::Added => "+",
        DiffStatus::Removed => "-",
        DiffStatus::Same => " ",
    };
    paint(status, raw, color)
}

pub fn render_diff(root: &DiffNode, options: RenderOptions) -> String {
    let mut lines: Vec<String> = Vec::new();
    walk(root, "", true, true, options.color, &mut lines);
    lines.join("\n")
}

fn walk(
    node: &DiffNode,
    indent: &str,
    is_last: bool,
    is_root: bool,
    color: bool,
    lines: &mut Vec<String>,
) {
    let branch = if is_root {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    lines.push(format!(
        "{} {indent}{branch}{}",
        status_prefix(node.status, color),
        paint(node.status, &node.label, color),
    ));

    // Conditional arms omit the continuing │ rail — they are alternate paths,
    // not a nested stack that continues past the branch.
    let rail = if node.kind == NodeKind::Branch || is_last || is_root {
        "   "
    } else {
        "│  "
    };
    let child_indent = if is_root {
        String::new()
    } else {
        format!("{indent}{rail}")
    };

    let count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        walk(
            child,
            &child_indent,
            index == count - 1,
            false,
            color,
            lines,
        );
    }
}
