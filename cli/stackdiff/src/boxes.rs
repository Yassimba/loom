//! Boxed-graph terminal view (beautiful-mermaid style): each node a rounded
//! Unicode box, children fanned out beneath on a connector bus, the whole
//! box colored by diff status — green added, red removed, yellow changed.

use owo_colors::OwoColorize;

use crate::render::node_text;
use crate::types::{DiffNode, DiffStatus};

const MAX_LABEL: usize = 38;
const GAP: usize = 2;
const CONNECT_ROWS: usize = 3;

struct Layout {
    lines: Vec<String>,
    status: DiffStatus,
    box_w: usize,
    box_h: usize,
    sub_w: usize,
    sub_h: usize,
    children: Vec<Layout>,
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if candidate_len > width && !current.is_empty() {
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

fn build(node: &DiffNode) -> Layout {
    let lines = wrap(&node_text(node, true), MAX_LABEL);
    let box_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4;
    let box_h = lines.len() + 2;
    let children: Vec<Layout> = node.children.iter().map(build).collect();
    let kids_w: usize =
        children.iter().map(|c| c.sub_w).sum::<usize>() + GAP * children.len().saturating_sub(1);
    let sub_w = box_w.max(kids_w);
    let sub_h = box_h
        + children
            .iter()
            .map(|c| c.sub_h)
            .max()
            .map(|h| h + CONNECT_ROWS)
            .unwrap_or(0);
    Layout {
        lines,
        status: node.status,
        box_w,
        box_h,
        sub_w,
        sub_h,
        children,
    }
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    status: DiffStatus,
}

struct Canvas {
    rows: Vec<Vec<Cell>>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas {
            rows: vec![
                vec![
                    Cell {
                        ch: ' ',
                        status: DiffStatus::Same
                    };
                    w
                ];
                h
            ],
        }
    }

    fn put(&mut self, x: usize, y: usize, ch: char, status: DiffStatus) {
        if let Some(cell) = self.rows.get_mut(y).and_then(|row| row.get_mut(x)) {
            *cell = Cell { ch, status };
        }
    }
}

fn draw_box(canvas: &mut Canvas, layout: &Layout, x: usize, y: usize) {
    let status = layout.status;
    let w = layout.box_w;
    canvas.put(x, y, '╭', status);
    canvas.put(x + w - 1, y, '╮', status);
    canvas.put(x, y + layout.box_h - 1, '╰', status);
    canvas.put(x + w - 1, y + layout.box_h - 1, '╯', status);
    for col in x + 1..x + w - 1 {
        canvas.put(col, y, '─', status);
        canvas.put(col, y + layout.box_h - 1, '─', status);
    }
    for (index, line) in layout.lines.iter().enumerate() {
        let row = y + 1 + index;
        canvas.put(x, row, '│', status);
        canvas.put(x + w - 1, row, '│', status);
        let pad = (w - 2 - line.chars().count()) / 2;
        for (offset, ch) in line.chars().enumerate() {
            canvas.put(x + 1 + pad + offset, row, ch, status);
        }
    }
}

fn place(canvas: &mut Canvas, layout: &Layout, x: usize, y: usize) {
    let node_x = x + (layout.sub_w - layout.box_w) / 2;
    draw_box(canvas, layout, node_x, y);
    if layout.children.is_empty() {
        return;
    }

    let parent_cx = node_x + layout.box_w / 2;
    let kids_w: usize =
        layout.children.iter().map(|c| c.sub_w).sum::<usize>() + GAP * (layout.children.len() - 1);
    let mut child_x = x + (layout.sub_w - kids_w) / 2;
    let child_y = y + layout.box_h + CONNECT_ROWS;

    let stem_row = y + layout.box_h;
    let bus_row = stem_row + 1;
    let drop_row = stem_row + 2;

    canvas.put(parent_cx, stem_row, '│', layout.status);

    let centers: Vec<(usize, DiffStatus)> = {
        let mut centers = Vec::new();
        let mut cx = child_x;
        for child in &layout.children {
            centers.push((cx + child.sub_w / 2, child.status));
            cx += child.sub_w + GAP;
        }
        centers
    };
    let left = centers
        .first()
        .map(|(c, _)| *c)
        .unwrap_or(parent_cx)
        .min(parent_cx);
    let right = centers
        .last()
        .map(|(c, _)| *c)
        .unwrap_or(parent_cx)
        .max(parent_cx);
    for col in left..=right {
        canvas.put(col, bus_row, '─', DiffStatus::Same);
    }
    for &(cx, status) in &centers {
        let junction = if cx == left && cx < parent_cx {
            '╭'
        } else if cx == right && cx > parent_cx {
            '╮'
        } else if cx == parent_cx {
            '│'
        } else {
            '┬'
        };
        canvas.put(cx, bus_row, junction, status);
        canvas.put(cx, drop_row, '▼', status);
    }
    if left < parent_cx && parent_cx < right {
        canvas.put(parent_cx, bus_row, '┴', layout.status);
    } else if parent_cx == left && centers.iter().all(|(c, _)| *c != parent_cx) {
        canvas.put(parent_cx, bus_row, '╰', layout.status);
    } else if parent_cx == right && centers.iter().all(|(c, _)| *c != parent_cx) {
        canvas.put(parent_cx, bus_row, '╯', layout.status);
    }

    for child in &layout.children {
        place(canvas, child, child_x, child_y);
        child_x += child.sub_w + GAP;
    }
}

fn paint(status: DiffStatus, text: &str, color: bool) -> String {
    if !color || text.trim().is_empty() {
        return text.to_string();
    }
    match status {
        DiffStatus::Added => text.green().to_string(),
        DiffStatus::Removed => text.red().to_string(),
        DiffStatus::Changed => text.yellow().to_string(),
        DiffStatus::Same => text.to_string(),
    }
}

pub fn render_boxes(root: &DiffNode, color: bool) -> String {
    let layout = build(root);
    let mut canvas = Canvas::new(layout.sub_w, layout.sub_h);
    place(&mut canvas, &layout, 0, 0);

    let mut out = Vec::new();
    for row in &canvas.rows {
        let mut line = String::new();
        let mut run = String::new();
        let mut run_status = DiffStatus::Same;
        for cell in row {
            if cell.status != run_status {
                line.push_str(&paint(run_status, &run, color));
                run.clear();
                run_status = cell.status;
            }
            run.push(cell.ch);
        }
        line.push_str(&paint(run_status, &run, color));
        out.push(line.trim_end().to_string());
    }
    out.join("\n")
}
