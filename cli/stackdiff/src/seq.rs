//! Native sequence-diagram renderer: one lifeline per file, calls as
//! arrows in execution order, returns as dim replies, branch guards as
//! full-width separators — the house palette throughout.

use crate::theme::Palette;
use crate::types::{DiffNode, DiffStatus, NodeKind};

#[derive(Debug)]
pub enum Event {
    Message {
        from: usize,
        to: usize,
        text: String,
        reply: bool,
        status: DiffStatus,
    },
    Guard {
        text: String,
        status: DiffStatus,
    },
}

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

fn file_of(node: &DiffNode) -> Option<String> {
    node.location.as_deref().map(|location| {
        let path = location
            .rsplit_once(':')
            .map(|(p, _)| p)
            .unwrap_or(location);
        path.rsplit('/').next().unwrap_or(path).to_string()
    })
}

/// Flatten a diff tree into lifeline participants and ordered events.
pub fn sequence_events(root: &DiffNode) -> (Vec<String>, Vec<Event>) {
    let mut participants: Vec<String> = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    let home = file_of(root).unwrap_or_else(|| "«entry»".to_string());
    let home_id = intern(&mut participants, &home);
    walk(root, home_id, &mut participants, &mut events);
    (participants, events)
}

fn intern(participants: &mut Vec<String>, name: &str) -> usize {
    if let Some(at) = participants.iter().position(|p| p == name) {
        return at;
    }
    participants.push(name.to_string());
    participants.len() - 1
}

fn node_label(node: &DiffNode) -> String {
    let mut text = node.key.clone();
    match &node.signature {
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
    clip(&text, 60)
}

fn walk(node: &DiffNode, from: usize, participants: &mut Vec<String>, events: &mut Vec<Event>) {
    for child in &node.children {
        if child.key == "…" || child.key == "▸" {
            continue;
        }
        match child.kind {
            NodeKind::Branch => {
                events.push(Event::Guard {
                    text: clip(&child.label, 60),
                    status: child.status,
                });
                walk(child, from, participants, events);
            }
            NodeKind::Call => {
                let to = file_of(child)
                    .map(|file| intern(participants, &file))
                    .unwrap_or(from);
                events.push(Event::Message {
                    from,
                    to,
                    text: node_label(child),
                    reply: false,
                    status: child.status,
                });
                walk(child, to, participants, events);
                if let Some(returns) = &child.returns {
                    if to != from {
                        events.push(Event::Message {
                            from: to,
                            to: from,
                            text: clip(returns, 40),
                            reply: true,
                            status: child.status,
                        });
                    }
                }
            }
        }
    }
}

fn open(status: DiffStatus, dim: bool, color: bool, palette: &Palette) -> (String, String) {
    if !color {
        return (String::new(), String::new());
    }
    match palette.open(status, false) {
        Some(code) => (code, "\x1b[0m".to_string()),
        None if dim => ("\x1b[2m".to_string(), "\x1b[0m".to_string()),
        None => (String::new(), String::new()),
    }
}

/// Render lifelines + events to the terminal.
pub fn render_sequence(
    participants: &[String],
    events: &[Event],
    color: bool,
    palette: &Palette,
) -> String {
    let count = participants.len().max(1);
    // Lifeline x positions: boxes side by side with breathing room.
    let mut centers = Vec::with_capacity(count);
    let mut x = 1;
    for name in participants {
        let w = name.chars().count() + 4;
        centers.push(x + w / 2);
        x += w + 6;
    }
    let width = x + 2;

    let mut lines: Vec<String> = Vec::new();
    let dim_open = if color { "\x1b[2m" } else { "" };
    let reset = if color { "\x1b[0m" } else { "" };

    let boxes_row = |lines: &mut Vec<String>| {
        let mut top = vec![' '; width];
        let mut mid = vec![' '; width];
        let mut bottom = vec![' '; width];
        for (name, &cx) in participants.iter().zip(&centers) {
            let w = name.chars().count() + 4;
            let left = cx - w / 2;
            top[left] = '╭';
            bottom[left] = '╰';
            for i in left + 1..left + w - 1 {
                top[i] = '─';
                bottom[i] = '─';
            }
            top[left + w - 1] = '╮';
            bottom[left + w - 1] = '╯';
            mid[left] = '│';
            mid[left + w - 1] = '│';
            for (offset, ch) in name.chars().enumerate() {
                mid[left + 2 + offset] = ch;
            }
        }
        for row in [top, mid, bottom] {
            lines.push(row.into_iter().collect::<String>().trim_end().to_string());
        }
    };

    // A blank canvas row with the lifelines stamped in, dim.
    let lifeline_row = |fill: &[(usize, usize, char)]| -> Vec<char> {
        let mut row = vec![' '; width];
        for &cx in &centers {
            row[cx] = '┆';
        }
        for &(from, to, ch) in fill {
            let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
            for cell in row.iter_mut().take(hi).skip(lo + 1) {
                *cell = ch;
            }
        }
        row
    };

    boxes_row(&mut lines);

    for event in events {
        match event {
            Event::Guard { text, status } => {
                lines.push(String::new());
                let (o, c) = open(*status, true, color, palette);
                let bar: String = "─".repeat(3);
                lines.push(format!(
                    "{dim_open}{bar}{reset} {o}{text}{c} {dim_open}{bar}{reset}"
                ));
            }
            Event::Message {
                from,
                to,
                text,
                reply,
                status,
            } => {
                let (o, c) = open(*status, *reply, color, palette);
                let fx = centers[*from];
                let tx = centers[*to];
                // text row: label near the source lifeline
                let text_start = if from == to || fx <= tx {
                    fx + 2
                } else {
                    tx + 2
                };
                let mut text_row = lifeline_row(&[]);
                for (offset, ch) in text.chars().enumerate() {
                    if text_start + offset < width {
                        text_row[text_start + offset] = ch;
                    }
                }
                lines.push(paint_row(
                    &text_row,
                    &centers,
                    o.as_str(),
                    c.as_str(),
                    dim_open,
                    reset,
                    text_start,
                    text.chars().count(),
                ));

                if from == to {
                    // self message: a small hook off the lifeline
                    let mut row = lifeline_row(&[]);
                    row[fx] = '├';
                    row[fx + 1] = '─';
                    row[fx + 2] = '╮';
                    let mut back = lifeline_row(&[]);
                    back[fx] = '◀';
                    back[fx + 1] = '─';
                    back[fx + 2] = '╯';
                    lines.push(paint_row(
                        &row,
                        &centers,
                        o.as_str(),
                        c.as_str(),
                        dim_open,
                        reset,
                        fx,
                        3,
                    ));
                    lines.push(paint_row(
                        &back,
                        &centers,
                        o.as_str(),
                        c.as_str(),
                        dim_open,
                        reset,
                        fx,
                        3,
                    ));
                } else {
                    let ch = if *reply { '┄' } else { '─' };
                    let mut row = lifeline_row(&[(fx, tx, ch)]);
                    if fx < tx {
                        row[tx - 1] = '▶';
                        row[fx] = '├';
                    } else {
                        row[tx + 1] = '◀';
                        row[fx] = '┤';
                    }
                    let (lo, len) = if fx < tx {
                        (fx, tx - fx)
                    } else {
                        (tx, fx - tx)
                    };
                    lines.push(paint_row(
                        &row,
                        &centers,
                        o.as_str(),
                        c.as_str(),
                        dim_open,
                        reset,
                        lo,
                        len + 1,
                    ));
                }
            }
        }
    }

    lines.push(
        lifeline_row(&[])
            .into_iter()
            .collect::<String>()
            .trim_end()
            .to_string(),
    );
    boxes_row(&mut lines);

    if color {
        // dim the bare lifeline rows' ┆ cells
        lines
            .iter()
            .map(|l| l.replace('┆', &format!("{dim_open}┆{reset}")))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        lines.join("\n")
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
    row: &[char],
    _centers: &[usize],
    open: &str,
    close: &str,
    _dim: &str,
    _reset: &str,
    span_start: usize,
    span_len: usize,
) -> String {
    let mut out = String::new();
    for (i, ch) in row.iter().enumerate() {
        if i == span_start && !open.is_empty() {
            out.push_str(open);
        }
        out.push(*ch);
        if i == span_start + span_len && !close.is_empty() {
            out.push_str(close);
        }
    }
    out.trim_end().to_string()
}
