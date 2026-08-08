//! Render a callstack diff (or a plain tree) as an ASCII tree with +/- markers.
//!
//! Plain mode prints exactly the label tree (byte-stable, used by tests).
//! Rich mode adds call-site facts per node: `binding = name(args) → ret`,
//! a dim `path:line` suffix (OSC 8-hyperlinked when a link template is set),
//! and a dim doc line under nodes that carry one.

use owo_colors::OwoColorize;

use crate::types::{DiffNode, DiffStatus, NodeKind};

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// When false, skip ANSI colors (useful for tests and pipes).
    pub color: bool,
    /// When true, show binding/args/returns/location/doc per node.
    pub rich: bool,
    /// URL template with `{path}` and `{line}` holes; wraps locations in
    /// OSC 8 hyperlinks. `{path}` is absolute when `repo_root` is set.
    pub link: Option<String>,
    /// Repo root used to absolutize `{path}` in link URLs.
    pub repo_root: Option<std::path::PathBuf>,
}

fn paint(status: DiffStatus, text: &str, color: bool) -> String {
    if !color {
        return text.to_string();
    }
    match status {
        DiffStatus::Added => text.green().to_string(),
        DiffStatus::Removed => text.red().to_string(),
        DiffStatus::Changed => text.yellow().to_string(),
        DiffStatus::Same => text.to_string(),
    }
}

fn dim(text: &str, color: bool) -> String {
    if color {
        text.dimmed().to_string()
    } else {
        text.to_string()
    }
}

fn status_prefix(status: DiffStatus, color: bool) -> String {
    let raw = match status {
        DiffStatus::Added => "+",
        DiffStatus::Removed => "-",
        DiffStatus::Changed => "!",
        DiffStatus::Same => " ",
    };
    paint(status, raw, color)
}

/// The node's display text: label in plain mode; in rich mode the call-site
/// view `binding = key(args) → ret` when call-site facts exist.
pub(crate) fn node_text(node: &DiffNode, rich: bool) -> String {
    if !rich || node.kind == NodeKind::Branch {
        return node.label.clone();
    }
    let mut text = String::new();
    if let Some(binding) = &node.meta.binding {
        text.push_str(binding);
        text.push_str(" = ");
    }
    if node.meta.args.is_empty() {
        text.push_str(&node.label);
    } else {
        // Call-site args replace the declared-parameter label.
        text.push_str(&node.key);
        text.push('(');
        text.push_str(&node.meta.args.join(", "));
        text.push(')');
    }
    if let Some(returns) = &node.returns {
        text.push_str(" → ");
        text.push_str(returns);
    }
    text
}

/// Compact display form of a location: basename:line. The full path still
/// backs the hyperlink; the screen only pays for the file name.
pub(crate) fn short_loc(location: &str) -> String {
    match location.rsplit_once(':') {
        Some((path, line)) => {
            let name = path.rsplit('/').next().unwrap_or(path);
            format!("{name}:{line}")
        }
        None => location.to_string(),
    }
}

fn location_suffix(node: &DiffNode, options: &RenderOptions) -> String {
    if !options.rich {
        return String::new();
    }
    let Some(location) = &node.location else {
        return String::new();
    };
    let shown = dim(&short_loc(location), options.color);
    let Some(template) = &options.link else {
        return format!("  {shown}");
    };
    let Some((path, line)) = location.rsplit_once(':') else {
        return format!("  {shown}");
    };
    let absolute = match &options.repo_root {
        Some(root) => root.join(path).to_string_lossy().into_owned(),
        None => path.to_string(),
    };
    let url = template
        .replace("{path}", &absolute)
        .replace("{line}", line);
    format!("  \x1b]8;;{url}\x1b\\{shown}\x1b]8;;\x1b\\")
}

pub fn render_diff(root: &DiffNode, options: &RenderOptions) -> String {
    let mut lines: Vec<String> = Vec::new();
    walk(root, "", true, true, options, &mut lines);
    lines.join("\n")
}

fn walk(
    node: &DiffNode,
    indent: &str,
    is_last: bool,
    is_root: bool,
    options: &RenderOptions,
    lines: &mut Vec<String>,
) {
    let color = options.color;
    // Plain mode hides call-site facts, so a call-site-only change has
    // nothing visible to mark.
    let status = if !options.rich && node.status == DiffStatus::Changed {
        DiffStatus::Same
    } else {
        node.status
    };
    let branch = if is_root {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    lines.push(format!(
        "{} {indent}{branch}{}{}",
        status_prefix(status, color),
        paint(status, &node_text(node, options.rich), color),
        location_suffix(node, options),
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

    if options.rich {
        if let Some(doc) = &node.doc {
            // The doc rides under its node, on the children's rail so the
            // tree lines stay connected.
            let rail_here = if node.children.is_empty() {
                "   "
            } else {
                "│  "
            };
            let doc_indent = format!("{child_indent}{rail_here}");
            lines.push(format!(
                "{} {doc_indent}{}",
                status_prefix(status, color),
                dim(&format!("“{doc}”"), color),
            ));
        }
    }

    let count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        walk(
            child,
            &child_indent,
            index == count - 1,
            false,
            options,
            lines,
        );
    }
}

/// Render the tree as a Mermaid flowchart: calls as boxes, branch arms as
/// diamonds, added nodes green, removed red.
pub fn render_mermaid(root: &DiffNode) -> String {
    let mut lines = vec!["flowchart TD".to_string()];
    let mut classes: Vec<String> = Vec::new();
    let mut counter = 0usize;
    mermaid_walk(root, None, &mut counter, &mut lines, &mut classes);
    lines.push("classDef added fill:#e6ffec,stroke:#1a7f37,color:#1a7f37".to_string());
    lines.push("classDef removed fill:#ffebe9,stroke:#cf222e,color:#cf222e".to_string());
    lines.push("classDef changed fill:#fff8c5,stroke:#9a6700,color:#9a6700".to_string());
    lines.extend(classes);
    lines.join("\n")
}

fn mermaid_walk(
    node: &DiffNode,
    parent: Option<usize>,
    counter: &mut usize,
    lines: &mut Vec<String>,
    classes: &mut Vec<String>,
) {
    let id = *counter;
    *counter += 1;
    let label = node_text(node, true).replace('"', "'");
    let shape = match node.kind {
        NodeKind::Branch => format!("n{id}{{\"{label}\"}}"),
        NodeKind::Call => format!("n{id}[\"{label}\"]"),
    };
    match parent {
        Some(parent) => lines.push(format!("n{parent} --> {shape}")),
        None => lines.push(shape),
    }
    match node.status {
        DiffStatus::Added => classes.push(format!("class n{id} added")),
        DiffStatus::Removed => classes.push(format!("class n{id} removed")),
        DiffStatus::Changed => classes.push(format!("class n{id} changed")),
        DiffStatus::Same => {}
    }
    for child in &node.children {
        mermaid_walk(child, Some(id), counter, lines, classes);
    }
}

/// Added/removed/changed node counts for `--stat`.
pub fn diff_stat(node: &DiffNode) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    count(node, &mut counts);
    counts
}

fn count(node: &DiffNode, counts: &mut (usize, usize, usize)) {
    match node.status {
        DiffStatus::Added => counts.0 += 1,
        DiffStatus::Removed => counts.1 += 1,
        DiffStatus::Changed => counts.2 += 1,
        DiffStatus::Same => {}
    }
    for child in &node.children {
        count(child, counts);
    }
}
