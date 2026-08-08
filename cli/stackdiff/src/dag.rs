//! Layered DAG renderer for the lineage view: the -m box aesthetic
//! (padding, status tints, badges, dim frames, clickable locations) on a
//! Sugiyama-style layout — longest-path layers, barycenter ordering, and
//! orthogonal edges with one lane per edge so nothing overlaps.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::theme::Palette;
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
    /// Rail connectivity bits: 1=up 2=down 4=left 8=right. The glyph for a
    /// rail cell derives from this union, so crossings and junctions can
    /// never erase each other.
    mask: u8,
}

fn mask_char(mask: u8) -> char {
    match mask {
        0b0011 => '│',
        0b1100 => '─',
        0b1010 => '╭',
        0b0110 => '╮',
        0b1001 => '╰',
        0b0101 => '╯',
        0b1011 => '├',
        0b0111 => '┤',
        0b1110 => '┬',
        0b1101 => '┴',
        0b1111 => '┼',
        0b0001 | 0b0010 => '│',
        0b0100 | 0b1000 => '─',
        _ => ' ',
    }
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
                        mask: 0,
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
            *cell = Cell {
                ch,
                status,
                role,
                mask: 0,
            };
        }
    }

    /// Union rail connectivity into a cell; boxes are untouchable.
    fn conn(&mut self, x: usize, y: usize, bits: u8, status: DiffStatus) {
        let Some(cell) = self.rows.get_mut(y).and_then(|row| row.get_mut(x)) else {
            return;
        };
        if cell.role != Role::Rail {
            return;
        }
        cell.mask |= bits;
        if cell.status == DiffStatus::Same {
            cell.status = status;
        }
    }

    /// Draw an orthogonal polyline through waypoints, unioning direction
    /// bits cell by cell.
    fn path(&mut self, points: &[(usize, usize)], status: DiffStatus) {
        const U: u8 = 1;
        const D: u8 = 2;
        const L: u8 = 4;
        const R: u8 = 8;
        for pair in points.windows(2) {
            let ((x1, y1), (x2, y2)) = (pair[0], pair[1]);
            if y1 == y2 {
                let (lo, hi) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
                for x in lo..=hi {
                    let mut bits = 0;
                    if x > lo {
                        bits |= L;
                    }
                    if x < hi {
                        bits |= R;
                    }
                    self.conn(x, y1, bits, status);
                }
            } else {
                let (lo, hi) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
                for y in lo..=hi {
                    let mut bits = 0;
                    if y > lo {
                        bits |= U;
                    }
                    if y < hi {
                        bits |= D;
                    }
                    self.conn(x1, y, bits, status);
                }
            }
        }
        // Stitch the corners: each interior waypoint needs both directions.
        for triple in points.windows(3) {
            let ((x1, y1), (x2, y2), (x3, y3)) = (triple[0], triple[1], triple[2]);
            let mut bits = 0;
            if y1 != y2 {
                bits |= if y1 < y2 { U } else { D };
            }
            if x1 != x2 {
                bits |= if x1 < x2 { L } else { R };
            }
            if y3 != y2 {
                bits |= if y3 < y2 { U } else { D };
            }
            if x3 != x2 {
                bits |= if x3 < x2 { L } else { R };
            }
            self.conn(x2, y2, bits, status);
        }
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
    // Hard-break words a wrap can't split (long dotted identifiers).
    let mut broken = Vec::new();
    for line in lines {
        if line.chars().count() <= width {
            broken.push(line);
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        for chunk in chars.chunks(width) {
            broken.push(chunk.iter().collect());
        }
    }
    if broken.is_empty() {
        broken.push(String::new());
    }
    broken
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

fn paint(status: DiffStatus, role: Role, color: bool, palette: &Palette) -> (String, String) {
    if !color {
        return (String::new(), String::new());
    }
    let inside = matches!(
        role,
        Role::Frame | Role::Fill | Role::Text | Role::Loc | Role::Badge
    );
    let prefix = match palette.open(status, inside) {
        Some(open) => open,
        None => match role {
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

/// Render the lineage graph left→right with layered layout, slimming
/// boxes (drop locations, tighten label wrap) until it fits the terminal
/// so rows never wrap into visual debris.
pub fn render_dag(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    color: bool,
    max_width: Option<usize>,
    palette: &Palette,
) -> String {
    const PROFILES: [(usize, bool); 4] = [
        (MAX_LABEL, true),
        (MAX_LABEL, false),
        (26, false),
        (20, false),
    ];
    let mut rendered = render_dag_profile(nodes, edges, color, PROFILES[0], palette);
    if let Some(limit) = max_width {
        for profile in &PROFILES[1..] {
            let width = rendered.lines().map(visible_width).max().unwrap_or(0);
            if width <= limit {
                break;
            }
            rendered = render_dag_profile(nodes, edges, color, *profile, palette);
        }
    }
    rendered
}

/// Printable width of an emitted line (ANSI/OSC sequences excluded).
fn visible_width(line: &str) -> usize {
    let mut width = 0;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC ... terminated by ESC \ or BEL
                    let mut prev = ' ';
                    for c in chars.by_ref() {
                        if c == '\u{7}' || (prev == '\x1b' && c == '\\') {
                            break;
                        }
                        prev = c;
                    }
                }
                _ => {}
            }
        } else {
            width += 1;
        }
    }
    width
}

fn render_dag_profile(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    color: bool,
    (max_label, show_loc): (usize, bool),
    palette: &Palette,
) -> String {
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
            let lines: Vec<String> = node
                .label
                .split('\n')
                .flat_map(|part| wrap(part, max_label))
                .collect();
            let loc = node.loc.as_ref().filter(|_| show_loc);
            let content = lines
                .iter()
                .map(|l| l.chars().count())
                .chain(loc.iter().map(|l| l.chars().count()))
                .max()
                .unwrap_or(0);
            Item {
                real: Some(i),
                layer: layer[i],
                w: content + 4,
                h: lines.len() + 2 + usize::from(loc.is_some()),
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
    {
        let mut sources: HashSet<(usize, usize)> = HashSet::new();
        for segment in &segments {
            if sources.insert((items[segment.b].layer, segment.a)) {
                lanes[items[segment.b].layer] += 1;
            }
        }
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
                lines: node
                    .label
                    .split('\n')
                    .flat_map(|part| wrap(part, max_label))
                    .collect(),
                loc: node.loc.clone().filter(|_| show_loc),
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

    // Rails: one bus per (source, gap): each target is a polyline
    // source → lane → target; shared trunk cells union into junctions.
    let mut groups: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (i, segment) in segments.iter().enumerate() {
        groups
            .entry((items[segment.b].layer, segment.a))
            .or_default()
            .push(i);
    }
    struct Finish {
        x: usize,
        y: usize,
        status: DiffStatus,
        label: Option<String>,
        label_from: usize,
        arrow: bool,
    }
    let mut finishes: Vec<Finish> = Vec::new();
    let mut ordered: Vec<(&(usize, usize), &Vec<usize>)> = groups.iter().collect();
    ordered.sort_by_key(|((gap, a), _)| (*gap, items[*a].y));
    let mut lane_cursor: BTreeMap<usize, usize> = BTreeMap::new();
    for ((gap, a), members) in ordered {
        let source = &items[*a];
        let sy = source.y + source.h / 2;
        let start_x = source.x + source.w;
        let cursor = lane_cursor.entry(*gap).or_insert(0);
        let lane_x = col_x[*gap] - 3 - (*cursor) * 2 - 1;
        *cursor += 1;

        for &i in members {
            let segment = &segments[i];
            let b = &items[segment.b];
            let ty = b.y + b.h / 2;
            let status = segment.status;
            let end_x = if segment.last { b.x } else { b.x + 1 };
            let tip = end_x.saturating_sub(1);
            canvas.path(
                &[(start_x, sy), (lane_x, sy), (lane_x, ty), (tip, ty)],
                status,
            );
            finishes.push(Finish {
                x: end_x,
                y: ty,
                status,
                label: segment.last.then(|| segment.label.clone()).flatten(),
                label_from: lane_x + 2,
                arrow: segment.last,
            });
        }
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
            let (open, close) = paint(key.0, key.1, color, palette);
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
            if cell.role == Role::Rail && cell.mask != 0 {
                run.push(mask_char(cell.mask));
            } else {
                run.push(cell.ch);
            }
        }
        flush(&mut line, &mut run, run_key);
        out.push(line.trim_end().to_string());
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}
