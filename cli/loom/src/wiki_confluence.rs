//! Credentials are transient form state, never Debug/registry/command arguments.
use crate::{CommandSpec, System};
use anyhow::{Context, Result};
use ratatui::crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use ratatui::widgets::{Block, Paragraph, Wrap};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[derive(Default)]
struct Credentials {
    url: String,
    username: String,
    token: String,
    pat: bool,
    field: usize,
}
impl Credentials {
    fn text(&self) -> String {
        format!("Confluence URL: {}\nEmail / username: {}\nToken: {}\nAuthentication: {}\n\nEditing: {}\n\nShared CME config: plaintext, owner-only permissions.\nNot saved in the Vault or Keychain.\nNo network authentication check will run.\n\nTab / ↑↓ field · type or paste · Space switches auth mode\nEnter on authentication submits · Esc keeps existing config",
            self.url, self.username, if self.token.is_empty() { "" } else { "********" },
            if self.pat { "PAT (username optional)" } else { "API token (username required)" },
            ["URL", "email / username", "token", "authentication mode"][self.field])
    }
    fn insert(&mut self, text: &str) {
        let target = match self.field {
            0 => &mut self.url,
            1 => &mut self.username,
            2 => &mut self.token,
            _ => return,
        };
        for c in text.chars().filter(|c| !c.is_control()) {
            if target.len() + c.len_utf8() > 16384 {
                break;
            }
            target.push(c);
        }
    }
    fn handle(&mut self, event: Event) -> Option<bool> {
        match event {
            Event::Paste(text) => self.insert(&text),
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    return Some(false);
                }
                match key.code {
                    KeyCode::Tab | KeyCode::Down => self.field = (self.field + 1) % 4,
                    KeyCode::BackTab | KeyCode::Up => self.field = (self.field + 3) % 4,
                    KeyCode::Enter if self.field == 3 => return Some(true),
                    KeyCode::Enter => self.field += 1,
                    KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right if self.field == 3 => {
                        self.pat = !self.pat
                    }
                    KeyCode::Backspace => match self.field {
                        0 => {
                            self.url.pop();
                        }
                        1 => {
                            self.username.pop();
                        }
                        2 => {
                            self.token.pop();
                        }
                        _ => {}
                    },
                    KeyCode::Char(c) => self.insert(&c.to_string()),
                    _ => {}
                }
            }
            _ => {}
        }
        None
    }
}

fn form() -> Result<Option<Credentials>> {
    let mut fields = Credentials::default();
    let mut terminal = ratatui::init();
    let result = (|| -> Result<Option<Credentials>> {
        execute!(std::io::stdout(), EnableBracketedPaste)?;
        loop {
            terminal.draw(|frame| {
                frame.render_widget(
                    Paragraph::new(fields.text())
                        .wrap(Wrap { trim: false })
                        .block(Block::bordered().title(" Confluence authentication ")),
                    frame.area(),
                );
            })?;
            if let Some(submit) = fields.handle(event::read()?) {
                return Ok(submit.then_some(fields));
            }
        }
    })();
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn adapter(system: &dyn System, command: &CommandSpec, input: Value) -> Result<Value> {
    let bytes = serde_json::to_vec(&input)?;
    let result = system
        .run_streamed(
            command,
            Duration::from_secs(15),
            &AtomicBool::new(false),
            Some(&bytes),
            &|_| {},
        )
        .map_err(|_| {
            anyhow::anyhow!("CME configuration helper failed; credentials were not logged")
        })?;
    anyhow::ensure!(result.success, "Cannot configure CME: invalid input/config, unsafe path, or concurrent change. Existing credentials were kept.");
    serde_json::from_str(&result.stdout).map_err(|_| anyhow::anyhow!("Invalid CME helper response"))
}

pub(crate) fn configure(system: &dyn System) -> Result<()> {
    let Some(credentials) = form()? else {
        return Ok(());
    };
    let root = system.run_probe(&CommandSpec::new(
        "mise",
        ["where", "pipx:confluence-markdown-exporter"],
    ))?;
    anyhow::ensure!(root.success, "Cannot locate the installed CME runtime");
    let root = PathBuf::from(root.stdout.trim());
    anyhow::ensure!(root.is_absolute(), "Invalid CME runtime path");
    // mise's pipx backend installs this exact-pinned package in its own venv.
    let python = root.join("confluence-markdown-exporter/bin/python");
    anyhow::ensure!(
        python.is_file(),
        "CME Python runtime is missing; repair the selected exporter"
    );
    let command = CommandSpec::new(
        python.to_str().context("invalid CME runtime path")?,
        ["-c", include_str!("wiki_confluence.py")],
    );
    let inspected = adapter(
        system,
        &command,
        json!({"action": "inspect", "url": credentials.url}),
    )?;
    let exists = inspected["exists"]
        .as_bool()
        .context("Invalid CME helper response")?;
    let title = if exists {
        "Replace credentials for this Confluence URL?"
    } else {
        "Save Confluence credentials?"
    };
    if !crate::wiki_tui::confirm(
        title,
        &[
            "Stored in shared CME plaintext config with owner-only permissions, not Keychain."
                .into(),
            "Other accounts and settings remain unchanged. Connectivity is not verified.".into(),
        ],
    )? {
        return Ok(());
    }
    adapter(
        system,
        &command,
        json!({
            "action": "save", "url": credentials.url, "username": credentials.username,
            "token": credentials.token, "pat": credentials.pat, "replace": exists,
            "digest": inspected["digest"],
        }),
    )?;
    println!("Confluence configuration saved. Connectivity not verified; CME environment overrides still apply.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;
    #[test]
    fn private_adapter_never_puts_tokens_in_argv_or_errors() {
        struct Fake;
        impl System for Fake {
            fn command_exists(&self, _: &str) -> bool {
                true
            }
            fn refresh_path(&self) {}
            fn run(&self, _: &CommandSpec) -> Result<crate::CommandResult> {
                unreachable!()
            }
            fn run_streamed(
                &self,
                command: &CommandSpec,
                _: Duration,
                _: &AtomicBool,
                input: Option<&[u8]>,
                _: &(dyn Fn(&[u8]) + Sync),
            ) -> Result<crate::CommandResult> {
                assert!(!format!("{command:?}").contains("SECRET-FIXTURE"));
                assert!(std::str::from_utf8(input.unwrap())
                    .unwrap()
                    .contains("SECRET-FIXTURE"));
                anyhow::bail!("private helper failure SECRET-FIXTURE");
            }
        }
        let command = CommandSpec::new("python", ["-c", include_str!("wiki_confluence.py")]);
        let error = adapter(&Fake, &command, json!({"token": "SECRET-FIXTURE"}))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("SECRET-FIXTURE"));
    }

    #[test]
    fn credentials_mask_paste_and_keep_navigation_letters_literal() {
        let mut fields = Credentials {
            field: 2,
            ..Default::default()
        };
        for c in "qjk".chars() {
            fields.handle(Event::Key(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            )));
        }
        fields.handle(Event::Paste("private-token\r\n\x1b".into()));
        assert_eq!(fields.token, "qjkprivate-token");
        assert!(!fields.text().contains("private-token"));
        assert!(fields.text().contains("********"));
        assert_eq!(
            fields.handle(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))),
            Some(false)
        );
    }
}
