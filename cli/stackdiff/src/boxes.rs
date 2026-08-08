//! Boxed-graph terminal view: rounded Unicode boxes with status colors and
//! background tints, docs and clickable locations inside the box, dashed
//! borders for branch arms. Left→right by default so sibling fans grow
//! down the terminal instead of across it; top→down available via --dir.

use crate::render::{node_text, short_loc};
use crate::types::{DiffNode, DiffStatus, NodeKind};

const MAX_LABEL: usize = 38;

/// Degrees of slimming, tried in order until the graph fits the terminal.
#[derive(Clone, Copy)]
struct Profile {
    docs: bool,
    loc: bool,
    max_label: usize,
}

const PROFILES: [Profile; 5] = [
    Profile {
        docs: true,
        loc: true,
        max_label: MAX_LABEL,
    },
    Profile {
        docs: false,
        loc: true,
        max_label: MAX_LABEL,
    },
    Profile {
        docs: false,
        loc: false,
        max_label: MAX_LABEL,
    },
    Profile {
        docs: false,
        loc: false,
        max_label: 28,
    },
    Profile {
        docs: false,
        loc: false,
        max_label: 20,
    },
];
const GAP_TD: usize = 3;
const GAP_LR: usize = 1;
const CONNECT_TD: usize = 4;
const CONNECT_LR: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    LeftRight,
    TopDown,
}

#[derive(Debug, Clone, Default)]
pub struct BoxOptions {
    pub color: bool,
    pub dir: Direction,
    /// URL template with `{path}` and `{line}` holes for OSC 8 links.
    pub link: Option<String>,
    pub repo_root: Option<std::path::PathBuf>,
    /// Terminal width; wider layouts slim themselves down to fit so rows
    /// never wrap (wrapping shreds the boxes into stripes).
    pub max_width: Option<usize>,
}

// GitHub-dark diff palette: bright text over a subtle tint.
const ADDED: ((u8, u8, u8), (u8, u8, u8)) = ((63, 185, 80), (14, 40, 22));
const REMOVED: ((u8, u8, u8), (u8, u8, u8)) = ((248, 81, 73), (45, 17, 17));
const CHANGED: ((u8, u8, u8), (u8, u8, u8)) = ((210, 153, 34), (43, 35, 10));

/// What a cell is part of; decides styling within its status.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// Box borders — tinted for status boxes, dim for Same
    Frame,
    /// Interior background (spaces) — tinted, never dim
    Fill,
    /// The label text
    Text,
    /// Doc sentence — dim inside the box
    Doc,
    /// path:line footer — dim, hyperlink target
    Loc,
    /// Connector lines between boxes
    Rail,
    /// Arrowheads
    Arrow,
    /// The +/−/! badge in the top border
    Badge,
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    status: DiffStatus,
    role: Role,
}

struct Link {
    row: usize,
    start: usize,
    end: usize,
    url: String,
}

struct Canvas {
    rows: Vec<Vec<Cell>>,
    links: Vec<Link>,
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
}

struct Layout {
    label: Vec<String>,
    doc: Vec<String>,
    loc: Option<String>,
    url: Option<String>,
    status: DiffStatus,
    branch: bool,
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

fn build(node: &DiffNode, options: &BoxOptions, profile: Profile) -> Layout {
    let label = wrap(&node_text(node, true), profile.max_label);
    let doc = if profile.docs {
        node.doc
            .as_deref()
            .map(|doc| wrap(&format!("“{doc}”"), profile.max_label))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let loc = node
        .location
        .as_deref()
        .filter(|_| profile.loc)
        .map(short_loc);
    let url = match (&options.link, &node.location) {
        (Some(template), Some(location)) => location.rsplit_once(':').map(|(path, line)| {
            let absolute = match &options.repo_root {
                Some(root) => root.join(path).to_string_lossy().into_owned(),
                None => path.to_string(),
            };
            template
                .replace("{path}", &absolute)
                .replace("{line}", line)
        }),
        _ => None,
    };

    let content_w = label
        .iter()
        .chain(doc.iter())
        .map(|l| l.chars().count())
        .chain(loc.iter().map(|l| l.chars().count()))
        .max()
        .unwrap_or(0);
    let box_w = content_w + 6;
    let extra = doc.len() + usize::from(loc.is_some());
    let box_h = label.len() + extra + 4;

    let children: Vec<Layout> = node
        .children
        .iter()
        .map(|c| build(c, options, profile))
        .collect();
    let (sub_w, sub_h) = match options.dir {
        Direction::TopDown => {
            let kids_w: usize = children.iter().map(|c| c.sub_w).sum::<usize>()
                + GAP_TD * children.len().saturating_sub(1);
            let kids_h = children
                .iter()
                .map(|c| c.sub_h)
                .max()
                .map(|h| h + CONNECT_TD)
                .unwrap_or(0);
            (box_w.max(kids_w), box_h + kids_h)
        }
        Direction::LeftRight => {
            let kids_h: usize = children.iter().map(|c| c.sub_h).sum::<usize>()
                + GAP_LR * children.len().saturating_sub(1);
            let kids_w = children
                .iter()
                .map(|c| c.sub_w)
                .max()
                .map(|w| w + CONNECT_LR)
                .unwrap_or(0);
            (box_w + kids_w, box_h.max(kids_h))
        }
    };

    Layout {
        label,
        doc,
        loc,
        url,
        status: node.status,
        branch: node.kind == NodeKind::Branch,
        box_w,
        box_h,
        sub_w,
        sub_h,
        children,
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

fn draw_box(canvas: &mut Canvas, layout: &Layout, x: usize, y: usize) {
    let status = layout.status;
    let w = layout.box_w;
    let h = layout.box_h;
    let (horizontal, vertical) = if layout.branch {
        ('┄', '┆')
    } else {
        ('─', '│')
    };

    canvas.put(x, y, '╭', status, Role::Frame);
    canvas.put(x + w - 1, y, '╮', status, Role::Frame);
    canvas.put(x, y + h - 1, '╰', status, Role::Frame);
    canvas.put(x + w - 1, y + h - 1, '╯', status, Role::Frame);
    for col in x + 1..x + w - 1 {
        canvas.put(col, y, horizontal, status, Role::Frame);
        canvas.put(col, y + h - 1, horizontal, status, Role::Frame);
    }
    for row in y + 1..y + h - 1 {
        canvas.put(x, row, vertical, status, Role::Frame);
        canvas.put(x + w - 1, row, vertical, status, Role::Frame);
        for col in x + 1..x + w - 1 {
            canvas.put(col, row, ' ', status, Role::Fill);
        }
    }
    if let Some(glyph) = badge(status) {
        canvas.put(x + 2, y, ' ', status, Role::Badge);
        canvas.put(x + 3, y, glyph, status, Role::Badge);
        canvas.put(x + 4, y, ' ', status, Role::Badge);
    }

    let mut row = y + 2;
    for line in &layout.label {
        let pad = (w - 2 - line.chars().count()) / 2;
        for (offset, ch) in line.chars().enumerate() {
            canvas.put(x + 1 + pad + offset, row, ch, status, Role::Text);
        }
        row += 1;
    }
    for line in &layout.doc {
        let pad = (w - 2 - line.chars().count()) / 2;
        for (offset, ch) in line.chars().enumerate() {
            canvas.put(x + 1 + pad + offset, row, ch, status, Role::Doc);
        }
        row += 1;
    }
    if let Some(loc) = &layout.loc {
        let pad = (w - 2 - loc.chars().count()) / 2;
        for (offset, ch) in loc.chars().enumerate() {
            canvas.put(x + 1 + pad + offset, row, ch, status, Role::Loc);
        }
        if let Some(url) = &layout.url {
            canvas.links.push(Link {
                row,
                start: x + 1 + pad,
                end: x + 1 + pad + loc.chars().count(),
                url: url.clone(),
            });
        }
    }
}

fn place_td(canvas: &mut Canvas, layout: &Layout, x: usize, y: usize) {
    let node_x = x + (layout.sub_w - layout.box_w) / 2;
    draw_box(canvas, layout, node_x, y);
    if layout.children.is_empty() {
        return;
    }

    let parent_cx = node_x + layout.box_w / 2;
    let kids_w: usize = layout.children.iter().map(|c| c.sub_w).sum::<usize>()
        + GAP_TD * (layout.children.len() - 1);
    let mut child_x = x + (layout.sub_w - kids_w) / 2;
    let child_y = y + layout.box_h + CONNECT_TD;

    let stem_row = y + layout.box_h;
    let bus_row = stem_row + 1;
    let drop_row = stem_row + 2;
    let arrow_row = stem_row + 3;

    canvas.put(parent_cx, stem_row, '│', layout.status, Role::Rail);

    let centers: Vec<(usize, DiffStatus)> = {
        let mut centers = Vec::new();
        let mut cx = child_x;
        for child in &layout.children {
            centers.push((cx + child.sub_w / 2, child.status));
            cx += child.sub_w + GAP_TD;
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
        canvas.put(col, bus_row, '─', DiffStatus::Same, Role::Rail);
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
        canvas.put(cx, bus_row, junction, status, Role::Rail);
        canvas.put(cx, drop_row, '│', status, Role::Rail);
        canvas.put(cx, arrow_row, '▼', status, Role::Arrow);
    }
    if left < parent_cx && parent_cx < right {
        canvas.put(parent_cx, bus_row, '┴', layout.status, Role::Rail);
    } else if parent_cx == left && centers.iter().all(|(c, _)| *c != parent_cx) {
        canvas.put(parent_cx, bus_row, '╰', layout.status, Role::Rail);
    } else if parent_cx == right && centers.iter().all(|(c, _)| *c != parent_cx) {
        canvas.put(parent_cx, bus_row, '╯', layout.status, Role::Rail);
    }

    for child in &layout.children {
        place_td(canvas, child, child_x, child_y);
        child_x += child.sub_w + GAP_TD;
    }
}

fn place_lr(canvas: &mut Canvas, layout: &Layout, x: usize, y: usize) {
    let node_y = y + (layout.sub_h.saturating_sub(layout.box_h)) / 2;
    draw_box(canvas, layout, x, node_y);
    if layout.children.is_empty() {
        return;
    }

    let parent_cy = node_y + layout.box_h / 2;
    let edge = x + layout.box_w - 1;
    let bus_col = edge + 2;
    let child_x = x + layout.box_w + CONNECT_LR;

    let mut centers: Vec<(usize, DiffStatus)> = Vec::new();
    let mut cy = y;
    for child in &layout.children {
        let child_box_y = cy + (child.sub_h.saturating_sub(child.box_h)) / 2;
        centers.push((child_box_y + child.box_h / 2, child.status));
        cy += child.sub_h + GAP_LR;
    }
    let top = centers
        .first()
        .map(|(c, _)| *c)
        .unwrap_or(parent_cy)
        .min(parent_cy);
    let bottom = centers
        .last()
        .map(|(c, _)| *c)
        .unwrap_or(parent_cy)
        .max(parent_cy);

    canvas.put(edge + 1, parent_cy, '─', layout.status, Role::Rail);
    for row in top..=bottom {
        canvas.put(bus_col, row, '│', DiffStatus::Same, Role::Rail);
    }
    for &(cy, status) in &centers {
        let junction = if cy == top && cy < parent_cy {
            '╭'
        } else if cy == bottom && cy > parent_cy {
            '╰'
        } else {
            '├'
        };
        canvas.put(bus_col, cy, junction, status, Role::Rail);
        canvas.put(bus_col + 1, cy, '─', status, Role::Rail);
        canvas.put(bus_col + 2, cy, '▶', status, Role::Arrow);
    }
    let parent_join = if parent_cy == top && centers.iter().all(|(c, _)| *c != parent_cy) {
        '╮'
    } else if parent_cy == bottom && centers.iter().all(|(c, _)| *c != parent_cy) {
        '╯'
    } else if centers.iter().any(|(c, _)| *c == parent_cy) {
        '┼'
    } else {
        '┤'
    };
    canvas.put(bus_col, parent_cy, parent_join, layout.status, Role::Rail);

    let mut cy = y;
    for child in &layout.children {
        place_lr(canvas, child, child_x, cy);
        cy += child.sub_h + GAP_LR;
    }
}

fn style(status: DiffStatus, role: Role, color: bool) -> (String, String) {
    if !color {
        return (String::new(), String::new());
    }
    let tint = |((fr, fg, fb), (br, bg, bb)): ((u8, u8, u8), (u8, u8, u8)), inside: bool| {
        if inside {
            format!("\x1b[38;2;{fr};{fg};{fb}m\x1b[48;2;{br};{bg};{bb}m")
        } else {
            format!("\x1b[38;2;{fr};{fg};{fb}m")
        }
    };
    let inside = matches!(
        role,
        Role::Frame | Role::Fill | Role::Text | Role::Doc | Role::Loc | Role::Badge
    );
    let prefix = match status {
        DiffStatus::Added => tint(ADDED, inside),
        DiffStatus::Removed => tint(REMOVED, inside),
        DiffStatus::Changed => tint(CHANGED, inside),
        DiffStatus::Same => match role {
            Role::Frame | Role::Rail | Role::Doc | Role::Loc => "\x1b[2m".to_string(),
            Role::Text | Role::Arrow | Role::Fill | Role::Badge => String::new(),
        },
    };
    // Docs and locations render dim even inside colored boxes.
    let prefix = match (status, role) {
        (DiffStatus::Same, _) => prefix,
        (_, Role::Doc | Role::Loc) => format!("{prefix}\x1b[2m"),
        _ => prefix,
    };
    if prefix.is_empty() {
        (String::new(), String::new())
    } else {
        (prefix, "\x1b[0m".to_string())
    }
}

pub fn render_boxes(root: &DiffNode, options: &BoxOptions) -> String {
    let mut layout = build(root, options, PROFILES[0]);
    if let Some(max_width) = options.max_width {
        for profile in &PROFILES[1..] {
            if layout.sub_w <= max_width {
                break;
            }
            layout = build(root, options, *profile);
        }
        if layout.sub_w > max_width {
            eprintln!(
                "note: graph is {} columns, terminal {} — rows will wrap; try --max-depth 2 or the text view",
                layout.sub_w, max_width
            );
        }
    }
    let mut canvas = Canvas::new(layout.sub_w, layout.sub_h);
    match options.dir {
        Direction::TopDown => place_td(&mut canvas, &layout, 0, 0),
        Direction::LeftRight => place_lr(&mut canvas, &layout, 0, 0),
    }

    let mut out = Vec::new();
    for (row_index, row) in canvas.rows.iter().enumerate() {
        let mut line = String::new();
        let mut run = String::new();
        let mut run_key = (DiffStatus::Same, Role::Rail);
        let links: Vec<&Link> = canvas
            .links
            .iter()
            .filter(|link| link.row == row_index)
            .collect();
        let flush = |line: &mut String, run: &mut String, key: (DiffStatus, Role)| {
            if run.is_empty() {
                return;
            }
            let (open, close) = style(key.0, key.1, options.color);
            line.push_str(&open);
            line.push_str(run);
            line.push_str(&close);
            run.clear();
        };
        for (col, cell) in row.iter().enumerate() {
            for link in &links {
                if link.start == col {
                    flush(&mut line, &mut run, run_key);
                    line.push_str(&format!("\x1b]8;;{}\x1b\\", link.url));
                }
                if link.end == col {
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
        for link in &links {
            if link.end >= row.len() {
                line.push_str("\x1b]8;;\x1b\\");
            }
        }
        out.push(line.trim_end().to_string());
    }
    out.join("\n")
}
