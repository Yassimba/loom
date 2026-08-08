//! Layered DAG renderer for the lineage view: the -m box aesthetic
//! (padding, status tints, badges, dim frames, clickable locations) on a
//! Sugiyama-style layout — longest-path layers, barycenter ordering, and
//! orthogonal edges with one lane per edge so nothing overlaps.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::types::DiffStatus;
use crate::views::{GraphEdge, GraphNode};

const MAX_LABEL: usize = 34;
const NODE_GAP: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Frame,
    Fill,
    Text,
    Loc,
    Rail,
    Arrow,
    Badge,
    Edge,
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    status: DiffStatus,
    role: Role,
}

struct Canvas {
    rows: Vec<Vec<Cell>>,
    links: Vec<(usize, usize, usize, String)>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas {
            rows: vec![
                vec![
                    Cell {
                        ch: ' ',
                        status: DiffStatus::Same,
                        role: Role::Rail,
                    };
                    w
                ];
                h
            ],
            links: Vec::new(),
        }
    }

    fn put(&mut self, x: usize, y: usize, ch: char, status: DiffStatus, role: Role) {
        if let Some(cell) = self.rows.get_mut(y).and_then(|row| row.get_mut(x)) {
            *cell = Cell { ch, status, role };
        }
    }

    /// Draw a rail char, merging with what's already there at crossings.
    fn rail(&mut self, x: usize, y: usize, ch: char, status: DiffStatus) {
        let Some(cell) = self.rows.get_mut(y).and_then(|row| row.get_mut(x)) else {
            return;
        };
        if cell.role != Role::Rail && cell.role != Role::Edge && cell.ch != ' ' {
            return; // never draw over boxes
        }
        let merged = match (cell.ch, ch) {
            (' ', c) => c,
            ('─', '│') | ('│', '─') => '┼',
            ('─', c) | (c, '─') if "╭╮╰╯".contains(c) => '┼',
            (a, b) if a == b => a,
            (_, c) => c,
        };
        cell.ch = merged;
        if cell.status == DiffStatus::Same {
            cell.status = status;
        }
        cell.role = Role::Rail;
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if candidate > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct Placed {
    lines: Vec<String>,
    loc: Option<String>,
    url: Option<String>,
    status: DiffStatus,
    data: bool,
    w: usize,
    h: usize,
    layer: usize,
    /// y of the box top, x of the box left
    x: usize,
    y: usize,
}

/// Longest-path layering over the DAG (back edges ignored for layering).
fn layers(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<usize> {
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.as_str(), i))
        .collect();
    let mut forward: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut indegree = vec![0usize; nodes.len()];
    let mut seen = HashSet::new();
    for edge in edges {
        let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        if a == b || !seen.insert((a, b)) {
            continue;
        }
        forward[a].push(b);
        indegree[b] += 1;
    }
    // Kahn; cycle leftovers get appended at their current depth.
    let mut layer = vec![0usize; nodes.len()];
    let mut queue: Vec<usize> = (0..nodes.len()).filter(|&i| indegree[i] == 0).collect();
    let mut remaining = indegree.clone();
    let mut order = Vec::new();
    while let Some(node) = queue.pop() {
        order.push(node);
        for &next in &forward[node] {
            layer[next] = layer[next].max(layer[node] + 1);
            remaining[next] -= 1;
            if remaining[next] == 0 {
                queue.push(next);
            }
        }
    }
    if order.len() < nodes.len() {
        for i in 0..nodes.len() {
            if !order.contains(&i) {
                layer[i] = layer.iter().copied().max().unwrap_or(0);
            }
        }
    }
    layer
}

/// Order nodes within layers by barycenter of their neighbors.
fn ordering(nodes: &[GraphNode], edges: &[GraphEdge], layer: &[usize]) -> Vec<Vec<usize>> {
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.as_str(), i))
        .collect();
    let max_layer = layer.iter().copied().max().unwrap_or(0);
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for (i, &l) in layer.iter().enumerate() {
        columns[l].push(i);
    }
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    for _ in 0..3 {
        let position: HashMap<usize, usize> = columns
            .iter()
            .flat_map(|column| column.iter().enumerate().map(|(pos, &n)| (n, pos)))
            .collect();
        for column in &mut columns {
            column.sort_by_key(|&n| {
                let hood = &neighbors[n];
                if hood.is_empty() {
                    return (position.get(&n).copied().unwrap_or(0) * 100) as i64;
                }
                let sum: usize = hood
                    .iter()
                    .map(|other| position.get(other).copied().unwrap_or(0) * 100)
                    .sum();
                (sum / hood.len()) as i64
            });
        }
    }
    columns
}

fn paint(status: DiffStatus, role: Role, color: bool) -> (String, String) {
    if !color {
        return (String::new(), String::new());
    }
    const ADDED: ((u8, u8, u8), (u8, u8, u8)) = ((63, 185, 80), (14, 40, 22));
    const REMOVED: ((u8, u8, u8), (u8, u8, u8)) = ((248, 81, 73), (45, 17, 17));
    const CHANGED: ((u8, u8, u8), (u8, u8, u8)) = ((210, 153, 34), (43, 35, 10));
    let inside = matches!(
        role,
        Role::Frame | Role::Fill | Role::Text | Role::Loc | Role::Badge
    );
    let tint = |((fr, fg, fb), (br, bg, bb)): ((u8, u8, u8), (u8, u8, u8))| {
        if inside {
            format!("\x1b[38;2;{fr};{fg};{fb}m\x1b[48;2;{br};{bg};{bb}m")
        } else {
            format!("\x1b[38;2;{fr};{fg};{fb}m")
        }
    };
    let prefix = match status {
        DiffStatus::Added => tint(ADDED),
        DiffStatus::Removed => tint(REMOVED),
        DiffStatus::Changed => tint(CHANGED),
        DiffStatus::Same => match role {
            Role::Frame | Role::Rail | Role::Loc => "\x1b[2m".to_string(),
            Role::Edge => "\x1b[2;3m".to_string(),
            _ => String::new(),
        },
    };
    let prefix = match (status, role) {
        (DiffStatus::Same, _) => prefix,
        (_, Role::Loc) => format!("{prefix}\x1b[2m"),
        (_, Role::Edge) => format!("\x1b[3m{prefix}"),
        _ => prefix,
    };
    if prefix.is_empty() {
        (String::new(), String::new())
    } else {
        (prefix, "\x1b[0m".to_string())
    }
}

fn badge(status: DiffStatus) -> Option<char> {
    match status {
        DiffStatus::Added => Some('+'),
        DiffStatus::Removed => Some('−'),
        DiffStatus::Changed => Some('!'),
        DiffStatus::Same => None,
    }
}

fn draw_box(canvas: &mut Canvas, node: &Placed) {
    let status = node.status;
    let (h, v) = if node.data {
        ('┄', '┆')
    } else {
        ('─', '│')
    };
    canvas.put(node.x, node.y, '╭', status, Role::Frame);
    canvas.put(node.x + node.w - 1, node.y, '╮', status, Role::Frame);
    canvas.put(node.x, node.y + node.h - 1, '╰', status, Role::Frame);
    canvas.put(
        node.x + node.w - 1,
        node.y + node.h - 1,
        '╯',
        status,
        Role::Frame,
    );
    for col in node.x + 1..node.x + node.w - 1 {
        canvas.put(col, node.y, h, status, Role::Frame);
        canvas.put(col, node.y + node.h - 1, h, status, Role::Frame);
    }
    for row in node.y + 1..node.y + node.h - 1 {
        canvas.put(node.x, row, v, status, Role::Frame);
        canvas.put(node.x + node.w - 1, row, v, status, Role::Frame);
        for col in node.x + 1..node.x + node.w - 1 {
            canvas.put(col, row, ' ', status, Role::Fill);
        }
    }
    if let Some(glyph) = badge(status) {
        canvas.put(node.x + 2, node.y, ' ', status, Role::Badge);
        canvas.put(node.x + 3, node.y, glyph, status, Role::Badge);
        canvas.put(node.x + 4, node.y, ' ', status, Role::Badge);
    }
    let mut row = node.y + 1;
    for line in &node.lines {
        let pad = (node.w - 2 - line.chars().count()) / 2;
        for (offset, ch) in line.chars().enumerate() {
            canvas.put(node.x + 1 + pad + offset, row, ch, status, Role::Text);
        }
        row += 1;
    }
    if let Some(loc) = &node.loc {
        let pad = (node.w - 2 - loc.chars().count()) / 2;
        for (offset, ch) in loc.chars().enumerate() {
            canvas.put(node.x + 1 + pad + offset, row, ch, status, Role::Loc);
        }
        if let Some(url) = &node.url {
            canvas.links.push((
                row,
                node.x + 1 + pad,
                node.x + 1 + pad + loc.chars().count(),
                url.clone(),
            ));
        }
    }
}

/// Render the lineage graph left→right with layered layout.
pub fn render_dag(nodes: &[GraphNode], edges: &[GraphEdge], color: bool) -> String {
    let layer = layers(nodes, edges);
    let columns = ordering(nodes, edges, &layer);

    // Box shapes
    let mut placed: Vec<Placed> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let lines = wrap(&node.label, MAX_LABEL);
            let content = lines
                .iter()
                .map(|l| l.chars().count())
                .chain(node.loc.iter().map(|l| l.chars().count()))
                .max()
                .unwrap_or(0);
            Placed {
                w: content + 4,
                h: lines.len() + 2 + usize::from(node.loc.is_some()),
                lines,
                loc: node.loc.clone(),
                url: node.url.clone(),
                status: node.status,
                data: node.data,
                layer: layer[i],
                x: 0,
                y: 0,
            }
        })
        .collect();

    // Corridor lanes: one per edge crossing each gap.
    let max_layer = columns.len();
    let mut gap_lanes = vec![0usize; max_layer + 1];
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.as_str(), i))
        .collect();
    let mut edge_refs: Vec<(usize, usize, &GraphEdge)> = Vec::new();
    for edge in edges {
        let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        edge_refs.push((a, b, edge));
        let (lo, hi) = if layer[a] <= layer[b] {
            (layer[a], layer[b])
        } else {
            (layer[b], layer[a])
        };
        for gap in lo..hi.max(lo + 1) {
            gap_lanes[gap + 1] += 1;
        }
    }

    // Column x positions.
    let mut col_x = vec![0usize; max_layer];
    let mut col_w = vec![0usize; max_layer];
    for (l, column) in columns.iter().enumerate() {
        col_w[l] = column.iter().map(|&n| placed[n].w).max().unwrap_or(0);
    }
    let mut x = 0;
    for l in 0..max_layer {
        col_x[l] = x;
        let lanes = gap_lanes.get(l + 1).copied().unwrap_or(0);
        x += col_w[l] + 3 + lanes * 2 + 3;
    }

    // Node y positions per column.
    for column in &columns {
        let mut y = 0;
        for &n in column {
            placed[n].y = y;
            y += placed[n].h + NODE_GAP + 1;
        }
    }
    for (l, column) in columns.iter().enumerate() {
        for &n in column {
            // center within column width
            placed[n].x = col_x[l] + (col_w[l] - placed[n].w) / 2;
        }
    }

    let width = x + 2;
    let height = placed.iter().map(|p| p.y + p.h).max().unwrap_or(1) + 2;
    let mut canvas = Canvas::new(width, height);

    // Edges under boxes: draw rails first is wrong (boxes protect themselves
    // via rail()), draw boxes first then rails that refuse to overwrite.
    for node in &placed {
        draw_box(&mut canvas, node);
    }

    // Assign lanes per gap in target-y order for tidy fanouts.
    let mut lane_cursor: BTreeMap<usize, usize> = BTreeMap::new();
    let mut sorted_edges: Vec<&(usize, usize, &GraphEdge)> = edge_refs.iter().collect();
    sorted_edges.sort_by_key(|(_, b, _)| (placed[*b].layer, placed[*b].y));

    for (a, b, edge) in sorted_edges {
        let (src, dst) = (&placed[*a], &placed[*b]);
        if src.layer >= dst.layer {
            continue; // back edges: convergence already shows fan-in; skip rails
        }
        let status = edge.status;
        let sy = src.y + src.h / 2;
        let dy = dst.y + dst.h / 2;
        let start_x = src.x + src.w;
        // lane in the gap just before dst's column
        let gap = dst.layer;
        let lanes_before: usize = gap_lanes.iter().take(gap).sum();
        let cursor = lane_cursor.entry(gap).or_insert(0);
        let lane_index = *cursor;
        *cursor += 1;
        let lane_x = col_x[gap].saturating_sub(3 + (gap_lanes[gap] - lane_index) * 2 - 2);
        let _ = lanes_before;
        // horizontal from source to lane (may pass through intermediate cols;
        // rails refuse to enter boxes, so runs clip visually at obstacles)
        let y_run = sy;
        for cx in start_x..lane_x {
            canvas.rail(cx, y_run, '─', status);
        }
        // vertical along the lane
        let (top, bottom) = if y_run <= dy {
            (y_run, dy)
        } else {
            (dy, y_run)
        };
        for cy in top..=bottom {
            canvas.rail(lane_x, cy, '│', status);
        }
        canvas.rail(lane_x, y_run, if y_run <= dy { '╮' } else { '╯' }, status);
        canvas.rail(lane_x, dy, if y_run <= dy { '╰' } else { '╭' }, status);
        if y_run == dy {
            canvas.rail(lane_x, dy, '─', status);
        }
        // horizontal into the target
        let end_x = dst.x;
        for cx in lane_x + 1..end_x.saturating_sub(1) {
            canvas.rail(cx, dy, '─', status);
        }
        if let Some(label) = &edge.label {
            let text: String = label.chars().take(14).collect();
            let space = end_x.saturating_sub(lane_x + 2);
            if text.chars().count() + 1 < space {
                for (offset, ch) in text.chars().enumerate() {
                    canvas.put(lane_x + 2 + offset, dy, ch, status, Role::Edge);
                }
            }
        }
        canvas.put(end_x.saturating_sub(1), dy, '▶', status, Role::Arrow);
    }

    // Emit with runs + links.
    let mut out = Vec::new();
    for (row_index, row) in canvas.rows.iter().enumerate() {
        let mut line = String::new();
        let mut run = String::new();
        let mut run_key = (DiffStatus::Same, Role::Rail);
        let links: Vec<&(usize, usize, usize, String)> = canvas
            .links
            .iter()
            .filter(|(r, _, _, _)| *r == row_index)
            .collect();
        let flush = |line: &mut String, run: &mut String, key: (DiffStatus, Role)| {
            if run.is_empty() {
                return;
            }
            let (open, close) = paint(key.0, key.1, color);
            line.push_str(&open);
            line.push_str(run);
            line.push_str(&close);
            run.clear();
        };
        for (col, cell) in row.iter().enumerate() {
            for (_, start, end, url) in &links {
                if *start == col {
                    flush(&mut line, &mut run, run_key);
                    line.push_str(&format!("\x1b]8;;{url}\x1b\\"));
                }
                if *end == col {
                    flush(&mut line, &mut run, run_key);
                    line.push_str("\x1b]8;;\x1b\\");
                }
            }
            if (cell.status, cell.role) != run_key {
                flush(&mut line, &mut run, run_key);
                run_key = (cell.status, cell.role);
            }
            run.push(cell.ch);
        }
        flush(&mut line, &mut run, run_key);
        out.push(line.trim_end().to_string());
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}
