//! Read-only setup review: selected capabilities, prerequisites, and repair/next steps.
use super::render::{bordered, plural, tidy, ACCENT, TITLE};
use super::state::Wizard;
use crate::{InstallPlan, Resource, ResourceKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

const TITLES: [&str; 3] = [
    " Selected capabilities ",
    " Required to work ",
    " Writes & notes ",
];

impl Wizard {
    pub(super) fn render_setup_review(&self, frame: &mut Frame, area: Rect, scroll: u16) {
        let plan = self.plan();
        let expanded = self.expanded_selection();
        let wiki = expanded.iter().any(|r| r.group == "Wiki");
        // The footer already says enter/esc; only a blocked plan or a
        // different-than-usual enter needs a line of its own.
        let message = match &plan {
            Err(_) => Some("Cannot install. See Writes & notes; esc goes back."),
            Ok(_) if self.model.dry_run => Some("Dry run: enter prints this plan and exits."),
            Ok(_) if wiki && expanded.iter().all(|r| r.group == "Wiki") => {
                Some("Enter opens Vault setup.")
            }
            Ok(_) if wiki => Some("Enter installs general items, then opens Vault setup."),
            Ok(_) => None,
        };
        let [headline, body, confirmation] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(match (&plan, message) {
                (Err(_), _) => 3,
                (_, Some(_)) => 1,
                (_, None) => 0,
            }),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(self.review_headline(&expanded, plan.as_ref().ok()))
                .wrap(Wrap { trim: true })
                .style(Style::new().fg(ACCENT)),
            headline,
        );
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
                        .block(bordered(title, true))
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
                .filter(|(lines, _)| !lines.is_empty())
                .flat_map(|(mut lines, title)| {
                    while lines.last().is_some_and(|line| line.width() == 0) {
                        lines.pop();
                    }
                    [Line::styled(title.trim(), TITLE)]
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
                    .block(bordered(" Review · scroll to inspect ", true))
                    .scroll((scroll.min(max_scroll.min(u16::MAX as usize) as u16), 0)),
                body,
            );
        }
        if let Some(message) = message {
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
    }

    /// One line that sizes the whole job: counts by kind and a rough time.
    fn review_headline(&self, resources: &[Resource], plan: Option<&InstallPlan>) -> String {
        let count = |kind: ResourceKind| resources.iter().filter(|r| r.kind == kind).count();
        let mut parts = Vec::new();
        let skills = count(ResourceKind::Skill);
        if skills > 0 {
            let folders = self.skill_destination().trees().len();
            parts.push(format!(
                "{} → {}",
                plural(skills, "skill"),
                plural(folders, "agent folder")
            ));
        }
        for (kind, noun) in [
            (ResourceKind::Tool, "tool"),
            (ResourceKind::PiPackage, "Pi package"),
            (ResourceKind::HerdrPlugin, "Herdr plugin"),
            (ResourceKind::McpServer, "MCP server"),
        ] {
            let n = count(kind);
            if n > 0 {
                parts.push(plural(n, noun));
            }
        }
        let settings = self.selected_settings().len();
        if settings > 0 {
            parts.push(plural(settings, "setting"));
        }
        if parts.is_empty() {
            return String::new();
        }
        // ponytail: flat per-step guess; measure real durations if it misleads.
        let steps = plan.map_or(0, |plan| plan.prerequisites.len() + plan.resources.len());
        let seconds = 20 * steps.max(1)
            + if plan.is_some_and(|plan| !plan.prerequisites.is_empty()) {
                60
            } else {
                0
            };
        let time = match seconds {
            s if s < 60 => "under a minute".to_owned(),
            s => format!("about {} min", s.div_ceil(60)),
        };
        format!("{} · {time}", parts.join(" · "))
    }

    fn review_columns(
        &self,
        resources: &[Resource],
        plan: Option<&InstallPlan>,
    ) -> [Vec<Line<'static>>; 3] {
        let mut selected = Vec::new();
        let mut required = Vec::new();
        let mut notes = Vec::new();
        if let Some(summary) = self.responses_summary() {
            notes.push(Line::from(summary));
            if self.adhd_enabled {
                notes.push(Line::from("Always-on flag: .i-have-adhd-always"));
            }
            notes.push(Line::from(""));
        }
        for wiki in self.wiki_jobs().unwrap_or_default() {
            selected.push(Line::styled(wiki.label(), TITLE));
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
        let label_width = resources.iter().map(|r| r.label.len()).max().unwrap_or(0);
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
            lines.push(Line::from(vec![
                Span::styled(format!("+ {:<label_width$}", resource.label), TITLE),
                Span::styled(
                    format!("  {}", self.selection_reason(index)),
                    Style::new().dim(),
                ),
            ]));
        }
        if let Some(plan) = plan {
            for step in &plan.prerequisites {
                required.push(Line::from(""));
                required.push(Line::styled(
                    format!("Install first: {}", step.target),
                    Style::new().bold(),
                ));
                required.push(Line::from(step.action.display()));
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
            if upgrade_adapter {
                notes.push(Line::styled(
                    format!(
                        "Repair: upgrade the official MCP adapter to {}",
                        crate::mcp::ADAPTER_SPEC
                    ),
                    Style::new().fg(Color::Green),
                ));
                notes.push(Line::from(""));
            }
        }
        let settings = self.selected_settings();
        if !settings.is_empty() {
            selected.push(Line::from(""));
            for spec in settings {
                selected.push(Line::from(vec![
                    Span::styled(format!("+ {}", spec.label), Style::new().bold()),
                    Span::styled(
                        format!(
                            "  {}",
                            tidy(
                                spec.target_path(&self.model.settings_paths),
                                &self.model.skill_destination.home
                            )
                        ),
                        Style::new().dim(),
                    ),
                ]));
            }
        }
        if required.is_empty() {
            required.push(Line::styled("Nothing extra needed.", Style::new().dim()));
        }
        let destination = self.skill_destination();
        if self.has_skills() {
            notes.push(Line::styled("Skills go to", Style::new().bold()));
            for tree in destination.trees() {
                notes.push(Line::from(tidy(&tree, &destination.home)));
            }
            notes.push(Line::from(""));
        }
        if self.has_mcp() {
            notes.push(Line::styled("MCP config", Style::new().bold()));
            notes.push(Line::from(tidy(
                &crate::mcp::config_path(&destination),
                &destination.home,
            )));
            notes.push(Line::from(crate::mcp::EXPOSURE_NOTE));
            notes.push(Line::from(""));
        }
        if resources.iter().any(|r| r.group == "Wiki") {
            notes.push(Line::styled("Vault setup follows", Style::new().bold()));
            notes.push(Line::from(
                "Choose Create or Connect after this review; Vault files get their own preview.",
            ));
            notes.push(Line::from(""));
        }
        while notes.last().is_some_and(|line| line.width() == 0) {
            notes.pop();
        }
        [selected, required, notes]
    }
}
