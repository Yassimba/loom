//! All drawing for the wizard: a one-line header with the step breadcrumb,
//! the active stage's panel, and a responsive footer with key hints and
//! clickable Back/Next buttons.

use super::state::{ChooseStage, Group, HitMap, Item, ItemState, Pane, Screen, Wizard};
use crate::settings::SettingSpec;
use crate::{ResourceKind, SkillAgent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Clear, List, ListState, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::ui::chrome::{self, Crumb};
pub(crate) use crate::ui::theme::{ACCENT, ERR, OK, SPINNER, TITLE, WARN};
const ON: &str = "[x]";
const OFF: &str = "[ ]";
const PART: &str = "[-]";

pub(super) type ListHit = Option<(Rect, usize)>;

#[derive(Default)]
struct ItemCounts {
    available: usize,
    picked: usize,
    required: usize,
    installed: usize,
    unavailable: usize,
}

impl ItemCounts {
    fn actionable(&self) -> usize {
        self.available + self.picked
    }

    /// `2 picks · 3 already installed · 1 required`, skipping zero counts.
    fn summary(&self) -> String {
        let mut parts = vec![plural(self.picked, "pick")];
        for (count, label) in [
            (self.installed, "already installed"),
            (self.required, "required"),
            (self.unavailable, "unavailable"),
        ] {
            if count > 0 {
                parts.push(format!("{count} {label}"));
            }
        }
        parts.join(" · ")
    }
}

impl Wizard {
    pub fn draw(&mut self, frame: &mut Frame) {
        self.hits = HitMap::default();
        let Some([header, body, footer]) =
            chrome::frame_areas(frame, "Or use `loom add --help` for scripted setup.")
        else {
            return;
        };
        self.render_header(frame, header);
        let list = match self.screen {
            Screen::Choose => {
                let (groups, kinds, items) = self.render_choose(frame, body, &self.choose);
                self.hits.groups = groups;
                self.hits.kinds = kinds;
                items
            }
            Screen::Where => self.render_where(frame, body, self.where_cursor),
            Screen::Responses => self.render_responses(frame, body, self.responses_cursor),
            Screen::Review => {
                self.review_next_enabled = self.render_review(frame, body, self.review_scroll);
                None
            }
            Screen::Install => {
                self.render_install(frame, body, &self.install);
                None
            }
        };
        self.hits.list = list;
        self.render_footer(frame, footer);
        self.render_wiki_unregister(frame);
        if self.show_help {
            self.render_help(frame);
        }
        if self.confirm_quit {
            self.render_confirm_quit(frame);
        }
        if self.confirm_cancel {
            self.render_confirm_cancel(frame);
        }
        if std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").is_ok_and(|term| term == "dumb")
        {
            let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
            for cell in &mut frame.buffer_mut().content {
                cell.set_fg(Color::Reset).set_bg(Color::Reset);
                if dumb {
                    let replacement = match cell.symbol() {
                        "✓" => Some("v"),
                        "✗" => Some("x"),
                        "○" => Some("o"),
                        "⊘" => Some("-"),
                        "›" | "→" | "▸" => Some(">"),
                        "◂" => Some("<"),
                        "•" => Some("*"),
                        symbol if SPINNER.contains(&symbol) => Some("."),
                        _ => None,
                    };
                    if let Some(symbol) = replacement {
                        cell.set_symbol(symbol);
                    }
                }
            }
        }
    }

    // ---- chrome ------------------------------------------------------------

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let command = if self.uninstalling() {
            "uninstall"
        } else {
            self.model.mode.command()
        };
        let mut status = Vec::new();
        if self.probing {
            status.push(Span::styled("scanning installed…   ", Style::new().dim()));
        }
        let count = self.user_picked();
        if count > 0 {
            status.push(Span::styled(
                format!("{count} picked   "),
                Style::new().fg(OK),
            ));
        }
        let visible = self.visible_stages();
        let current = visible
            .iter()
            .position(|&index| index == self.screen)
            .unwrap_or(0);
        let crumbs: Vec<Crumb> = visible
            .iter()
            .map(|&index| Crumb {
                label: index.title().to_owned(),
                done: index < self.screen,
            })
            .collect();
        chrome::header(frame, area, command, status, &crumbs, current);
    }

    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {
        let searching =
            self.search.is_some() || (self.browsing_wiki() && self.wiki.search.is_some());
        let hint = if searching {
            " type to filter · ↑↓ move · space pick · enter jump to it · esc cancel"
        } else if self.browsing_wiki() {
            " ↑↓ browse · ←→ column · space/enter pick · / search · u unregisters · n review · esc back"
        } else {
            match self.screen {
                Screen::Choose => {
                    " ↑↓ move · ←→ column · space pick · / search · c clear · enter continue · ? keys"
                }
                Screen::Where => " ↑↓ move · space toggle · enter continue · esc back",
                Screen::Responses => " ↑↓ choose · enter continue · esc back",
                Screen::Review if self.nothing_chosen() => " enter leave · esc back",
                Screen::Review => {
                    if self.uninstalling() {
                        " ↑↓ scroll · enter review removal · esc back"
                    } else {
                        " ↑↓ scroll · enter install · esc back"
                    }
                }
                Screen::Install if self.install.running && self.confirm_cancel => {
                    " press ctrl-c again to cancel · install may be partial"
                }
                Screen::Install if self.install.running => " installing… · ctrl-c cancel",
                Screen::Install if self.can_retry() => {
                    " ↑↓ inspect · d details · enter/r retry failed · esc finish"
                }
                Screen::Install if self.next_command().is_some() => {
                    " enter finish · c copy the next command · d details"
                }
                Screen::Install => " ↑↓ inspect · d details · enter finish",
            }
        };
        let short = match self.screen {
            Screen::Choose if searching => " type to filter · enter jump · esc cancel",
            Screen::Choose if self.browsing_wiki() => " space pick · n review · ? keys",
            Screen::Choose => " space pick · enter next · ? keys",
            Screen::Where => " space toggle · enter next · esc back",
            Screen::Responses => " ↑↓ choose · enter next · esc back",
            Screen::Review if self.nothing_chosen() => " enter leave · esc back",
            Screen::Review if self.uninstalling() => " ↑↓ scroll · enter remove · esc back",
            Screen::Review => " ↑↓ scroll · enter install · esc back",
            Screen::Install if self.install.running => " ctrl-c cancel · completed work stays",
            Screen::Install if self.can_retry() => " enter retry · d details · esc finish",
            Screen::Install => " enter finish · d details",
        };
        let (back_enabled, next_label, next_enabled) = self.button_states();
        let hint_width = area.width.saturating_sub(if area.height > 1 {
            0
        } else {
            chrome::NEXT_WIDTH
                + 1
                + if back_enabled {
                    chrome::BACK_WIDTH + 1
                } else {
                    0
                }
        });
        let hint = if hint.width() > usize::from(hint_width) {
            short
        } else {
            hint
        };
        let (back, next) = chrome::footer(
            frame,
            area,
            hint,
            back_enabled.then_some("Back"),
            (next_label, next_enabled),
        );
        self.hits.back_button = back;
        self.hits.next_button = next;
    }

    fn button_states(&self) -> (bool, &'static str, bool) {
        match self.screen {
            Screen::Choose if self.browsing_wiki() => {
                (self.choose.focus != Pane::Groups, "Next", true)
            }
            Screen::Choose => (false, "Next", true),
            Screen::Where | Screen::Responses => (true, "Next", true),
            Screen::Review => (
                true,
                if self.nothing_chosen() {
                    "Leave"
                } else if self.uninstalling() {
                    "Remove"
                } else {
                    "Install"
                },
                self.review_next_enabled,
            ),
            Screen::Install => (
                false,
                if self.can_retry() { "Retry" } else { "Finish" },
                self.install.report.is_some(),
            ),
        }
    }

    fn render_help(&self, frame: &mut Frame) {
        let key = |text: &'static str| Span::styled(text, TITLE);
        let profile_mode = self.profile_mode();
        let (columns, left, set) = if profile_mode {
            ("goals ⇄ types ⇄ capabilities", "goals", "goal")
        } else {
            ("groups ⇄ items", "groups", "group")
        };
        let mut lines = vec![
            Line::from(vec![key("↑ ↓        "), Span::raw("move (j/k work too)")]),
            Line::from(vec![
                key("← →        "),
                Span::raw(format!("{columns} (tab too)")),
            ]),
            Line::from(vec![key("home end   "), Span::raw("top / bottom")]),
            Line::from(""),
            Line::from(vec![key("space      "), Span::raw("pick, then step down")]),
            Line::from(vec![
                Span::raw("           "),
                Span::styled(
                    format!("in the {left} column: picks the whole {set} (or Everything)"),
                    Style::new().dim(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                key("/          "),
                Span::raw("search · esc cancel · enter jump to the match"),
            ]),
            Line::from(""),
            Line::from(vec![
                key("enter      "),
                Span::raw("continue (Review: install)"),
            ]),
            Line::from(vec![key("c          "), Span::raw("clear every pick")]),
            Line::from(vec![key("esc        "), Span::raw("back a step")]),
            Line::from(vec![key("q          "), Span::raw("quit")]),
            Line::from(""),
            Line::styled("any key closes this", Style::new().dim()),
        ];
        if profile_mode {
            lines.insert(
                6,
                Line::from(vec![
                    Span::raw("           "),
                    Span::styled(
                        "in the types column: picks every capability of that type",
                        Style::new().dim(),
                    ),
                ]),
            );
        }
        if frame.area().width < 80 || frame.area().height < 22 {
            lines = vec![
                Line::from(vec![key("↑↓ / j k "), Span::raw("move · ←→ / tab column")]),
                Line::from(vec![key("space  "), Span::raw("pick · c clear · / search")]),
                Line::from(vec![key("enter   "), Span::raw("next · esc back · q quit")]),
                Line::from(vec![key("home/end "), Span::raw("first / last")]),
                Line::from(format!("space on a {set}: pick all")),
                Line::styled("any key closes this", Style::new().dim()),
            ];
        }
        let widest = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
        let width = (widest + chrome::PANEL_FRAME).min(frame.area().width.saturating_sub(4));
        let height = (lines.len() as u16 + 2).min(frame.area().height.saturating_sub(2));
        let area = if frame.area().height < 12 || frame.area().width < 50 {
            frame.area()
        } else {
            frame
                .area()
                .centered(Constraint::Length(width), Constraint::Length(height))
        };
        frame.render_widget(Clear, area);
        frame.render_widget(Paragraph::new(lines).block(bordered(" Keys ", true)), area);
    }

    fn render_confirm_quit(&self, frame: &mut Frame) {
        let count = self.user_picked();
        chrome::confirm_modal(
            frame,
            " Quit? ",
            vec![Line::from(format!(
                "Quit and drop {} picked?",
                plural(count, "item")
            ))],
            ("y", "quit"),
            ("any other key", "stay"),
        );
    }

    fn render_confirm_cancel(&self, frame: &mut Frame) {
        chrome::confirm_modal(
            frame,
            " Cancel install? ",
            vec![Line::from(
                "Cancel the running install? Completed changes stay in place.",
            )],
            ("ctrl-c", "cancel"),
            ("wait", "continue installing"),
        );
    }

    // ---- choose ------------------------------------------------------------

    fn render_choose(
        &self,
        frame: &mut Frame,
        area: Rect,
        stage: &ChooseStage,
    ) -> (ListHit, ListHit, ListHit) {
        let searching = self.search.is_some();
        let profile_mode = self.profile_mode();
        let wiki_mode = self.browsing_wiki();

        // Keep the sliding Miller columns: Goals | Types | Capabilities,
        // then Types | Capabilities | Overview when inspecting an item.
        let widest = stage
            .groups
            .iter()
            .map(|group| group.title.width())
            .max()
            .unwrap_or(0) as u16;
        // A column holds ` [x] Title  10/23 ` inside the panel frame.
        const COLUMN_EXTRA: u16 = chrome::PANEL_FRAME + 5 + 1 + 5;
        let max_groups = (area.width / 3).max(1);
        let groups_width = (widest + COLUMN_EXTRA).clamp(24.min(max_groups), max_groups);
        let kinds_width = stage
            .groups
            .iter()
            .flat_map(|group| &group.kinds)
            .map(|kind| kind.title.width())
            .max()
            .unwrap_or(0) as u16
            + COLUMN_EXTRA;
        let kinds_width = if wiki_mode { 30 } else { kinds_width };
        let narrow = area.width < 70;
        let deep = profile_mode && !wiki_mode && (searching || stage.focus == Pane::Items);
        // On a goal, the third column previews the goal instead of listing
        // one type's capabilities, so picking a goal is never blind.
        let goal_card = profile_mode && !wiki_mode && !searching && stage.focus == Pane::Groups;
        let [groups_area, kinds_area, items_area, details_area] = if narrow {
            let [only] = Layout::horizontal([Constraint::Min(1)]).areas(area);
            let empty = Rect::new(area.x, area.y, 0, 0);
            match (searching, stage.focus) {
                (true, _) | (false, Pane::Items) => [empty, empty, only, empty],
                (false, Pane::Kinds) => [empty, only, empty, empty],
                (false, Pane::Groups) => [only, empty, empty, empty],
            }
        } else if area.width < 100 {
            // Two readable columns beat three clipped ones. Keep the focused
            // lane and its next lane or overview; keyboard navigation is unchanged.
            let [left, right] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Min(1)])
                    .spacing(1)
                    .areas(area);
            let empty = Rect::default();
            if wiki_mode {
                if stage.focus == Pane::Groups {
                    [left, right, empty, empty]
                } else {
                    [empty, left, right, empty]
                }
            } else if searching || stage.focus == Pane::Items {
                [empty, empty, left, right]
            } else if goal_card {
                [left, empty, empty, right]
            } else if stage.focus == Pane::Kinds {
                [empty, left, right, empty]
            } else {
                [left, empty, right, empty]
            }
        } else if deep {
            let [kinds, items, details] = Layout::horizontal([
                Constraint::Length(kinds_width),
                Constraint::Min(30),
                Constraint::Percentage(34),
            ])
            .spacing(1)
            .areas(area);
            [Rect::default(), kinds, items, details]
        } else if goal_card {
            let [groups, kinds, details] = Layout::horizontal([
                Constraint::Length(groups_width),
                Constraint::Length(kinds_width),
                Constraint::Min(30),
            ])
            .spacing(1)
            .areas(area);
            [groups, kinds, Rect::default(), details]
        } else if profile_mode || wiki_mode {
            let [groups, kinds, items] = Layout::horizontal([
                Constraint::Length(groups_width),
                Constraint::Length(kinds_width),
                Constraint::Min(30),
            ])
            .spacing(1)
            .areas(area);
            [groups, kinds, items, Rect::default()]
        } else {
            let [groups, items, details] = Layout::horizontal([
                Constraint::Length(groups_width),
                Constraint::Min(30),
                Constraint::Percentage(34),
            ])
            .spacing(1)
            .areas(area);
            [groups, Rect::default(), items, details]
        };

        // Column one: every group with its state.
        let (groups, counts): (Vec<_>, Vec<_>) = stage
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| self.group_item(index, group))
            .unzip();
        let group_count_width = counts.into_iter().max().unwrap_or(0) + 1;
        let mut group_state = TableState::default().with_selected(Some(stage.group_cursor));
        let groups_focused = stage.focus == Pane::Groups && !searching;
        if groups_area.width > 0 {
            let title = match (area.width < 100, profile_mode) {
                (true, true) if wiki_mode => " Goals · → Wikis ",
                (true, true) => " Goals · → types ",
                (false, true) => " Goals ",
                (true, false) => " Groups · → items ",
                (false, false) => " Groups ",
            };
            frame.render_stateful_widget(
                Table::new(
                    groups,
                    [
                        Constraint::Length(5),
                        Constraint::Fill(1),
                        Constraint::Length(group_count_width),
                    ],
                )
                .column_spacing(0)
                .block(bordered(title, groups_focused))
                .row_highlight_style(highlight(groups_focused)),
                groups_area,
                &mut group_state,
            );
        }

        if wiki_mode {
            let browser = &self.wiki;
            let (vaults, capabilities) = browser.draw(
                frame,
                kinds_area,
                items_area,
                stage.focus,
                &self.model.skill_destination.home,
            );
            return (
                (groups_area.width > 0).then_some((groups_area, group_state.offset())),
                vaults,
                capabilities,
            );
        }
        // Column two: capability types within the focused profile.
        let (kinds, counts): (Vec<_>, Vec<_>) = stage
            .group()
            .kinds
            .iter()
            .map(|kind| self.kind_item(kind, stage.group()))
            .unzip();
        let kind_count_width = counts.into_iter().max().unwrap_or(0) + 1;
        let mut kind_state = TableState::default().with_selected(Some(stage.kind_cursor));
        let kinds_focused = stage.focus == Pane::Kinds && !searching;
        if kinds_area.width > 0 {
            let title = if area.width < 100 {
                " Types · ← goals · → items "
            } else {
                " Types "
            };
            frame.render_stateful_widget(
                Table::new(
                    kinds,
                    [
                        Constraint::Length(5),
                        Constraint::Fill(1),
                        Constraint::Length(kind_count_width),
                    ],
                )
                .column_spacing(0)
                .block(bordered(title, kinds_focused))
                .row_highlight_style(highlight(kinds_focused)),
                kinds_area,
                &mut kind_state,
            );
        }

        // Column three: the focused type's capabilities, or search hits.
        let (rows, cursor, title): (Vec<Item>, usize, String) = match &self.search {
            Some(query) => {
                let matches = self.search_matches();
                let cursor = self.search_cursor.min(matches.len().saturating_sub(1));
                let rows = matches.iter().map(|&hit| hit.1).collect();
                (
                    rows,
                    cursor,
                    format!(" /{query}▏  {} matches ", matches.len()),
                )
            }
            None => {
                let kind = stage.kind();
                let counts = self.item_counts(&kind.rows);
                let prefix = if profile_mode {
                    format!("{} · ", kind.title)
                } else {
                    String::new()
                };
                let title = if counts.actionable() == 0 {
                    let summary = [
                        (counts.installed, "installed"),
                        (counts.required, "required"),
                        (counts.unavailable, "unavailable"),
                    ]
                    .into_iter()
                    .filter(|(count, _)| *count > 0)
                    .map(|(count, label)| format!("{count} {label}"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                    format!(
                        " {prefix}{} ",
                        if summary.is_empty() {
                            "No items"
                        } else {
                            &summary
                        }
                    )
                } else {
                    format!(" {prefix}{}/{} picked ", counts.picked, counts.actionable())
                };
                (kind.rows.clone(), stage.item_cursor, title)
            }
        };
        let kind_width = if searching { 13 } else { 0 };
        let label_width = rows
            .iter()
            .map(|row| self.row_label(row).width())
            .max()
            .unwrap_or(0)
            .min(32)
            .min(
                (items_area.width as usize)
                    .saturating_sub(chrome::PANEL_FRAME as usize + 5 + kind_width + 1),
            );
        // Descriptions already live in Overview. Do not leave a one-letter
        // fragment beside each label when a column is too narrow to read it.
        let note_width = usize::from(items_area.width)
            .saturating_sub(usize::from(chrome::PANEL_FRAME) + 5 + kind_width + label_width + 1);
        let items = rows
            .iter()
            .map(|row| self.choose_row_item(row))
            .collect::<Vec<_>>();
        let mut item_state =
            TableState::default().with_selected((!rows.is_empty()).then_some(cursor));
        let items_focused = stage.focus == Pane::Items || searching;
        if items_area.width > 0 && searching && rows.is_empty() {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled("No matches", Style::new().bold()),
                    Line::from(""),
                    Line::from("Backspace widens the search."),
                    Line::from("esc returns to browsing."),
                ])
                .wrap(Wrap { trim: true })
                .block(bordered(&title, true)),
                items_area,
            );
        } else if items_area.width > 0 {
            frame.render_stateful_widget(
                Table::new(
                    items,
                    [
                        Constraint::Length(5),
                        Constraint::Length(kind_width as u16),
                        Constraint::Length(label_width as u16 + 1),
                        if note_width >= 16 {
                            Constraint::Fill(1)
                        } else {
                            Constraint::Length(0)
                        },
                    ],
                )
                .column_spacing(0)
                .block(bordered(&title, items_focused))
                .row_highlight_style(highlight(items_focused)),
                items_area,
                &mut item_state,
            );
        }

        // Column three: what the cursor is on.
        let details = match (stage.focus, rows.get(cursor)) {
            (_, None) if searching => Vec::new(),
            (_, _) if !searching && stage.focus == Pane::Groups => {
                self.group_details(stage.group())
            }
            (_, _) if !searching && stage.focus == Pane::Kinds => self.kind_details(stage.kind()),
            (_, Some(row)) => self.row_details(row),
            (_, None) => Vec::new(),
        };
        if details_area.width > 0 {
            frame.render_widget(
                Paragraph::new(details)
                    .wrap(Wrap { trim: true })
                    .block(bordered(
                        if profile_mode {
                            " Overview "
                        } else {
                            " Details "
                        },
                        false,
                    )),
                details_area,
            );
        }
        (
            (groups_area.width > 0).then_some((groups_area, group_state.offset())),
            (kinds_area.width > 0).then_some((kinds_area, kind_state.offset())),
            (items_area.width > 0).then_some((items_area, item_state.offset())),
        )
    }

    fn item_counts(&self, items: &[super::state::Item]) -> ItemCounts {
        let mut counts = ItemCounts::default();
        for item in items {
            match self.item_state(*item) {
                ItemState::Available => counts.available += 1,
                ItemState::Picked => counts.picked += 1,
                ItemState::Required(_) | ItemState::RequiredKeep(_) => counts.required += 1,
                ItemState::Installed => counts.installed += 1,
                ItemState::Unavailable(_) => counts.unavailable += 1,
            }
        }
        counts
    }

    fn group_item(&self, index: usize, group: &Group) -> (Row<'static>, u16) {
        if self.is_wiki_group(group) {
            let browser = &self.wiki;
            let ready = !browser.vaults.is_empty()
                && browser.vaults.iter().all(|record| {
                    browser
                        .health
                        .get(&record.path)
                        .is_some_and(|health| health.healthy)
                });
            let (mark, style) = if browser.count() > 0 {
                (ON, Style::new().fg(OK))
            } else if ready {
                mark_done()
            } else {
                (" › ", Style::new().fg(ACCENT))
            };
            let count = browser.vaults.len().to_string();
            let width = count.width() as u16;
            return (
                Row::new([
                    Cell::from(Span::styled(format!(" {mark} "), style)),
                    Cell::from(group.title.clone()),
                    Cell::from(Line::from(count).right_aligned()).style(Style::new().dim()),
                ]),
                width,
            );
        }
        let goal_selected =
            (!group.everything && self.profile_mode()).then(|| self.picked_goals.contains(&index));
        self.selection_group_item(&group.title, &group.bulk_rows, goal_selected)
    }

    fn kind_item(&self, kind: &super::state::KindGroup, group: &Group) -> (Row<'static>, u16) {
        self.selection_group_item(
            &kind.title,
            &kind.bulk_items(group).collect::<Vec<_>>(),
            None,
        )
    }

    fn selection_group_item(
        &self,
        title: &str,
        items: &[super::state::Item],
        goal_selected: Option<bool>,
    ) -> (Row<'static>, u16) {
        let (mark, mark_style, count) = if items.is_empty() {
            ("   ", Style::new(), String::new())
        } else {
            let counts = self.item_counts(items);
            let actionable = counts.actionable();
            let (mark, style) = if actionable == 0 && counts.unavailable > 0 {
                (" ! ", Style::new().fg(WARN))
            } else if actionable == 0 && counts.required == 0 {
                mark_done()
            } else if let Some(selected) = goal_selected {
                mark_for(selected)
            } else if counts.picked == 0 && counts.required == 0 {
                (OFF, Style::new().dim())
            } else if counts.picked == actionable {
                (ON, Style::new().fg(OK))
            } else {
                (PART, Style::new().fg(OK))
            };
            let count = if actionable == 0 {
                // Nothing left to pick: the mark says why, the number says how many.
                format!(
                    "{}",
                    counts.installed + counts.required + counts.unavailable
                )
            } else if counts.picked == 0 {
                format!("{actionable}")
            } else {
                format!("{}/{actionable}", counts.picked)
            };
            (mark, style, count)
        };
        let width = count.width() as u16;
        (
            Row::new([
                Cell::from(Span::styled(format!(" {mark} "), mark_style)),
                Cell::from(title.to_owned()),
                Cell::from(Line::from(count).right_aligned()).style(
                    if mark == ON || mark == PART {
                        Style::new().fg(OK)
                    } else {
                        Style::new().dim()
                    },
                ),
            ]),
            width,
        )
    }

    fn row_label(&self, row: &Item) -> &str {
        match row {
            Item::Resource(index) => &self.model.resources[*index].label,
            Item::Setting(index) => &self.model.settings[*index].label,
        }
    }

    fn row_kind(&self, row: &Item) -> &'static str {
        match row {
            Item::Resource(index) => match self.model.resources[*index].kind {
                ResourceKind::Skill => "Skill",
                ResourceKind::Tool => "Tool",
                ResourceKind::PiPackage => "Pi package",
                ResourceKind::HerdrPlugin => "Herdr plugin",
                ResourceKind::McpServer => "MCP server",
            },
            Item::Setting(_) => "Setting",
        }
    }

    fn choose_row_item(&self, row: &Item) -> Row<'_> {
        let (mark, mark_style, dim_label, note) = match row {
            Item::Resource(index) => {
                let resource = &self.model.resources[*index];
                if let Some(note) = self.included_note(*index) {
                    let actionable = !self
                        .actionable(&[super::state::Item::Resource(*index)])
                        .is_empty();
                    let (mark, style) = if actionable {
                        mark_for(self.selected[*index])
                    } else {
                        (ON, Style::new().fg(OK).dim())
                    };
                    (mark, style, !actionable, note)
                } else if self.resource_installed(*index) {
                    let (mark, style) = mark_done();
                    (
                        mark,
                        style,
                        true,
                        if self.installed_globally_only(*index) {
                            format!("Installed globally · {}", resource.description)
                        } else {
                            resource.description.clone()
                        },
                    )
                } else if let Some(parent) = self.required_note(*index) {
                    if self.uninstalling() {
                        (
                            OFF,
                            Style::new().fg(WARN).dim(),
                            true,
                            format!("kept; required by {parent}"),
                        )
                    } else {
                        (
                            ON,
                            Style::new().fg(OK).dim(),
                            true,
                            format!("needed by {parent}"),
                        )
                    }
                } else {
                    let (mark, style) = mark_for(self.selected[*index]);
                    let reason = self.selection_reason(*index);
                    (
                        mark,
                        style,
                        false,
                        if self.selected[*index] && !reason.is_empty() {
                            reason
                        } else {
                            resource.description.clone()
                        },
                    )
                }
            }
            Item::Setting(index) => {
                let spec = &self.model.settings[*index];
                match self.item_state(super::state::Item::Setting(*index)) {
                    ItemState::Installed => {
                        let (mark, style) = mark_done();
                        (mark, style, true, spec.description.clone())
                    }
                    ItemState::Unavailable(reason) => {
                        (" - ", Style::new().fg(WARN).dim(), true, reason)
                    }
                    _ => {
                        let (mark, style) = mark_for(self.setting_on[*index]);
                        (mark, style, false, spec.description.clone())
                    }
                }
            }
        };
        // Scope is only worth a word when it is not the default.
        let scope = match row {
            Item::Resource(index) if self.model.resources[*index].group == "Wiki" => "Vault · ",
            Item::Resource(index)
                if matches!(
                    self.model.resources[*index].kind,
                    ResourceKind::Skill | ResourceKind::McpServer
                ) && self.skill_scope == crate::SkillScope::Project =>
            {
                "This project · "
            }
            _ => "",
        };
        Row::new([
            Cell::from(Span::styled(format!(" {mark} "), mark_style)),
            Cell::from(self.row_kind(row)).style(Style::new().dim()),
            Cell::from(self.row_label(row)).style(if dim_label {
                Style::new().dim()
            } else {
                Style::new()
            }),
            Cell::from(format!("{scope}{note}")).style(Style::new().dim()),
        ])
    }

    fn kind_details(&self, kind: &super::state::KindGroup) -> Vec<Line<'_>> {
        vec![
            Line::styled(kind.title.clone(), TITLE),
            Line::from(""),
            Line::from(self.item_counts(&kind.rows).summary()),
            Line::from(""),
            Line::styled(
                "space picks or clears this capability type.",
                Style::new().dim(),
            ),
        ]
    }

    fn group_details(&self, group: &Group) -> Vec<Line<'_>> {
        let counts = self.item_counts(&group.bulk_rows);
        let mut lines = vec![Line::styled(group.title.clone(), TITLE)];
        if group.description != group.title {
            lines.push(Line::from(""));
            lines.push(Line::from(group.description.clone()));
        }
        lines.push(Line::from(""));
        if group.everything {
            lines.push(Line::from(counts.summary()));
            lines.push(Line::from(""));
            let suffix = if self.profile_mode() { "type" } else { "group" };
            lines.push(Line::styled(
                format!("space picks every capability, or clears them all; then trim by {suffix}."),
                Style::new().dim(),
            ));
        } else {
            let actionable = counts.actionable();
            for kind in &group.kinds {
                let labels = kind
                    .bulk_items(group)
                    .map(|row| {
                        let label = self.row_label(&row);
                        match self.item_state(row) {
                            ItemState::Installed => format!("{label} ✓"),
                            _ => label.to_owned(),
                        }
                    })
                    .collect::<Vec<_>>();
                if labels.is_empty() {
                    continue;
                }
                lines.push(Line::from(vec![
                    Span::styled(kind.title.clone(), Style::new().bold()),
                    Span::styled(format!("  {}", labels.len()), Style::new().dim()),
                ]));
                lines.push(Line::styled(labels.join(", "), Style::new().dim()));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(counts.summary()));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                match (self.profile_mode(), actionable == 0) {
                    (true, true) => "→ opens the goal's types".to_owned(),
                    (true, false) => format!("space picks all {actionable} · → trims by type"),
                    (false, true) => "→ opens the group".to_owned(),
                    (false, false) => {
                        "space picks or clears available items · → opens it".to_owned()
                    }
                },
                Style::new().dim(),
            ));
        }
        lines
    }

    fn row_details(&self, row: &Item) -> Vec<Line<'_>> {
        match row {
            Item::Resource(index) => self.resource_details(*index),
            Item::Setting(index) => self.setting_details(&self.model.settings[*index], *index),
        }
    }

    fn resource_details(&self, index: usize) -> Vec<Line<'_>> {
        let resource = &self.model.resources[index];
        let mut lines = vec![
            Line::styled(resource.label.clone(), TITLE),
            Line::styled(
                format!("{} · {}", resource.kind, resource.group),
                Style::new().dim(),
            ),
            Line::from(""),
            Line::from(resource.description.clone()),
            Line::from(""),
        ];
        let reason = self.selection_reason(index);
        let needs_installed_note =
            self.resource_installed(index) && !reason.starts_with("Already installed");
        if !reason.is_empty() {
            lines.push(field("why", reason, ACCENT));
        }
        if resource.group == "Wiki" {
            lines.push(field("scope", "Vault-local".into(), ACCENT));
            lines.push(Line::from(
                "Choose a Vault in Wiki setup to check or install this resource.",
            ));
            lines.push(Line::from(
                "Global installs do not count as installed in a Vault.",
            ));
            return lines;
        }
        if matches!(resource.kind, ResourceKind::Skill | ResourceKind::McpServer)
            && self.skill_scope == crate::SkillScope::Project
        {
            lines.push(field("scope", "This project".into(), ACCENT));
        }
        if !resource.dependencies.is_empty() {
            lines.push(field("pulls in", resource.dependencies.join(", "), WARN));
        }
        match resource.kind {
            ResourceKind::McpServer => {
                lines.push(field(
                    "via",
                    "Pi MCP gateway · tools discovered on request".into(),
                    ACCENT,
                ));
                lines.push(field(
                    "agents",
                    "Pi supported; other agent adapters not yet verified".into(),
                    WARN,
                ));
            }
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
                format!("pi install {}", resource.pi_install_spec()),
                ACCENT,
            )),
            ResourceKind::HerdrPlugin => lines.push(field(
                "via",
                format!("herdr plugin install {}", resource.install_target),
                ACCENT,
            )),
        }
        if !resource.next_action.is_empty() {
            lines.push(Line::from(""));
            lines.push(field("then", resource.next_action.clone(), ACCENT));
        }
        if needs_installed_note {
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
            Line::styled(spec.label.clone(), TITLE),
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

    fn render_responses(&self, frame: &mut Frame, area: Rect, cursor: usize) -> ListHit {
        let [question, explanation, choices] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(4),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new("Do you have ADHD and want ADHD-friendly responses in Pi?")
                .style(Style::new().bold())
                .wrap(Wrap { trim: true }),
            question,
        );
        let enabled = crate::settings::setting_state(
            &crate::settings::pi_adhd_setting(),
            &self.model.settings_paths,
        ) == crate::settings::SettingState::Applied;
        let detail = if enabled {
            "Already enabled. No keeps it enabled."
        } else {
            "Yes installs the plugin and enables it for future Pi sessions."
        };
        frame.render_widget(
            Paragraph::new(detail).wrap(Wrap { trim: true }),
            explanation,
        );
        let list = List::new(["Yes — always enable", "No — leave settings unchanged"])
            .block(bordered(" Response preference ", true))
            .highlight_style(highlight(true));
        frame.render_stateful_widget(
            list,
            choices,
            &mut ListState::default().with_selected(Some(cursor)),
        );
        Some((choices, 0))
    }

    fn render_where(&self, frame: &mut Frame, area: Rect, cursor: usize) -> ListHit {
        // The agent rows already show every path, so the only extra text is
        // a note that changes what the rows mean.
        let destination = self.skill_destination();
        let skills = self.skill_count();
        let has_mcp = self.has_mcp();
        let note = if destination.agents.is_empty() {
            Some(Line::styled(
                " Pick at least one agent, or nothing can be installed.",
                Style::new().fg(WARN),
            ))
        } else if has_mcp {
            Some(Line::styled(
                " Scope applies to skills and MCP config. MCP goes to Pi only; other agents receive skills.",
                Style::new().dim(),
            ))
        } else {
            None
        };
        let [list_area, note_area] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(u16::from(note.is_some())),
        ])
        .areas(area);
        let (global, project) = match self.skill_scope {
            crate::SkillScope::Global => ("(•)", "( )"),
            crate::SkillScope::Project => ("( )", "(•)"),
        };
        let mut items = vec![Row::new([Cell::from(Line::from(vec![
            Span::styled(format!(" {global} "), Style::new().fg(OK)),
            Span::raw("All projects    "),
            Span::styled(format!("{project} "), Style::new().fg(OK)),
            Span::raw("This project"),
        ]))
        .column_span(3)])];
        let agent_width = SkillAgent::ALL
            .iter()
            .map(|agent| agent.label().width())
            .max()
            .unwrap_or(0);
        let tree_of = |agent: &SkillAgent| match self.skill_scope {
            crate::SkillScope::Global => agent.global_skill_tree(&destination.home),
            crate::SkillScope::Project => agent.project_skill_tree(&destination.project_root),
        };
        for (agent, on) in SkillAgent::ALL.iter().zip(&self.agent_on) {
            let unsupported = has_mcp && skills == 0 && *agent != SkillAgent::Pi;
            let (mark, style) = if unsupported {
                (" - ", Style::new().dim())
            } else {
                mark_for(*on)
            };
            let tree = tree_of(agent);
            // Two agents reading one folder: show it once, then point back.
            let same_as = SkillAgent::ALL
                .iter()
                .take_while(|other| *other != agent)
                .find(|other| tree_of(other) == tree)
                .map(|other| format!("= {}", other.label()));
            let detail = if unsupported {
                "MCP not yet verified".into()
            } else if has_mcp && *agent == SkillAgent::Pi {
                tidy(&crate::mcp::config_path(&destination), &destination.home)
            } else {
                same_as.unwrap_or_else(|| tidy(&tree, &destination.home))
            };
            items.push(Row::new([
                Cell::from(Span::styled(format!(" {mark} "), style)),
                Cell::from(agent.label()),
                Cell::from(detail).style(Style::new().dim()),
            ]));
        }
        let title = format!(
            " Where · {} · {}/{} agents ",
            plural(skills, "skill"),
            self.selected_agents().len(),
            SkillAgent::ALL.len()
        );
        let list = Table::new(
            items,
            [
                Constraint::Length(5),
                Constraint::Length(agent_width as u16 + 1),
                Constraint::Fill(1),
            ],
        )
        .column_spacing(0)
        .block(bordered(&title, true))
        .row_highlight_style(highlight(true));
        let mut state = TableState::default().with_selected(Some(cursor));
        frame.render_stateful_widget(list, list_area, &mut state);
        if let Some(note) = note {
            frame.render_widget(Paragraph::new(note).wrap(Wrap { trim: true }), note_area);
        }
        Some((list_area, state.offset()))
    }

    // ---- review ------------------------------------------------------------

    fn render_review(&self, frame: &mut Frame, area: Rect, scroll: u16) -> bool {
        let mut lines = Vec::new();
        if let Some(summary) = self.responses_summary() {
            lines.push(Line::from(summary));
            lines.push(Line::from(""));
        }
        if self.nothing_chosen() {
            lines.push(Line::from("Nothing picked, so nothing changes."));
        } else if self.uninstalling() {
            let selected = self.selection();
            lines.push(heading("Remove", selected.len()));
            for resource in selected {
                lines.push(Line::from(vec![
                    Span::styled("  - ", Style::new().fg(ERR).bold()),
                    Span::raw(resource.label),
                ]));
            }
            let kept = self
                .model
                .resources
                .iter()
                .enumerate()
                .filter(|(index, _)| !self.selected[*index] || self.required_note(*index).is_some())
                .collect::<Vec<_>>();
            if !kept.is_empty() {
                lines.push(Line::from(""));
                lines.push(heading("Keep", kept.len()));
                for (index, resource) in kept {
                    let note = self
                        .required_note(index)
                        .map_or_else(String::new, |parent| format!("  required by kept {parent}"));
                    lines.push(Line::from(vec![
                        Span::styled("  o ", Style::new().dim()),
                        Span::raw(resource.label.clone()),
                        Span::styled(note, Style::new().fg(WARN).dim()),
                    ]));
                }
            }
        } else {
            return self.render_setup_review(frame, area, scroll);
        }
        let paragraph = Paragraph::new(lines)
            .block(bordered(" Review ", true))
            .scroll((scroll, 0));
        frame.render_widget(paragraph, area);
        true
    }
}

pub(super) fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A path with the home directory folded to `~`.
pub(super) use crate::ui::tidy_path as tidy;

/// `label  value` with a dim, fixed-width label column.
pub(super) fn field(label: &'static str, value: String, color: Color) -> Line<'static> {
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

/// The mark for something already present: nothing left to do here.
fn mark_done() -> (&'static str, Style) {
    (" ✓ ", Style::new().fg(OK).dim())
}

fn mark_for(on: bool) -> (&'static str, Style) {
    if on {
        (ON, Style::new().fg(OK))
    } else {
        (OFF, Style::new().dim())
    }
}

pub(super) use crate::ui::chrome::panel as bordered;

pub(super) fn highlight(focused: bool) -> Style {
    if focused {
        Style::new().fg(ACCENT).add_modifier(Modifier::REVERSED)
    } else {
        Style::new().bold()
    }
}
