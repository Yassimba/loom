//! Alternate diagram views generated from the call data: a sequence
//! diagram of who-messages-whom in execution order, and a class ("ER")
//! diagram of the types in play. Layout is delegated to mermaid-text;
//! diff colors are painted onto the rendered lines afterward, since we
//! know exactly which labels are added, removed, or changed.

use std::collections::BTreeMap;

use crate::types::{DiffNode, DiffStatus};

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
/// Class/ER view as a graph: one box per type with fields and touched
/// methods stacked inside, edges where one type's methods call another's.
pub fn class_graph(
    roots: &[DiffNode],
    types: &BTreeMap<String, Vec<(String, Option<String>)>>,
) -> Option<(Vec<GraphNode>, Vec<GraphEdge>)> {
    let mut methods: BTreeMap<String, BTreeMap<String, DiffStatus>> = BTreeMap::new();
    let mut edges: BTreeMap<(String, String), DiffStatus> = BTreeMap::new();
    for root in roots {
        collect_classes(root, type_of(&root.key), &mut methods, &mut edges);
    }
    if methods.is_empty() {
        return None;
    }
    let nodes: Vec<GraphNode> = methods
        .iter()
        .map(|(type_name, type_methods)| {
            let mut label = type_name.clone();
            for (field, field_type) in types.get(type_name).into_iter().flatten() {
                label.push('\n');
                match field_type {
                    Some(ty) => label.push_str(&format!("{field}: {ty}")),
                    None => label.push_str(field),
                }
            }
            let mut status = DiffStatus::Same;
            for (method, method_status) in type_methods {
                label.push('\n');
                label.push_str(&format!("+{method}()"));
                if status == DiffStatus::Same {
                    status = *method_status;
                }
            }
            GraphNode {
                key: type_name.clone(),
                label,
                status,
                data: false,
                loc: None,
                url: None,
            }
        })
        .collect();
    let edge_list: Vec<GraphEdge> = edges
        .iter()
        .map(|((from, to), status)| GraphEdge {
            from: from.clone(),
            to: to.clone(),
            label: None,
            status: *status,
        })
        .collect();
    Some((nodes, edge_list))
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

/// Module zoom as a graph: one node per file, an edge when a call
/// crosses files, statuses aggregated.
pub fn module_graph(roots: &[DiffNode]) -> Option<(Vec<GraphNode>, Vec<GraphEdge>)> {
    let mut edges: BTreeMap<(String, String), DiffStatus> = BTreeMap::new();
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
    for root in roots {
        collect(root, None, &mut edges);
    }
    if edges.is_empty() {
        return None;
    }
    let mut names: Vec<&String> = edges.keys().flat_map(|(from, to)| [from, to]).collect();
    names.sort();
    names.dedup();
    let nodes: Vec<GraphNode> = names
        .iter()
        .map(|name| GraphNode {
            key: (*name).clone(),
            label: (*name).clone(),
            status: DiffStatus::Same,
            data: false,
            loc: None,
            url: None,
        })
        .collect();
    let edge_list: Vec<GraphEdge> = edges
        .iter()
        .map(|((from, to), status)| GraphEdge {
            from: from.clone(),
            to: to.clone(),
            label: None,
            status: *status,
        })
        .collect();
    Some((nodes, edge_list))
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
