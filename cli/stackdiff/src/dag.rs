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

/// One placed thing: a real box, or a zero-width waypoint a long edge
/// bends through (classic Sugiyama virtual node).
struct Item {
    real: Option<usize>,
    layer: usize,
    w: usize,
    h: usize,
    x: usize,
    y: usize,
}

/// Render the lineage graph left→right with layered layout.
pub fn render_dag(nodes: &[GraphNode], edges: &[GraphEdge], color: bool) -> String {
    let layer = layers(nodes, edges);
    let index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.as_str(), i))
        .collect();

    // Items: every real node, plus virtual waypoints for layer-skipping
    // edges so each drawn segment spans exactly one gap.
    let mut items: Vec<Item> = nodes
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
            Item {
                real: Some(i),
                layer: layer[i],
                w: content + 4,
                h: lines.len() + 2 + usize::from(node.loc.is_some()),
                x: 0,
                y: 0,
            }
        })
        .collect();

    // Chains: per edge, the item ids it threads through.
    struct Segment {
        a: usize,
        b: usize,
        status: DiffStatus,
        /// label + arrow live on the final segment
        label: Option<String>,
        last: bool,
    }
    let mut segments: Vec<Segment> = Vec::new();
    for edge in edges {
        let (Some(&na), Some(&nb)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        let (la, lb) = (layer[na], layer[nb]);
        if la >= lb {
            continue; // back edges: fan-in already tells the story
        }
        let mut chain = vec![na];
        for l in la + 1..lb {
            items.push(Item {
                real: None,
                layer: l,
                w: 1,
                h: 1,
                x: 0,
                y: 0,
            });
            chain.push(items.len() - 1);
        }
        chain.push(nb);
        for pair in chain.windows(2) {
            segments.push(Segment {
                a: pair[0],
                b: pair[1],
                status: edge.status,
                label: edge.label.clone(),
                last: pair[1] == nb,
            });
        }
    }

    // Column membership + barycenter ordering over items.
    let max_layer = items.iter().map(|i| i.layer).max().unwrap_or(0);
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for (i, item) in items.iter().enumerate() {
        columns[item.layer].push(i);
    }
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    for segment in &segments {
        neighbors[segment.a].push(segment.b);
        neighbors[segment.b].push(segment.a);
    }
    for _ in 0..4 {
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

    // Geometry: column x, lanes per gap, item y.
    let mut lanes = vec![0usize; max_layer + 1];
    for segment in &segments {
        lanes[items[segment.b].layer] += 1;
    }
    let mut col_w = vec![0usize; max_layer + 1];
    for (l, column) in columns.iter().enumerate() {
        col_w[l] = column.iter().map(|&n| items[n].w).max().unwrap_or(1);
    }
    let mut col_x = vec![0usize; max_layer + 1];
    let mut x = 1;
    for l in 0..=max_layer {
        // corridor before this column holds this gap's lanes
        x += if l == 0 { 0 } else { 3 + lanes[l] * 2 + 2 };
        col_x[l] = x;
        x += col_w[l];
    }
    for column in &columns {
        let mut y = 0;
        for &n in column {
            items[n].y = y;
            y += items[n].h + NODE_GAP + 1;
        }
    }
    for (l, column) in columns.iter().enumerate() {
        for &n in column {
            let w = items[n].w;
            items[n].x = col_x[l] + (col_w[l] - w) / 2;
        }
    }

    let width = x + 2;
    let height = items.iter().map(|i| i.y + i.h).max().unwrap_or(1) + 1;
    let mut canvas = Canvas::new(width, height);

    // Boxes first (rails refuse to enter them).
    let placed: Vec<Placed> = items
        .iter()
        .filter_map(|item| {
            let i = item.real?;
            let node = &nodes[i];
            Some(Placed {
                lines: wrap(&node.label, MAX_LABEL),
                loc: node.loc.clone(),
                url: node.url.clone(),
                status: node.status,
                data: node.data,
                w: item.w,
                h: item.h,
                x: item.x,
                y: item.y,
            })
        })
        .collect();
    for node in &placed {
        draw_box(&mut canvas, node);
    }

    // Rails: every segment spans one gap; lanes assigned by target y.
    let mut sorted: Vec<usize> = (0..segments.len()).collect();
    sorted.sort_by_key(|&i| {
        let b = &items[segments[i].b];
        (b.layer, b.y, items[segments[i].a].y)
    });
    let mut lane_cursor: BTreeMap<usize, usize> = BTreeMap::new();
    struct Finish {
        x: usize,
        y: usize,
        status: DiffStatus,
        label: Option<String>,
        label_from: usize,
        arrow: bool,
    }
    let mut finishes: Vec<Finish> = Vec::new();
    for i in sorted {
        let segment = &segments[i];
        let (a, b) = (&items[segment.a], &items[segment.b]);
        let status = segment.status;
        let sy = a.y + a.h / 2;
        let dy = b.y + b.h / 2;
        let start_x = a.x + a.w;
        let gap = b.layer;
        let cursor = lane_cursor.entry(gap).or_insert(0);
        let lane_x = col_x[gap] - 3 - (*cursor) * 2 - 1;
        *cursor += 1;
        for cx in start_x..lane_x {
            canvas.rail(cx, sy, '─', status);
        }
        if sy == dy {
            canvas.rail(lane_x, dy, '─', status);
        } else {
            let (top, bottom) = if sy < dy { (sy, dy) } else { (dy, sy) };
            for cy in top + 1..bottom {
                canvas.rail(lane_x, cy, '│', status);
            }
            canvas.rail(lane_x, sy, if sy < dy { '╮' } else { '╯' }, status);
            canvas.rail(lane_x, dy, if sy < dy { '╰' } else { '╭' }, status);
        }
        let end_x = if segment.last { b.x } else { b.x + 1 };
        for cx in lane_x + 1..end_x.saturating_sub(1) {
            canvas.rail(cx, dy, '─', status);
        }
        finishes.push(Finish {
            x: end_x,
            y: dy,
            status,
            label: segment.last.then(|| segment.label.clone()).flatten(),
            label_from: lane_x + 2,
            arrow: segment.last,
        });
    }

    // Labels and arrowheads last, so crossing rails never eat them.
    for finish in &finishes {
        if finish.arrow {
            canvas.put(
                finish.x.saturating_sub(1),
                finish.y,
                '▶',
                finish.status,
                Role::Arrow,
            );
        }
        if let Some(label) = &finish.label {
            let text: String = label.chars().take(14).collect();
            let space = finish.x.saturating_sub(finish.label_from + 1);
            if text.chars().count() + 1 < space {
                canvas.put(finish.label_from, finish.y, ' ', finish.status, Role::Edge);
                for (offset, ch) in text.chars().enumerate() {
                    canvas.put(
                        finish.label_from + 1 + offset,
                        finish.y,
                        ch,
                        finish.status,
                        Role::Edge,
                    );
                }
                canvas.put(
                    finish.label_from + 1 + text.chars().count(),
                    finish.y,
                    ' ',
                    finish.status,
                    Role::Edge,
                );
            }
        }
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
