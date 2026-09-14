//! The Install stage: progress gauge, task list, and the result summary.
use super::render::{bordered, field, highlight, plural};
use super::render::{ACCENT, ERR, OK, SPINNER, TITLE, WARN};
use super::state::{ExecStatus, InstallStage, Wizard};
use crate::ui::chrome;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Gauge, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl Wizard {
    pub(super) fn render_install(&self, frame: &mut Frame, area: Rect, stage: &InstallStage) {
        let mut goal_actions = stage
            .report
            .as_ref()
            .map(|report| self.goal_next_actions(report))
            .unwrap_or_default();
        if let Some(report) = &stage.report {
            for wiki in self.reviewed_job.iter().flat_map(|job| &job.wikis) {
                if report.installed.contains(&wiki.id()) {
                    goal_actions.push((
                        wiki.label(),
                        format!(
                            "Open {} and run pi{}",
                            wiki.record.path.display(),
                            if wiki.record.confluence {
                                "; configure Confluence with cme config edit auth.confluence"
                            } else {
                                ""
                            }
                        ),
                    ));
                }
            }
        }
        let total = stage.items.len().max(1);
        let done = stage
            .items
            .iter()
            .filter(|item| {
                !matches!(
                    item.status,
                    ExecStatus::Pending | ExecStatus::Running | ExecStatus::Verifying
                )
            })
            .count();
        let failed = stage
            .items
            .iter()
            .filter(|item| matches!(item.status, ExecStatus::Failed(_)))
            .count();
        let gauge_color = if failed > 0 { ERR } else { OK };
        let active = if stage.running {
            stage
                .items
                .iter()
                .position(|item| matches!(item.status, ExecStatus::Running | ExecStatus::Verifying))
                .unwrap_or(done.saturating_sub(1))
        } else {
            (stage.scroll as usize).min(stage.items.len().saturating_sub(1))
        };
        // The summary takes only the height its text needs; the task list
        // keeps the rest.
        let summary = stage
            .report
            .as_ref()
            .map(|report| self.install_summary(stage, report, active, &goal_actions));
        let summary_height = summary.as_ref().map_or(0, |paragraph| {
            (paragraph.line_count(area.width.saturating_sub(chrome::PANEL_FRAME)) as u16 + 2)
                .min(area.height.saturating_sub(6))
        });
        let [gauge_area, steps_area, summary_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(summary_height),
        ])
        .areas(area);
        frame.render_widget(
            Gauge::default()
                .block(bordered(
                    if stage.running {
                        " Install and verify "
                    } else {
                        " Result "
                    },
                    true,
                ))
                .gauge_style(Style::new().fg(gauge_color).bg(Color::DarkGray))
                .ratio(done as f64 / total as f64)
                .label(format!(
                    "{done} of {total}{}{}",
                    if failed > 0 {
                        format!(" · {failed} failed")
                    } else {
                        String::new()
                    },
                    match if stage.running {
                        stage.started.map(|started| started.elapsed())
                    } else {
                        Some(stage.elapsed)
                    } {
                        Some(elapsed) if elapsed.as_secs() > 0 => {
                            format!(" · {}s", elapsed.as_secs())
                        }
                        _ => String::new(),
                    }
                )),
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
                    ExecStatus::Verifying => (
                        spinner.to_string(),
                        Style::new().fg(ACCENT),
                        "Verifying installation".into(),
                    ),
                    ExecStatus::Ok(note) => ("✓".into(), Style::new().fg(OK), note.clone()),
                    ExecStatus::Failed(message) => (
                        "✗".into(),
                        Style::new().fg(ERR),
                        crate::ui::failure_advice(message).0.into(),
                    ),
                    ExecStatus::Skipped(message) => {
                        ("⊘".into(), Style::new().fg(WARN), message.clone())
                    }
                };
                let mut spans = Vec::new();
                if !matches!(item.status, ExecStatus::Pending) {
                    let elapsed = item
                        .started
                        .map_or(item.elapsed, |started| started.elapsed());
                    spans.push(Span::styled(
                        format!("{}s · ", elapsed.as_secs()),
                        Style::new().dim(),
                    ));
                }
                if !note.is_empty() {
                    spans.push(Span::styled(
                        note.lines().next().unwrap_or(&note).to_owned(),
                        style.add_modifier(Modifier::DIM),
                    ));
                }
                Row::new([
                    Cell::from(Span::styled(format!(" {mark} "), style)),
                    Cell::from(item.label.as_str()),
                    Cell::from(Line::from(spans)),
                ])
            })
            .collect::<Vec<_>>();
        let list = Table::new(
            items,
            [
                Constraint::Length(3),
                Constraint::Length(label_width as u16 + 1),
                Constraint::Fill(1),
            ],
        )
        .column_spacing(0)
        .block(bordered(" Tasks ", false));
        let mut state = TableState::default().with_selected(Some(active));
        frame.render_stateful_widget(
            list.row_highlight_style(if stage.running {
                Style::default()
            } else {
                highlight(true)
            }),
            steps_area,
            &mut state,
        );

        if let Some(paragraph) = summary {
            let max_scroll = paragraph
                .line_count(summary_area.width.saturating_sub(chrome::PANEL_FRAME))
                .saturating_sub(summary_area.height.saturating_sub(2) as usize);
            frame.render_widget(
                paragraph.block(bordered(" Result ", true)).scroll((
                    stage.scroll.min(max_scroll.min(u16::MAX as usize) as u16),
                    0,
                )),
                summary_area,
            );
        }
    }

    fn install_summary(
        &self,
        stage: &InstallStage,
        report: &crate::InstallReport,
        active: usize,
        goal_actions: &[(String, String)],
    ) -> Paragraph<'static> {
        let mut lines = Vec::new();
        if report.failures.is_empty() {
            lines.push(Line::styled("✓ Ready to try", Style::new().fg(OK).bold()));
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
                "{} installed. Completed work stays.",
                report.installed.len()
            )));
        }
        if let Some(item) = stage.items.get(active) {
            if let ExecStatus::Failed(message) = &item.status {
                let (cause, recovery) = crate::ui::failure_advice(message);
                lines.push(field("cause", cause.into(), ERR));
                lines.push(Line::from(recovery));
            }
            if stage.show_details {
                lines.push(field("details", item.detail.clone(), ACCENT));
            }
        }
        if report.failures.is_empty() {
            for (goal, action) in goal_actions {
                lines.push(Line::styled(goal.clone(), TITLE));
                lines.push(Line::from(action.clone()));
            }
            if let Some(command) = self.next_command() {
                lines.push(Line::styled(
                    match &stage.copied {
                        Some(copied) if copied == &command => {
                            format!("✓ copied `{command}` · paste it in your shell")
                        }
                        _ => format!("c copies `{command}` to the clipboard"),
                    },
                    Style::new().fg(ACCENT),
                ));
            }

            if goal_actions.is_empty() {
                lines.push(field(
                    "next",
                    crate::app::install_next_action(
                        self.model.mode,
                        &self.expanded_selection(),
                        report,
                    ),
                    ACCENT,
                ));
            }
        }
        Paragraph::new(lines).wrap(Wrap { trim: true })
    }
}
