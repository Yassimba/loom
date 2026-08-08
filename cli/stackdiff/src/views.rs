//! Alternate diagram views generated from the call data: a sequence
//! diagram of who-messages-whom in execution order, and a class ("ER")
//! diagram of the types in play. Layout is delegated to mermaid-text;
//! diff colors are painted onto the rendered lines afterward, since we
//! know exactly which labels are added, removed, or changed.

use std::collections::BTreeMap;

use crate::types::{DiffNode, DiffStatus, NodeKind};

/// A label expected in the rendered output, with the status to paint it.
pub type Marks = Vec<(String, DiffStatus)>;

fn clip(text: &str, max: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > max {
        let mut short: String = collapsed.chars().take(max - 1).collect();
        short.push('…');
        short
    } else {
        collapsed
    }
}

fn participant(node: &DiffNode, fallback: &str) -> String {
    node.location
        .as_deref()
        .and_then(|location| {
            let path = location
                .rsplit_once(':')
                .map(|(p, _)| p)
                .unwrap_or(location);
            path.rsplit('/').next().map(str::to_string)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn mark(marks: &mut Marks, label: &str, status: DiffStatus) {
    if status != DiffStatus::Same && !label.is_empty() {
        marks.push((label.to_string(), status));
    }
}

/// Build mermaid `sequenceDiagram` source from a diff tree.
pub fn sequence_mermaid(root: &DiffNode) -> (String, Marks) {
    let mut lines = vec!["sequenceDiagram".to_string()];
    let mut marks = Marks::new();
    let home = participant(root, "«entry»");
    walk_sequence(root, &home, &mut lines, &mut marks);
    (lines.join("\n"), marks)
}

fn walk_sequence(node: &DiffNode, from: &str, lines: &mut Vec<String>, marks: &mut Marks) {
    for child in &node.children {
        if child.key == "…" {
            continue;
        }
        match child.kind {
            NodeKind::Branch => {
                let condition = clip(&child.label, 40);
                lines.push(format!("alt {condition}"));
                mark(marks, &condition, child.status);
                walk_sequence(child, from, lines, marks);
                lines.push("end".to_string());
            }
            NodeKind::Call => {
                let to = participant(child, from);
                let message = clip(&child.label, 48);
                lines.push(format!("{from}->>{to}: {message}"));
                mark(marks, &message, child.status);
                walk_sequence(child, &to, lines, marks);
                if let Some(returns) = &child.returns {
                    if to != *from {
                        let reply = clip(returns, 32);
                        lines.push(format!("{to}-->>{from}: {reply}"));
                        mark(marks, &reply, child.status);
                    }
                }
            }
        }
    }
}

/// Type name of a `Type.method` / `Type::method` key, if it looks like one.
fn type_of(key: &str) -> Option<String> {
    let (type_name, _) = key.split_once("::").or_else(|| key.split_once('.'))?;
    type_name
        .chars()
        .next()
        .filter(|c| c.is_uppercase())
        .map(|_| type_name.to_string())
}

/// Build mermaid `classDiagram` source from the types reached by the trees:
/// each type with its touched methods, edges where one type's method calls
/// into another type.
pub fn class_mermaid(
    roots: &[DiffNode],
    types: &BTreeMap<String, Vec<(String, Option<String>)>>,
) -> Option<(String, Marks)> {
    let mut methods: BTreeMap<String, BTreeMap<String, DiffStatus>> = BTreeMap::new();
    let mut edges: BTreeMap<(String, String), DiffStatus> = BTreeMap::new();
    for root in roots {
        collect_classes(root, type_of(&root.key), &mut methods, &mut edges);
    }
    if methods.is_empty() {
        return None;
    }

    let mut lines = vec!["classDiagram".to_string()];
    let mut marks = Marks::new();
    for (type_name, type_methods) in &methods {
        lines.push(format!("class {type_name} {{"));
        for (field, field_type) in types.get(type_name).into_iter().flatten() {
            match field_type {
                Some(ty) => lines.push(format!("  +{field} {ty}")),
                None => lines.push(format!("  +{field}")),
            }
        }
        for (method, status) in type_methods {
            lines.push(format!("  +{method}()"));
            mark(&mut marks, &format!("{method}()"), *status);
        }
        lines.push("}".to_string());
    }
    for ((from, to), status) in &edges {
        lines.push(format!("{from} ..> {to}"));
        if *status != DiffStatus::Same {
            mark(&mut marks, &format!("{from} ..> {to}"), *status);
        }
    }
    Some((lines.join("\n"), marks))
}

fn note_method(
    methods: &mut BTreeMap<String, BTreeMap<String, DiffStatus>>,
    type_name: &str,
    method: &str,
    status: DiffStatus,
) {
    let slot = methods
        .entry(type_name.to_string())
        .or_default()
        .entry(method.to_string())
        .or_insert(status);
    if *slot == DiffStatus::Same {
        *slot = status;
    }
}

fn collect_classes(
    node: &DiffNode,
    context: Option<String>,
    methods: &mut BTreeMap<String, BTreeMap<String, DiffStatus>>,
    edges: &mut BTreeMap<(String, String), DiffStatus>,
) {
    let own_type = type_of(&node.key).or(context);
    if let (Some(type_name), Some((_, method))) = (
        type_of(&node.key),
        node.key
            .split_once("::")
            .or_else(|| node.key.split_once('.')),
    ) {
        note_method(methods, &type_name, method, node.status);
    }
    for child in &node.children {
        if let (Some(from), Some(to)) = (own_type.as_deref(), type_of(&child.key)) {
            if from != to {
                let slot = edges
                    .entry((from.to_string(), to.clone()))
                    .or_insert(child.status);
                if *slot == DiffStatus::Same {
                    *slot = child.status;
                }
            }
        }
        collect_classes(child, own_type.clone(), methods, edges);
    }
}

fn paint(status: DiffStatus, text: &str) -> String {
    let code = match status {
        DiffStatus::Added => "\x1b[38;2;63;185;80m",
        DiffStatus::Removed => "\x1b[38;2;248;81;73m",
        DiffStatus::Changed => "\x1b[38;2;210;153;34m",
        DiffStatus::Same => return text.to_string(),
    };
    format!("{code}{text}\x1b[0m")
}

/// Render mermaid source through mermaid-text and paint the marked labels.
pub fn render_colored(
    source: &str,
    marks: &Marks,
    color: bool,
    max_width: Option<usize>,
) -> anyhow::Result<String> {
    let rendered = mermaid_text::render_with_width(source, max_width)
        .map_err(|error| anyhow::anyhow!("mermaid-text failed: {error}"))?;
    if !color {
        return Ok(rendered);
    }
    let mut sorted: Vec<&(String, DiffStatus)> = marks.iter().collect();
    sorted.sort_by_key(|(label, _)| std::cmp::Reverse(label.chars().count()));
    let out = rendered
        .lines()
        .map(|line| {
            let mut line = line.to_string();
            for (label, status) in &sorted {
                if let Some(at) = line.find(label.as_str()) {
                    line = format!(
                        "{}{}{}",
                        &line[..at],
                        paint(*status, label),
                        &line[at + label.len()..]
                    );
                    break;
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(out)
}

/// Module zoom: one node per file, an edge when a call crosses files,
/// statuses aggregated (any added/removed/changed edge wins over same).
pub fn module_mermaid(roots: &[DiffNode]) -> Option<(String, Marks)> {
    fn file_of(node: &DiffNode) -> Option<String> {
        node.location.as_deref().map(|location| {
            let path = location
                .rsplit_once(':')
                .map(|(p, _)| p)
                .unwrap_or(location);
            path.rsplit('/').next().unwrap_or(path).to_string()
        })
    }
    fn collect(
        node: &DiffNode,
        from: Option<&String>,
        edges: &mut BTreeMap<(String, String), DiffStatus>,
    ) {
        let own = file_of(node).or_else(|| from.cloned());
        if let (Some(from_file), Some(to_file)) = (from, file_of(node).as_ref()) {
            if from_file != to_file {
                let slot = edges
                    .entry((from_file.clone(), to_file.clone()))
                    .or_insert(node.status);
                if *slot == DiffStatus::Same {
                    *slot = node.status;
                }
            }
        }
        for child in &node.children {
            collect(child, own.as_ref(), edges);
        }
    }
    let mut edges = BTreeMap::new();
    for root in roots {
        collect(root, None, &mut edges);
    }
    if edges.is_empty() {
        return None;
    }
    let mut lines = vec!["flowchart LR".to_string()];
    let mut marks = Marks::new();
    let mut ids: BTreeMap<&String, usize> = BTreeMap::new();
    for (from, to) in edges.keys() {
        let next = ids.len();
        ids.entry(from).or_insert(next);
        let next = ids.len();
        ids.entry(to).or_insert(next);
    }
    for (file, id) in &ids {
        lines.push(format!("n{id}[{file}]"));
    }
    for ((from, to), status) in &edges {
        lines.push(format!("n{} --> n{}", ids[from], ids[to]));
        if *status != DiffStatus::Same {
            mark(&mut marks, from, *status);
            mark(&mut marks, to, *status);
        }
    }
    Some((lines.join("\n"), marks))
}

/// Lineage view: the call DAG at data granularity. One node per function
/// (drawn once — convergence is visible as fan-in), data constructors as
/// stadium nodes, edges labeled with the binding the result lands in.
/// Branches flatten away; unresolved plumbing drops out entirely.
pub fn lineage_mermaid(roots: &[DiffNode]) -> Option<(String, Marks)> {
    #[derive(Default)]
    struct Graph {
        nodes: BTreeMap<String, (String, DiffStatus, bool)>,
        edges: BTreeMap<(String, String), (Option<String>, DiffStatus)>,
    }

    fn is_data(node: &DiffNode) -> bool {
        node.location.is_none()
            && node
                .key
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
    }

    fn keep(node: &DiffNode) -> bool {
        node.location.is_some() || is_data(node)
    }

    fn merge_status(slot: &mut DiffStatus, status: DiffStatus) {
        if *slot == DiffStatus::Same {
            *slot = status;
        }
    }

    fn label_of(node: &DiffNode) -> String {
        let name = node.key.clone();
        match &node.returns {
            Some(ret) => clip(&format!("{name} → {ret}"), 44),
            None => clip(&name, 44),
        }
    }

    fn walk(node: &DiffNode, ancestor: Option<&str>, graph: &mut Graph) {
        let own: Option<String> = if keep(node) {
            let data = is_data(node);
            let entry = graph
                .nodes
                .entry(node.key.clone())
                .or_insert_with(|| (label_of(node), node.status, data));
            merge_status(&mut entry.1, node.status);
            if let Some(from) = ancestor {
                if from != node.key {
                    let edge = graph
                        .edges
                        .entry((from.to_string(), node.key.clone()))
                        .or_insert_with(|| (node.meta.binding.clone(), node.status));
                    merge_status(&mut edge.1, node.status);
                }
            }
            Some(node.key.clone())
        } else {
            None
        };
        let next = own.as_deref().or(ancestor);
        for child in &node.children {
            if child.key == "…" || child.key == "▸" {
                continue;
            }
            walk(child, next, graph);
        }
    }

    let mut graph = Graph::default();
    for root in roots {
        walk(root, None, &mut graph);
    }
    if graph.edges.is_empty() {
        return None;
    }

    let mut ids: BTreeMap<&String, usize> = BTreeMap::new();
    for key in graph.nodes.keys() {
        let next = ids.len();
        ids.insert(key, next);
    }
    let mut lines = vec!["flowchart LR".to_string()];
    let mut marks = Marks::new();
    for (key, (label, status, data)) in &graph.nodes {
        let id = ids[key];
        if *data {
            lines.push(format!("n{id}([{label}])"));
        } else {
            lines.push(format!("n{id}[{label}]"));
        }
        mark(&mut marks, label, *status);
    }
    for ((from, to), (binding, status)) in &graph.edges {
        let (Some(from_id), Some(to_id)) = (ids.get(from), ids.get(to)) else {
            continue;
        };
        match binding {
            Some(binding) => {
                lines.push(format!("n{from_id} -->|{binding}| n{to_id}"));
                mark(&mut marks, binding, *status);
            }
            None => lines.push(format!("n{from_id} --> n{to_id}")),
        }
    }
    Some((lines.join("\n"), marks))
}
