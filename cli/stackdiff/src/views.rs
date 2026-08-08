//! Alternate diagram views generated from the call data: a sequence
//! diagram of who-messages-whom in execution order, and a class ("ER")
//! diagram of the types in play. Layout is delegated to mermaid-text;
//! diff colors are painted onto the rendered lines afterward, since we
//! know exactly which labels are added, removed, or changed.

use std::collections::BTreeMap;

use crate::types::{DiffNode, DiffStatus, NodeKind};

/// A label expected in the rendered output: paint it by status, and wrap
/// it in an OSC 8 hyperlink when a url is attached.
#[derive(Debug)]
pub struct Mark {
    pub label: String,
    pub status: DiffStatus,
    pub url: Option<String>,
}

pub type Marks = Vec<Mark>;

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
        marks.push(Mark {
            label: label.to_string(),
            status,
            url: None,
        });
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

/// Connected clusters of the lineage graph, largest change first, each
/// with a suggested entry to open it with.
#[allow(clippy::type_complexity)]
fn cluster_overview(
    nodes: &BTreeMap<String, (String, DiffStatus, bool, Option<String>)>,
    edges: &BTreeMap<(String, String), (Option<String>, DiffStatus)>,
) -> Vec<ClusterRow> {
    let keys: Vec<&String> = nodes.keys().collect();
    let index: BTreeMap<&String, usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut parent: Vec<usize> = (0..keys.len()).collect();
    fn find(parent: &mut Vec<usize>, i: usize) -> usize {
        if parent[i] != i {
            let root = find(parent, parent[i]);
            parent[i] = root;
        }
        parent[i]
    }
    let mut incoming = vec![0usize; keys.len()];
    for (from, to) in edges.keys() {
        let (a, b) = (index[from], index[to]);
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
        incoming[b] += 1;
    }
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..keys.len() {
        let root = find(&mut parent, i);
        clusters.entry(root).or_default().push(i);
    }
    let mut rows: Vec<ClusterRow> = clusters
        .values()
        .map(|members| {
            let changed = members
                .iter()
                .filter(|&&i| nodes[keys[i]].1 != DiffStatus::Same)
                .count();
            // Suggest the member with no incoming edge (a root), largest first.
            let entry = members
                .iter()
                .copied()
                .min_by_key(|&i| (incoming[i], std::cmp::Reverse(members.len()), keys[i]))
                .map(|i| keys[i].clone())
                .unwrap_or_default();
            ClusterRow {
                changed,
                size: members.len(),
                entry,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.changed.cmp(&a.changed).then(b.size.cmp(&a.size)));
    rows
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

fn paint(status: DiffStatus, text: &str, palette: &crate::theme::Palette) -> String {
    match palette.open(status, false) {
        Some(open) => format!("{open}{text}\x1b[0m"),
        None => text.to_string(),
    }
}

/// Render mermaid source through mermaid-text, painting and hyperlinking
/// the marked labels. Tries both flow directions and keeps whichever fits
/// the terminal best (narrow enough first, then shortest).
pub fn render_colored(
    source: &str,
    marks: &Marks,
    color: bool,
    max_width: Option<usize>,
    palette: &crate::theme::Palette,
) -> anyhow::Result<String> {
    render_colored_flip(source, marks, color, max_width, true, palette)
}

pub fn render_colored_flip(
    source: &str,
    marks: &Marks,
    color: bool,
    max_width: Option<usize>,
    auto_flip: bool,
    palette: &crate::theme::Palette,
) -> anyhow::Result<String> {
    // Double-rendering to pick a direction is only worth it on small graphs.
    let rendered = if auto_flip && source.lines().count() <= 120 {
        best_layout(source, max_width)?
    } else {
        mermaid_text::render_with_width(source, max_width)
            .map_err(|error| anyhow::anyhow!("mermaid-text failed: {error}"))?
    };
    let mut sorted: Vec<&Mark> = marks.iter().collect();
    sorted.sort_by_key(|mark| std::cmp::Reverse(mark.label.chars().count()));
    let out = rendered
        .lines()
        .map(|line| {
            let mut line = line.to_string();
            for mark in &sorted {
                if let Some(at) = line.find(mark.label.as_str()) {
                    let painted = if color {
                        paint(mark.status, &mark.label, palette)
                    } else {
                        mark.label.clone()
                    };
                    let shown = match &mark.url {
                        Some(url) => {
                            format!("\x1b]8;;{url}\x1b\\{painted}\x1b]8;;\x1b\\")
                        }
                        None => painted,
                    };
                    if !color && mark.url.is_none() {
                        break;
                    }
                    line = format!("{}{}{}", &line[..at], shown, &line[at + mark.label.len()..]);
                    break;
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(out)
}

fn best_layout(source: &str, max_width: Option<usize>) -> anyhow::Result<String> {
    let render = |src: &str| {
        mermaid_text::render_with_width(src, max_width)
            .map_err(|error| anyhow::anyhow!("mermaid-text failed: {error}"))
    };
    let Some(flipped) = flip_direction(source) else {
        return render(source);
    };
    let first = render(source)?;
    let second = match render(&flipped) {
        Ok(second) => second,
        Err(_) => return Ok(first),
    };
    let measure = |text: &str| {
        let width = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
        let height = text.lines().count();
        (width, height)
    };
    let (w1, h1) = measure(&first);
    let (w2, h2) = measure(&second);
    let limit = max_width.unwrap_or(usize::MAX);
    let pick_second = match (w1 <= limit, w2 <= limit) {
        (true, false) => false,
        (false, true) => true,
        // Both fit (or neither): prefer the smaller footprint.
        _ => w2 * h2 < w1 * h1,
    };
    Ok(if pick_second { second } else { first })
}

fn flip_direction(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("flowchart LR") {
        Some(format!("flowchart TD{rest}"))
    } else {
        source
            .strip_prefix("flowchart TD")
            .map(|rest| format!("flowchart LR{rest}"))
    }
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

/// A node of the lineage graph, renderer-agnostic.
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub key: String,
    pub label: String,
    pub status: DiffStatus,
    pub data: bool,
    pub loc: Option<String>,
    pub url: Option<String>,
}

/// An edge of the lineage graph.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub status: DiffStatus,
}

/// Lineage view: the call DAG at data granularity. One node per function
/// (drawn once — convergence is visible as fan-in), data constructors as
/// stadium nodes, edges labeled with the binding the result lands in.
/// Branches flatten away; unresolved plumbing drops out entirely.
/// One connected cluster of an oversized lineage graph.
#[derive(Debug, Clone)]
pub struct ClusterRow {
    pub changed: usize,
    pub size: usize,
    pub entry: String,
}

/// What the lineage view decided to show: the graph, or — past the size
/// limit — its connected clusters with entry hints.
pub enum Lineage {
    Graph(Vec<GraphNode>, Vec<GraphEdge>),
    Overview(Vec<ClusterRow>),
}

pub fn lineage_graph(
    roots: &[DiffNode],
    link: Option<(&str, Option<&std::path::Path>)>,
    limit: Option<usize>,
) -> Option<Lineage> {
    #[derive(Default)]
    struct Graph {
        nodes: BTreeMap<String, (String, DiffStatus, bool, Option<String>)>,
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
        let mut text = node.key.clone();
        match &node.signature {
            // Full typed inputs; unresolved calls show their call-site
            // args; last resort is the declared-name label.
            Some(signature) => text.push_str(signature),
            None if !node.meta.args.is_empty() => {
                text.push('(');
                text.push_str(&node.meta.args.join(", "));
                text.push(')');
            }
            None => {
                if let Some(params) = node.label.strip_prefix(&node.key) {
                    text.push_str(params);
                }
            }
        }
        if let Some(ret) = &node.returns {
            text.push_str(" → ");
            text.push_str(ret);
        }
        clip(&text, 140)
    }

    fn walk(node: &DiffNode, ancestor: Option<&str>, graph: &mut Graph) {
        let own: Option<String> = if keep(node) {
            let data = is_data(node);
            let entry = graph
                .nodes
                .entry(node.key.clone())
                .or_insert_with(|| (label_of(node), node.status, data, node.location.clone()));
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

    if let Some(limit) = limit {
        if graph.nodes.len() > limit {
            return Some(Lineage::Overview(cluster_overview(
                &graph.nodes,
                &graph.edges,
            )));
        }
    }

    let nodes: Vec<GraphNode> = graph
        .nodes
        .iter()
        .map(|(key, (label, status, data, location))| {
            let url = match (link, location) {
                (Some((template, root)), Some(location)) => {
                    location.rsplit_once(':').map(|(path, line)| {
                        let absolute = match root {
                            Some(root) => root.join(path).to_string_lossy().into_owned(),
                            None => path.to_string(),
                        };
                        template
                            .replace("{path}", &absolute)
                            .replace("{line}", line)
                    })
                }
                _ => None,
            };
            GraphNode {
                key: key.clone(),
                label: label.clone(),
                status: *status,
                data: *data,
                loc: location.as_deref().map(crate::render::short_loc),
                url,
            }
        })
        .collect();
    let edge_list: Vec<GraphEdge> = graph
        .edges
        .iter()
        .map(|((from, to), (binding, status))| GraphEdge {
            from: from.clone(),
            to: to.clone(),
            label: binding.clone(),
            status: *status,
        })
        .collect();
    Some(Lineage::Graph(nodes, edge_list))
}
