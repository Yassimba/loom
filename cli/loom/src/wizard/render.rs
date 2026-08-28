//! All drawing for the wizard: a one-line header with the step breadcrumb,
//! the active stage's panel, and a one-line footer with key hints and
//! clickable Back/Next buttons.

use super::state::{
    ChooseStage, ExecStatus, Group, HitMap, InstallStage, Pane, Row, Stage, WhereStage, Wizard,
};
use crate::settings::SettingSpec;
use crate::{ResourceKind, SkillAgent};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Clear, Gauge, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub(crate) const ACCENT: Color = Color::Cyan;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;
const ERR: Color = Color::Red;
const ON: &str = "[x]";
const OFF: &str = "[ ]";
const PART: &str = "[-]";
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

type ListHit = Option<(Rect, usize)>;

impl Wizard {
    pub fn draw(&mut self, frame: &mut Frame) {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        self.hits = HitMap::default();
        self.render_header(frame, header);
        let list = match &self.stages[self.stage_index] {
            Stage::Choose(stage) => {
                let (groups, items) = self.render_choose(frame, body, stage);
                self.hits.groups = groups;
                items
            }
            Stage::Where(stage) => self.render_where(frame, body, stage),
            Stage::Review { scroll } => {
                self.render_review(frame, body, *scroll);
                None
            }
            Stage::Install(stage) => {
                self.render_install(frame, body, stage);
                None
            }
        };
        self.hits.list = list;
        self.render_footer(frame, footer);
        if self.show_help {
            self.render_help(frame);
        }
        if self.confirm_quit {
            self.render_confirm_quit(frame);
        }
    }

    // ---- chrome ------------------------------------------------------------

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(vec![
            Span::styled(" loom", Style::new().fg(ACCENT).bold()),
            Span::styled(
                concat!("  v", env!("CARGO_PKG_VERSION")),
                Style::new().dim(),
            ),
        ]);
        frame.render_widget(Paragraph::new(title), area);

        let mut spans = Vec::new();
        if self.probing {
            spans.push(Span::styled("scanning installed…   ", Style::new().dim()));
        }
        let count = self.total_selected();
        if count > 0 {
            spans.push(Span::styled(
                format!("{count} selected   "),
                Style::new().fg(OK),
            ));
        }
        let visible = self.visible_stages();
        if area.width < 80 {
            // No room for the breadcrumb: "step 2/4 · Where".
            let step = visible
                .iter()
                .position(|&index| index == self.stage_index)
                .unwrap_or(0)
                + 1;
            spans.push(Span::styled(
                format!("step {step}/{} · ", visible.len()),
                Style::new().dim(),
            ));
            spans.push(Span::styled(
                self.stages[self.stage_index].title(),
                Style::new().fg(ACCENT).bold(),
            ));
            spans.push(Span::raw(" "));
            frame.render_widget(
                Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
                area,
            );
            return;
        }
        for (position, &index) in visible.iter().enumerate() {
            if position > 0 {
                spans.push(Span::styled(" › ", Style::new().dim()));
            }
            let title = self.stages[index].title();
            spans.push(match index.cmp(&self.stage_index) {
                std::cmp::Ordering::Less => Span::styled(format!("✓ {title}"), Style::new().dim()),
                std::cmp::Ordering::Equal => Span::styled(title, Style::new().fg(ACCENT).bold()),
                std::cmp::Ordering::Greater => Span::styled(title, Style::new().dim()),
            });
        }
        spans.push(Span::raw(" "));
        frame.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
            area,
        );
    }

    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {
        let hint = if self.search.is_some() {
            " type to filter · ↑↓ move · space pick · enter keep place · esc cancel"
        } else {
            match &self.stages[self.stage_index] {
                Stage::Choose(_) => {
                    " ↑↓ move · ←→ column · space pick · / search · enter continue · ? keys"
                }
                Stage::Where(_) => " ↑↓ move · space toggle · enter continue · esc back",
                Stage::Review { .. } => " enter install · ↑↓ scroll · esc back",
                Stage::Install(stage) if stage.running => " installing…",
                Stage::Install(_) => " enter finish · ↑↓ scroll",
            }
        };
        let short = match &self.stages[self.stage_index] {
            Stage::Choose(_) => " ↑↓ ←→ space / enter ?",
            Stage::Where(_) => " ↑↓ space enter esc",
            Stage::Review { .. } => " enter ↑↓ esc",
            Stage::Install(_) => " enter",
        };
        let hint = if area.width < hint.chars().count() as u16 + 26 {
            short
        } else {
            hint
        };
        let [hint_area, back_area, _, next_area, _] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(10),
            Constraint::Length(1),
            Constraint::Length(13),
            Constraint::Length(1),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, Style::new().dim()))),
            hint_area,
        );

        let (back_enabled, next_label, next_enabled) = self.button_states();
        let back_style = if back_enabled {
            Style::new().fg(ACCENT)
        } else {
            Style::new().dim()
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("[ ◂ Back ]", back_style))),
            back_area,
        );
        let next_style = if next_enabled {
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::REVERSED)
                .bold()
        } else {
            Style::new().dim()
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("[ {next_label:^7} ▸ ]"),
                next_style,
            ))),
            next_area,
        );
        if back_enabled {
            self.hits.back_button = back_area;
        }
        if next_enabled {
            self.hits.next_button = next_area;
        }
    }

    fn button_states(&self) -> (bool, &'static str, bool) {
        match &self.stages[self.stage_index] {
            Stage::Choose(_) => (false, "Next", true),
            Stage::Where(_) => (true, "Next", true),
            Stage::Review { .. } => (
                true,
                "Install",
                self.nothing_chosen() || self.plan().is_ok(),
            ),
            Stage::Install(stage) => (false, "Finish", stage.report.is_some()),
        }
    }

    fn render_help(&self, frame: &mut Frame) {
        let key = |text: &'static str| Span::styled(text, Style::new().fg(ACCENT).bold());
        let lines = vec![
            Line::from(vec![key("↑ ↓        "), Span::raw("move (j/k work too)")]),
            Line::from(vec![
                key("← →        "),
                Span::raw("groups column ⇄ items column (tab too)"),
            ]),
            Line::from(vec![key("home end   "), Span::raw("top / bottom")]),
            Line::from(""),
            Line::from(vec![key("space      "), Span::raw("pick, then step down")]),
            Line::from(vec![
                Span::raw("           "),
                Span::styled(
                    "in the groups column: picks the whole group (or Everything)",
                    Style::new().dim(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                key("/          "),
                Span::raw("search · esc cancel · enter keep place"),
            ]),
            Line::from(""),
            Line::from(vec![
                key("enter      "),
                Span::raw("continue (Review: install)"),
            ]),
            Line::from(vec![key("esc        "), Span::raw("back a step")]),
            Line::from(vec![key("q          "), Span::raw("quit")]),
            Line::from(""),
            Line::styled("any key closes this", Style::new().dim()),
        ];
        let width = 58.min(frame.area().width.saturating_sub(4));
        let height = (lines.len() as u16 + 2).min(frame.area().height.saturating_sub(2));
        let area = centered_rect(frame.area(), width, height);
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines).block(bordered(" Keys ", true).padding(Padding::horizontal(1))),
            area,
        );
    }

    fn render_confirm_quit(&self, frame: &mut Frame) {
        let count = self.total_selected();
        let lines = vec![
            Line::from(format!("Quit and drop {} picked?", plural(count, "item"))),
            Line::from(""),
            Line::from(vec![
                Span::styled("enter", Style::new().fg(ERR).bold()),
                Span::raw(" quit   "),
                Span::styled("any other key", Style::new().fg(ACCENT).bold()),
                Span::raw(" stay"),
            ]),
        ];
        let area = centered_rect(
            frame.area(),
            46.min(frame.area().width.saturating_sub(4)),
            5,
        );
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .block(bordered(" Quit? ", true)),
            area,
        );
    }

    // ---- choose ------------------------------------------------------------

    fn render_choose(
        &self,
        frame: &mut Frame,
        area: Rect,
        stage: &ChooseStage,
    ) -> (ListHit, ListHit) {
        let searching = self.search.is_some();
        // Column one is as wide as its longest title plus marks and a count,
        // never more than a third of the screen. Under 70 columns there is
        // no room for three columns: show the focused one alone.
        let widest = stage
            .groups
            .iter()
            .map(|group| group.title.width())
            .max()
            .unwrap_or(0) as u16;
        let max_groups = (area.width / 3).max(1);
        let groups_width = (widest + 14).clamp(24.min(max_groups), max_groups);
        let narrow = area.width < 70;
        let [groups_area, items_area, details_area] = if narrow {
            let show_groups = stage.focus == Pane::Groups && !searching;
            let [only] = Layout::horizontal([Constraint::Min(1)]).areas(area);
            let empty = Rect::new(area.x, area.y, 0, 0);
            if show_groups {
                [only, empty, empty]
            } else {
                [empty, only, empty]
            }
        } else {
            Layout::horizontal([
                Constraint::Length(groups_width),
                Constraint::Min(30),
                Constraint::Percentage(34),
            ])
            .areas(area)
        };

        // Column one: every group with its state.
        let group_width = groups_area.width.saturating_sub(2) as usize;
        let groups = stage
            .groups
            .iter()
            .map(|group| self.group_item(group, group_width))
            .collect::<Vec<_>>();
        let group_offset = list_offset(
            stage.groups.len(),
            groups_area.height.saturating_sub(2),
            stage.group_cursor,
        );
        let groups_focused = stage.focus == Pane::Groups && !searching;
        if groups_area.width > 0 {
            let title = if narrow {
                " Groups · → items "
            } else {
                " Groups "
            };
            frame.render_stateful_widget(
                List::new(groups)
                    .block(bordered(title, groups_focused))
                    .highlight_style(highlight(groups_focused)),
                groups_area,
                &mut ListState::default()
                    .with_selected(Some(stage.group_cursor))
                    .with_offset(group_offset),
            );
        }

        // Column two: the focused group's rows, or the search hits.
        let (rows, cursor, title): (Vec<Row>, usize, String) = match &self.search {
            Some(query) => {
                let matches = self.search_matches();
                let cursor = self.search_cursor.min(matches.len().saturating_sub(1));
                let rows = matches.iter().map(|&hit| self.search_row(hit)).collect();
                (
                    rows,
                    cursor,
                    format!(" /{query}▏  {} matches ", matches.len()),
                )
            }
            None => {
                let group = stage.group();
                let (on, actionable) = self.group_counts(&self.group_items(group));
                let title = if group.everything {
                    format!(" {} · {on}/{actionable} picked ", group.title)
                } else if actionable == 0 {
                    format!(" {} · all installed ", group.title)
                } else {
                    format!(" {} · {on}/{actionable} picked ", group.title)
                };
                (group.rows.clone(), stage.item_cursor, title)
            }
        };
        // Label column from the widest label on screen; the description
        // takes what is left and is cut with an ellipsis.
        let label_width = rows
            .iter()
            .map(|row| self.row_label(row).width())
            .max()
            .unwrap_or(0)
            .min(32);
        let description_width = (items_area.width as usize).saturating_sub(2 + 5 + label_width + 1);
        let items = if !searching && stage.group().everything {
            vec![ListItem::new(Line::styled(
                "  space picks or clears the whole catalog; ↓ for the groups.",
                Style::new().dim(),
            ))]
        } else {
            rows.iter()
                .map(|row| self.choose_row_item(row, label_width, description_width))
                .collect::<Vec<_>>()
        };
        let offset = list_offset(rows.len(), items_area.height.saturating_sub(2), cursor);
        let items_focused = stage.focus == Pane::Items || searching;
        let title = if narrow && !searching {
            format!("{title}· ← groups ")
        } else {
            title
        };
        if items_area.width > 0 {
            frame.render_stateful_widget(
                List::new(items)
                    .block(bordered(&title, items_focused))
                    .highlight_style(highlight(items_focused)),
                items_area,
                &mut ListState::default()
                    .with_selected((!rows.is_empty()).then_some(cursor))
                    .with_offset(offset),
            );
        }

        // Column three: what the cursor is on.
        let details = match (stage.focus, rows.get(cursor)) {
            (_, None) if searching => vec![Line::styled(
                "No matches. Backspace widens, esc cancels.",
                Style::new().dim(),
            )],
            (_, _) if !searching && (stage.focus == Pane::Groups || stage.group().everything) => {
                self.group_details(stage.group())
            }
            (_, Some(row)) => self.row_details(row),
            (_, None) => Vec::new(),
        };
        if details_area.width > 0 {
            frame.render_widget(
                Paragraph::new(details)
                    .wrap(Wrap { trim: true })
                    .block(bordered(" Details ", false).padding(Padding::horizontal(1))),
                details_area,
            );
        }
        (
            (groups_area.width > 0).then_some((groups_area, group_offset)),
            (items_area.width > 0).then_some((items_area, offset)),
        )
    }

    fn group_item(&self, group: &Group, width: usize) -> ListItem<'_> {
        let items = self.group_items(group);
        let (mark, mark_style, count) = if items.is_empty() {
            ("   ", Style::new(), String::new())
        } else {
            let (on, actionable) = self.group_counts(&items);
            let (mark, style) = if actionable == 0 {
                (" ✓ ", Style::new().fg(OK).dim())
            } else if on == 0 {
                (OFF, Style::new().dim())
            } else if on == actionable {
                (ON, Style::new().fg(OK))
            } else {
                (PART, Style::new().fg(OK))
            };
            let count = if actionable == 0 {
                "✓".to_owned()
            } else if on == 0 {
                format!("{actionable}")
            } else {
                format!("{on}/{actionable}")
            };
            (mark, style, count)
        };
        let title_style = if count == "✓" {
            Style::new().dim()
        } else {
            Style::new()
        };
        // Title, then at least one space, then the count flush right;
        // a title that cannot fit is cut with an ellipsis.
        let room = width.saturating_sub(5 + count.width() + 2);
        let title = cut(&group.title, room);
        let fill = room.saturating_sub(title.width()) + 1;
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {mark} "), mark_style),
            Span::styled(title, title_style),
            Span::raw(" ".repeat(fill)),
            Span::styled(
                count,
                if mark == ON || mark == PART {
                    Style::new().fg(OK)
                } else {
                    Style::new().dim()
                },
            ),
        ]))
    }

    fn row_label(&self, row: &Row) -> &str {
        match row {
            Row::Resource(index) => &self.model.resources[*index].label,
            Row::Setting(index) => &self.model.settings[*index].label,
        }
    }

    fn choose_row_item(&self, row: &Row, label_width: usize, room: usize) -> ListItem<'_> {
        let label = pad(self.row_label(row), label_width);
        let (mark, mark_style, dim_label, note) = match row {
            Row::Resource(index) => {
                let resource = &self.model.resources[*index];
                if self.resource_installed(*index) {
                    (
                        " ✓ ",
                        Style::new().fg(OK).dim(),
                        true,
                        resource.description.clone(),
                    )
                } else if let Some(parent) = self.required_note(*index) {
                    (
                        ON,
                        Style::new().fg(OK).dim(),
                        true,
                        format!("needed by {parent}"),
                    )
                } else {
                    let (mark, style) = mark_for(self.selected[*index]);
                    (mark, style, false, resource.description.clone())
                }
            }
            Row::Setting(index) => {
                let spec = &self.model.settings[*index];
                if self.setting_applied(*index) {
                    (
                        " ✓ ",
                        Style::new().fg(OK).dim(),
                        true,
                        spec.description.clone(),
                    )
                } else {
                    let (mark, style) = mark_for(self.setting_on[*index]);
                    (mark, style, false, spec.description.clone())
                }
            }
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {mark} "), mark_style),
            Span::styled(
                format!("{label} "),
                if dim_label {
                    Style::new().dim()
                } else {
                    Style::new()
                },
            ),
            Span::styled(cut(&note, room), Style::new().dim()),
        ]))
    }

    fn group_details(&self, group: &Group) -> Vec<Line<'_>> {
        let items = self.group_items(group);
        let mut lines = vec![
            Line::styled(group.title.clone(), Style::new().bold().fg(ACCENT)),
            Line::from(""),
        ];
        if group.everything {
            let (on, actionable) = self.group_counts(&items);
            lines.push(Line::from(format!(
                "{on} of {actionable} picked across the whole catalog · {} already installed.",
                items.len() - actionable
            )));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "space picks everything not yet installed, or clears it all; then trim by group.",
                Style::new().dim(),
            ));
        } else {
            let (on, actionable) = self.group_counts(&items);
            let installed = items.len() - actionable;
            lines.push(Line::from(if actionable == 0 {
                format!("All {} installed.", plural(installed, "item"))
            } else if installed == 0 {
                format!("{on} of {actionable} picked.")
            } else {
                format!("{on} of {actionable} picked · {installed} already installed.")
            }));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                if actionable == 0 {
                    "→ opens the group"
                } else {
                    "space picks or clears the whole group · → opens it"
                },
                Style::new().dim(),
            ));
        }
        lines
    }

    fn row_details(&self, row: &Row) -> Vec<Line<'_>> {
        match row {
            Row::Resource(index) => self.resource_details(*index),
            Row::Setting(index) => self.setting_details(&self.model.settings[*index], *index),
        }
    }

    fn resource_details(&self, index: usize) -> Vec<Line<'_>> {
        let resource = &self.model.resources[index];
        let mut lines = vec![
            Line::styled(resource.label.clone(), Style::new().bold().fg(ACCENT)),
            Line::styled(
                format!("{} · {}", resource.kind, resource.group),
                Style::new().dim(),
            ),
            Line::from(""),
            Line::from(resource.description.clone()),
            Line::from(""),
        ];
        if !resource.dependencies.is_empty() {
            lines.push(field("pulls in", resource.dependencies.join(", "), WARN));
        }
        match resource.kind {
            ResourceKind::Skill => {
                let destination = self.skill_destination();
                let trees = destination.trees();
                if trees.is_empty() {
                    lines.push(field("goes to", "no agent chosen yet".into(), WARN));
                } else {
                    lines.push(field("goes to", String::new(), ACCENT));
                    for tree in trees {
                        lines.push(Line::styled(
                            tidy(&tree, &destination.home),
                            Style::new().dim(),
                        ));
                    }
                }
            }
            ResourceKind::Tool => lines.push(field(
                "via",
                format!("mise · {}", resource.install_target),
                ACCENT,
            )),
            ResourceKind::PiPackage => lines.push(field(
                "via",
                format!("pi install {}", resource.install_target),
                ACCENT,
            )),
            ResourceKind::HerdrPlugin => lines.push(field(
                "via",
                format!("herdr plugin install {}", resource.install_target),
                ACCENT,
            )),
        }
        lines.push(Line::from(""));
        lines.push(field("then", resource.next_action.clone(), ACCENT));
        if self.resource_installed(index) {
            lines.push(Line::from(""));
            lines.push(Line::styled("Already installed.", Style::new().fg(OK)));
        } else if let Some(parent) = self.required_note(index) {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!("Needed by {parent}; clear that to drop this."),
                Style::new().fg(OK),
            ));
        }
        lines
    }

    fn setting_details(&self, spec: &SettingSpec, index: usize) -> Vec<Line<'_>> {
        let mut lines = vec![
            Line::styled(spec.label.clone(), Style::new().bold().fg(ACCENT)),
            Line::styled(format!("Setting · {}", spec.group), Style::new().dim()),
            Line::from(""),
            Line::from(spec.description.clone()),
            Line::from(""),
            field(
                "file",
                spec.target_path(&self.model.settings_paths)
                    .display()
                    .to_string(),
                ACCENT,
            ),
        ];
        for change in spec.change_summary() {
            lines.push(Line::styled(format!("  + {change}"), Style::new().fg(OK)));
        }
        if self.setting_applied(index) {
            lines.push(Line::from(""));
            lines.push(Line::styled("Already set.", Style::new().fg(OK)));
        }
        lines
    }

    // ---- where -------------------------------------------------------------

    fn render_where(&self, frame: &mut Frame, area: Rect, stage: &WhereStage) -> ListHit {
        let [list_area, details_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);
        let destination = self.skill_destination();
        let (global, project) = match self.skill_scope {
            crate::SkillScope::Global => ("(•)", "( )"),
            crate::SkillScope::Project => ("( )", "(•)"),
        };
        let project_name = destination
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| destination.project_root.display().to_string());
        let mut items = vec![ListItem::new(Line::from(vec![
            Span::styled(format!(" {global} "), Style::new().fg(OK)),
            Span::raw("Global"),
            Span::styled("  every project     ", Style::new().dim()),
            Span::styled(format!("{project} "), Style::new().fg(OK)),
            Span::raw("Project"),
            Span::styled(format!("  only {project_name}/"), Style::new().dim()),
        ]))];
        let agent_width = SkillAgent::ALL
            .iter()
            .map(|agent| agent.label().width())
            .max()
            .unwrap_or(0);
        for (agent, on) in SkillAgent::ALL.iter().zip(&self.agent_on) {
            let (mark, style) = mark_for(*on);
            let tree = match self.skill_scope {
                crate::SkillScope::Global => agent.global_skill_tree(&destination.home),
                crate::SkillScope::Project => agent.project_skill_tree(&destination.project_root),
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {mark} "), style),
                Span::raw(format!("{} ", pad(agent.label(), agent_width))),
                Span::styled(tidy(&tree, &destination.home), Style::new().dim()),
            ])));
        }
        let skills = self
            .expanded_selection()
            .iter()
            .filter(|resource| resource.kind == ResourceKind::Skill)
            .count();
        let title = format!(
            " Where do {} go? · {}/{} agents ",
            plural(skills, "skill"),
            self.selected_agents().len(),
            SkillAgent::ALL.len()
        );
        let list = List::new(items)
            .block(bordered(&title, true))
            .highlight_style(highlight(true));
        let mut state = ListState::default().with_selected(Some(stage.cursor));
        frame.render_stateful_widget(list, list_area, &mut state);

        let mut details = vec![
            Line::styled("Skill destination", Style::new().bold().fg(ACCENT)),
            Line::from(""),
            Line::from(
                "Skills are copied into each chosen agent's skill tree. Global trees serve every \
                 project; a project tree serves this repository only.",
            ),
            Line::from(""),
        ];
        if destination.agents.is_empty() {
            details.push(Line::styled(
                "Pick at least one agent, or nothing can be installed.",
                Style::new().fg(WARN),
            ));
        } else {
            details.push(Line::styled("Will write to", Style::new().bold()));
            for tree in destination.trees() {
                details.push(Line::styled(
                    format!("  {}", tidy(&tree, &destination.home)),
                    Style::new().dim(),
                ));
            }
        }
        frame.render_widget(
            Paragraph::new(details)
                .wrap(Wrap { trim: true })
                .block(bordered(" Details ", false).padding(Padding::horizontal(1))),
            details_area,
        );
        Some((list_area, 0))
    }

    // ---- review ------------------------------------------------------------

    fn render_review(&self, frame: &mut Frame, area: Rect, scroll: u16) {
        let mut lines = Vec::new();
        if self.nothing_chosen() {
            lines.push(Line::from(
                "Nothing picked. Enter leaves without changes; esc goes back.",
            ));
        } else {
            let expanded = self.expanded_selection();
            let direct_ids = self
                .selection()
                .iter()
                .map(|resource| resource.id.clone())
                .collect::<std::collections::HashSet<_>>();
            let parent_of = |target: &str| {
                expanded
                    .iter()
                    .find(|parent| parent.dependencies.iter().any(|dep| dep == target))
                    .map(|parent| parent.label.clone())
            };
            let plan = self.plan();
            let label_width = expanded
                .iter()
                .map(|resource| resource.label.width())
                .chain(
                    self.selected_settings()
                        .iter()
                        .map(|spec| spec.label.width()),
                )
                .chain(
                    plan.iter()
                        .flat_map(|plan| plan.prerequisites.iter())
                        .map(|step| step.target.width()),
                )
                .max()
                .unwrap_or(0);
            if let Ok(plan) = &plan {
                if !plan.prerequisites.is_empty() {
                    lines.push(heading("Installs first", plan.prerequisites.len()));
                    for step in &plan.prerequisites {
                        lines.push(Line::from(vec![
                            Span::styled("  ! ", Style::new().fg(WARN).bold()),
                            Span::raw(format!("{} ", pad(&step.target, label_width))),
                            Span::styled(step.action.display(), Style::new().dim()),
                        ]));
                    }
                    lines.push(Line::from(""));
                }
            }
            for (kind, title) in [
                (ResourceKind::Skill, "Skills"),
                (ResourceKind::Tool, "Tools"),
                (ResourceKind::PiPackage, "Pi packages"),
                (ResourceKind::HerdrPlugin, "Herdr plugins"),
            ] {
                let of_kind = expanded
                    .iter()
                    .filter(|resource| resource.kind == kind)
                    .collect::<Vec<_>>();
                if of_kind.is_empty() {
                    continue;
                }
                lines.push(heading(title, of_kind.len()));
                if kind == ResourceKind::Skill {
                    let destination = self.skill_destination();
                    if destination.agents.is_empty() {
                        lines.push(Line::styled("  ! no agent chosen", Style::new().fg(ERR)));
                    } else {
                        for tree in destination.trees() {
                            lines.push(Line::styled(
                                format!("  → {}", tidy(&tree, &destination.home)),
                                Style::new().dim(),
                            ));
                        }
                    }
                }
                for resource in of_kind {
                    let mut spans = vec![
                        Span::styled("  + ", Style::new().fg(OK).bold()),
                        Span::raw(format!("{} ", pad(&resource.label, label_width))),
                    ];
                    if !direct_ids.contains(&resource.id) {
                        spans.push(Span::styled(
                            format!(
                                "needed by {}",
                                parent_of(&resource.install_target)
                                    .unwrap_or_else(|| "a picked skill".into())
                            ),
                            Style::new().dim(),
                        ));
                    }
                    lines.push(Line::from(spans));
                }
                lines.push(Line::from(""));
            }
            let settings = self.selected_settings();
            if !settings.is_empty() {
                lines.push(heading("Settings", settings.len()));
                for spec in &settings {
                    lines.push(Line::from(vec![
                        Span::styled("  ~ ", Style::new().fg(WARN).bold()),
                        Span::raw(format!("{} ", pad(&spec.label, label_width))),
                        Span::styled(
                            spec.target_path(&self.model.settings_paths)
                                .display()
                                .to_string(),
                            Style::new().dim(),
                        ),
                    ]));
                }
                lines.push(Line::from(""));
            }
            match plan {
                Ok(_) => lines.push(Line::styled(
                    if self.model.dry_run {
                        "Dry run: enter prints this plan and exits."
                    } else {
                        "Enter installs all of this. Esc goes back."
                    },
                    Style::new().fg(OK),
                )),
                Err(error) => lines.push(Line::styled(
                    format!("Cannot install: {error}"),
                    Style::new().fg(ERR),
                )),
            }
        }
        let paragraph = Paragraph::new(lines)
            .block(bordered(" Review ", true).padding(Padding::horizontal(1)))
            .scroll((scroll, 0));
        frame.render_widget(paragraph, area);
    }

    // ---- install -----------------------------------------------------------

    fn render_install(&self, frame: &mut Frame, area: Rect, stage: &InstallStage) {
        let summary_height = if stage.report.is_some() { 7 } else { 0 };
        let [gauge_area, steps_area, summary_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(summary_height),
        ])
        .areas(area);

        let total = stage.items.len().max(1);
        let done = stage
            .items
            .iter()
            .filter(|item| !matches!(item.status, ExecStatus::Pending | ExecStatus::Running))
            .count();
        let failed = stage
            .items
            .iter()
            .any(|item| matches!(item.status, ExecStatus::Failed(_)));
        let gauge_color = if failed { ERR } else { OK };
        frame.render_widget(
            Gauge::default()
                .block(bordered(" Installing ", true))
                .gauge_style(Style::new().fg(gauge_color).bg(Color::DarkGray))
                .ratio(done as f64 / total as f64)
                .label(format!("{done} of {total}")),
            gauge_area,
        );

        let spinner = SPINNER[stage.tick % SPINNER.len()];
        let label_width = stage
            .items
            .iter()
            .map(|item| item.label.width())
            .max()
            .unwrap_or(0);
        let items = stage
            .items
            .iter()
            .map(|item| {
                let (mark, style, note) = match &item.status {
                    ExecStatus::Pending => ("○".into(), Style::new().dim(), String::new()),
                    ExecStatus::Running => (
                        spinner.to_string(),
                        Style::new().fg(ACCENT),
                        item.detail.clone(),
                    ),
                    ExecStatus::Ok(note) => ("✓".into(), Style::new().fg(OK), note.clone()),
                    ExecStatus::Failed(message) => {
                        ("✗".into(), Style::new().fg(ERR), message.clone())
                    }
                    ExecStatus::Skipped(message) => {
                        ("⊘".into(), Style::new().fg(WARN), message.clone())
                    }
                };
                let mut spans = vec![
                    Span::styled(format!(" {mark} "), style),
                    Span::raw(format!("{} ", pad(&item.label, label_width))),
                ];
                if !note.is_empty() {
                    spans.push(Span::styled(
                        first_line(&note).to_owned(),
                        style.add_modifier(Modifier::DIM),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect::<Vec<_>>();
        let visible = steps_area.height.saturating_sub(2);
        let active = stage
            .items
            .iter()
            .position(|item| matches!(item.status, ExecStatus::Running))
            .unwrap_or(done.saturating_sub(1));
        let offset = if stage.running {
            list_offset(stage.items.len(), visible, active)
        } else {
            (stage.scroll as usize).min(stage.items.len().saturating_sub(visible as usize))
        };
        let list = List::new(items).block(bordered(" Tasks ", false));
        let mut state = ListState::default().with_offset(offset);
        frame.render_stateful_widget(list, steps_area, &mut state);

        if let Some(report) = &stage.report {
            let mut lines = Vec::new();
            if report.failures.is_empty() {
                lines.push(Line::styled("✓ Done", Style::new().fg(OK).bold()));
                lines.push(Line::from(format!(
                    "{} installed, nothing failed.",
                    report.installed.len()
                )));
            } else {
                let failed = report.failures.len();
                lines.push(Line::styled(
                    format!("✗ {} failed", plural(failed, "task")),
                    Style::new().fg(ERR).bold(),
                ));
                lines.push(Line::from(format!(
                    "{} installed · the failed rows above say why.",
                    report.installed.len()
                )));
            }
            for action in self.next_actions(report).iter().take(2) {
                lines.push(field("next", action.clone(), ACCENT));
            }
            lines.push(Line::from(vec![
                Span::styled("enter", Style::new().fg(ACCENT).bold()),
                Span::raw(" to finish."),
            ]));
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .block(bordered(" Result ", true).padding(Padding::horizontal(1))),
                summary_area,
            );
        }
    }

    fn next_actions(&self, report: &crate::InstallReport) -> Vec<String> {
        let mut actions = Vec::new();
        for resource in self.selection() {
            let installed = report.installed.contains(&resource.id)
                || (resource.kind == ResourceKind::Skill
                    && report.installed.iter().any(|target| target == "skills"));
            if installed && !actions.contains(&resource.next_action) {
                actions.push(resource.next_action.clone());
            }
        }
        actions
    }
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Pad to a display width (wide glyphs count double).
fn pad(text: &str, width: usize) -> String {
    let fill = width.saturating_sub(text.width());
    format!("{text}{}", " ".repeat(fill))
}

/// Cut to a display width with an ellipsis.
fn cut(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let next = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if out.width() + next + 1 > width {
            break;
        }
        out.push(ch);
    }
    format!("{}…", out.trim_end())
}

/// A path with the home directory folded to `~`.
fn tidy(path: &std::path::Path, home: &std::path::Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// `label  value` with a dim, fixed-width label column.
fn field(label: &'static str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::new().fg(color).dim()),
        Span::raw(value),
    ])
}

fn heading(title: &'static str, count: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(title, Style::new().bold()),
        Span::styled(format!(" ({count})"), Style::new().dim()),
    ])
}

fn mark_for(on: bool) -> (&'static str, Style) {
    if on {
        (ON, Style::new().fg(OK))
    } else {
        (OFF, Style::new().dim())
    }
}

fn bordered(title: &str, focused: bool) -> Block<'_> {
    let border_style = if focused {
        Style::new().fg(ACCENT)
    } else {
        Style::new().dim()
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title.to_owned())
}

/// The cursor row: the terminal's own colors inverted, tinted with the
/// accent when the column has focus, so contrast holds on any palette.
fn highlight(focused: bool) -> Style {
    if focused {
        Style::new().fg(ACCENT).add_modifier(Modifier::REVERSED)
    } else {
        Style::new().add_modifier(Modifier::REVERSED | Modifier::DIM)
    }
}

fn list_offset(len: usize, visible: u16, cursor: usize) -> usize {
    let visible = visible.max(1) as usize;
    if cursor < visible || len <= visible {
        0
    } else {
        (cursor + 1 - visible).min(len - visible)
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text)
}
