//! Read-only setup review: selected capabilities, prerequisites, and repair/next steps.
use super::render::{bordered, tidy, ACCENT};
use super::state::{Stage, Wizard};
use crate::{InstallPlan, Resource, ResourceKind, SkillScope};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Padding, Paragraph, Wrap};
use ratatui::Frame;

const TITLES: [&str; 3] = [
    " Selected capabilities ",
    " Required to work ",
    " Repairs & next steps ",
];

impl Wizard {
    pub(super) fn render_setup_review(&self, frame: &mut Frame, area: Rect, scroll: u16) {
        let plan = self.plan();
        let expanded = self.expanded_selection();
        let wiki = expanded.iter().any(|r| r.group == "Wiki");
        let message = match &plan {
            Err(_) => "Cannot install. See Repairs & next steps; Esc goes back.",
            Ok(_) if self.model.dry_run => "Dry run: enter prints this plan and exits.",
            Ok(_) if wiki && expanded.iter().all(|r| r.group == "Wiki") => {
                "Enter opens Vault setup. Esc goes back."
            }
            Ok(_) if wiki => "Enter installs general items, then opens Vault setup. Esc goes back.",
            Ok(_) => "Enter installs these items. Esc goes back to change your picks.",
        };
        let [body, confirmation] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(if plan.is_err() { 4 } else { 2 }),
        ])
        .areas(area);
        let mut columns = self.review_columns(&expanded, plan.as_ref().ok());
        if let Err(error) = &plan {
            columns[2].insert(
                0,
                Line::styled(
                    format!("Cannot install: {error}"),
                    Style::new().fg(Color::Red),
                ),
            );
        }
        if body.width >= 120 {
            let areas = Layout::horizontal([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .spacing(1)
            .split(body);
            for ((lines, title), area) in columns.into_iter().zip(TITLES).zip(areas.iter()) {
                let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
                let max_scroll = paragraph
                    .line_count(area.width.saturating_sub(4))
                    .saturating_sub(area.height.saturating_sub(2) as usize);
                frame.render_widget(
                    paragraph
                        .block(bordered(title, true).padding(Padding::horizontal(1)))
                        .scroll((scroll.min(max_scroll.min(u16::MAX as usize) as u16), 0)),
                    *area,
                );
            }
        } else {
            let [selected, required, notes] = columns;
            let sections = if wiki && expanded.iter().all(|r| r.group == "Wiki") {
                [
                    (notes, TITLES[2]),
                    (selected, TITLES[0]),
                    (required, TITLES[1]),
                ]
            } else {
                [
                    (selected, TITLES[0]),
                    (required, TITLES[1]),
                    (notes, TITLES[2]),
                ]
            };
            let lines = sections
                .into_iter()
                .flat_map(|(lines, title)| {
                    [Line::styled(title.trim(), Style::new().fg(ACCENT).bold())]
                        .into_iter()
                        .chain(lines)
                        .chain([Line::from("")])
                })
                .collect::<Vec<_>>();
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            let max_scroll = paragraph
                .line_count(body.width.saturating_sub(4))
                .saturating_sub(body.height.saturating_sub(2) as usize);
            frame.render_widget(
                paragraph
                    .block(
                        bordered(" Review · scroll to inspect ", true)
                            .padding(Padding::horizontal(1)),
                    )
                    .scroll((scroll.min(max_scroll.min(u16::MAX as usize) as u16), 0)),
                body,
            );
        }
        frame.render_widget(
            Paragraph::new(message)
                .wrap(Wrap { trim: true })
                .style(Style::new().fg(if plan.is_ok() {
                    Color::Green
                } else {
                    Color::Red
                })),
            confirmation,
        );
    }

    fn review_columns(
        &self,
        resources: &[Resource],
        plan: Option<&InstallPlan>,
    ) -> [Vec<Line<'static>>; 3] {
        let mut selected = vec![
            Line::from("Included by your goals or picked individually."),
            Line::from(""),
        ];
        let mut required = vec![
            Line::from("Needed by the capabilities you chose."),
            Line::from(""),
        ];
        let mut notes = vec![
            Line::from("Only reviewed, safe changes run automatically."),
            Line::from(""),
        ];
        if self
            .visible_stages()
            .iter()
            .any(|index| matches!(self.stages[*index], Stage::Responses { .. }))
        {
            notes.push(Line::from(if self.adhd_enabled {
                "Pi responses: always enable ADHD-friendly responses"
            } else {
                "Pi responses: leave settings unchanged"
            }));
            if self.adhd_enabled {
                notes.push(Line::from("Always-on flag: .i-have-adhd-always"));
            }
            notes.push(Line::from(""));
        }
        for wiki in self.wiki_jobs().unwrap_or_default() {
            selected.push(Line::styled(wiki.label(), Style::new().fg(ACCENT).bold()));
            selected.push(Line::from(wiki.record.path.display().to_string()));
            selected.extend(
                wiki.labels
                    .iter()
                    .map(|label| Line::from(format!("+ {label}"))),
            );
            selected.push(Line::from(
                "Skills → this Wiki’s .agents/skills; packages → this Wiki’s .pi",
            ));
            selected.push(Line::from(""));
            if wiki.request.is_some() {
                required.push(Line::styled("Wiki essentials", Style::new().bold()));
                required.push(Line::from("Vault-local claude-obsidian + QMD skill/index. Shared pinned runtimes: Python, Pi, claude-obsidian, QMD."));
                if wiki.record.confluence {
                    required.push(Line::from(
                        "Confluence: shared exporter + Vault-local skill; credentials unchanged.",
                    ));
                }
                notes.push(Line::from(format!("{}: exact file changes require approval during Install. No separate setup wizard, sign-in or launch.", wiki.record.path.display())));
            }
            for step in &wiki.plan.prerequisites {
                required.push(Line::from(format!(
                    "Shared requirement: {}",
                    step.action.display()
                )));
            }
            for resource in &wiki.resources {
                if !wiki.labels.contains(&resource.label) && resource.kind == ResourceKind::Skill {
                    required.push(Line::from(format!(
                        "{} → {}",
                        resource.label,
                        wiki.record.path.display()
                    )));
                }
            }
            required.push(Line::from(""));
        }
        let direct = self.selection();
        for resource in resources {
            let index = self
                .model
                .resources
                .iter()
                .position(|r| r.id == resource.id)
                .unwrap();
            let automatic = self.setup_requirement(index);
            let lines = if !automatic && direct.iter().any(|r| r.id == resource.id) {
                &mut selected
            } else {
                &mut required
            };
            lines.push(Line::styled(
                format!("+ {}", resource.label),
                Style::new().fg(ACCENT).bold(),
            ));
            lines.push(Line::from(self.selection_reason(index)));
            lines.push(Line::from(self.review_destination(resource)));
            lines.push(Line::from(""));
        }
        if let Some(plan) = plan {
            for step in &plan.prerequisites {
                required.push(Line::styled(
                    format!("Install first: {}", step.target),
                    Style::new().bold(),
                ));
                required.push(Line::from(step.action.display()));
                required.push(Line::from(""));
            }
            let destination = self.skill_destination();
            let upgrade_adapter = plan
                .resources
                .iter()
                .any(|step| step.target == "pi-package:pi-mcp-adapter")
                && destination
                    .home
                    .join(".pi/agent/npm/node_modules/pi-mcp-adapter/package.json")
                    .is_file()
                && crate::mcp::adapter_needed(&destination).is_ok_and(|needed| needed);
            notes.push(Line::styled(
                if upgrade_adapter {
                    format!(
                        "Repair: upgrade the official MCP adapter to {}",
                        crate::mcp::ADAPTER_SPEC
                    )
                } else {
                    "No automatic repairs planned.".into()
                },
                Style::new().fg(Color::Green),
            ));
        }
        for spec in self.selected_settings() {
            selected.push(Line::styled(
                format!("Setting: {}", spec.label),
                Style::new().bold(),
            ));
            selected.push(Line::from(
                spec.target_path(&self.model.settings_paths)
                    .display()
                    .to_string(),
            ));
            selected.push(Line::from(""));
        }
        let destination = self.skill_destination();
        if self.has_skills() {
            notes.push(Line::from(""));
            notes.push(Line::styled("Agent skill folders", Style::new().bold()));
            for tree in destination.trees() {
                notes.push(Line::from(tidy(&tree, &destination.home)));
            }
        }
        if self.has_mcp() {
            notes.push(Line::from(""));
            notes.push(Line::from(
                crate::mcp::config_path(&destination).display().to_string(),
            ));
            notes.push(Line::from(crate::mcp::EXPOSURE_NOTE));
        }
        if resources.iter().any(|r| r.group == "Wiki") {
            notes.push(Line::from(""));
            notes.push(Line::styled("Vault setup follows", Style::new().bold()));
            notes.push(Line::from("→ choose Create or Connect after this review"));
            notes.push(Line::from(
                "Vault files get their own preview before anything changes.",
            ));
        }
        notes.push(Line::from(""));
        notes.push(Line::styled("After installation", Style::new().bold()));
        notes.push(Line::from(
            "Accounts and live connections are not checked here. No automatic sign-in.",
        ));
        notes.push(Line::from(""));
        notes.push(Line::from("Custom sources and edited files stay protected. Failed steps can retry without discarding completed work."));
        [selected, required, notes]
    }

    fn review_destination(&self, resource: &Resource) -> String {
        if resource.group == "Wiki" {
            return "Vault-local · choose a Vault next".into();
        }
        match resource.kind {
            ResourceKind::Skill | ResourceKind::McpServer
                if self.skill_scope == SkillScope::Project =>
            {
                format!(
                    "This project · {}",
                    self.model.skill_destination.project_root.display()
                )
            }
            ResourceKind::Skill => "All projects · selected agents".into(),
            ResourceKind::McpServer => "Global · Pi configuration".into(),
            _ => "Global · this machine".into(),
        }
    }
}
