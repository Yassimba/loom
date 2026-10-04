//! A scripted [`System`] for tests. Public (but hidden) only so the
//! integration tests under `tests/` share one fake with the unit tests.

use crate::{CommandResult, CommandSpec, System};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Mutex;

/// A successful result printing `stdout`.
pub fn ok(stdout: impl Into<String>) -> CommandResult {
    CommandResult {
        success: true,
        stdout: stdout.into(),
        stderr: String::new(),
    }
}

/// A failed result printing `stderr`.
pub fn failed(stderr: impl Into<String>) -> CommandResult {
    CommandResult {
        success: false,
        stdout: String::new(),
        stderr: stderr.into(),
    }
}

/// Answers commands from a script and records every command it was asked
/// to run. Unless told otherwise every binary exists and every command
/// succeeds with no output.
pub struct ScriptedSystem {
    only: Option<Vec<String>>,
    without: Vec<String>,
    rules: Vec<(Vec<String>, CommandResult)>,
    otherwise: CommandResult,
    home: Option<PathBuf>,
    cwd: Option<PathBuf>,
    calls: Mutex<Vec<CommandSpec>>,
}

impl Default for ScriptedSystem {
    fn default() -> Self {
        Self {
            only: None,
            without: Vec::new(),
            rules: Vec::new(),
            otherwise: ok(""),
            home: None,
            cwd: None,
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl ScriptedSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Only these binaries exist; `&[]` means none do.
    pub fn only(mut self, names: &[&str]) -> Self {
        self.only = Some(names.iter().map(ToString::to_string).collect());
        self
    }

    /// This binary does not exist.
    pub fn without(mut self, name: &str) -> Self {
        self.without.push(name.into());
        self
    }

    /// Answer matching commands with `result`; the first matching rule wins.
    /// The first word of `pattern` is the program, the remaining words must
    /// appear among the arguments in that order: `"pi list"` matches
    /// `pi list --json`, `"mise doctor"` matches `mise exec -- python doctor`.
    pub fn on(mut self, pattern: &str, result: CommandResult) -> Self {
        let words = pattern.split_whitespace().map(str::to_owned).collect();
        self.rules.push((words, result));
        self
    }

    /// Answer every command no rule matches with `result`.
    pub fn otherwise(mut self, result: CommandResult) -> Self {
        self.otherwise = result;
        self
    }

    pub fn home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Every command run so far, in order.
    pub fn calls(&self) -> Vec<CommandSpec> {
        self.calls.lock().unwrap().clone()
    }

    /// [`Self::calls`] as `CommandSpec::display` strings.
    pub fn shown(&self) -> Vec<String> {
        self.calls().iter().map(CommandSpec::display).collect()
    }
}

fn matches(words: &[String], command: &CommandSpec) -> bool {
    let Some((program, wanted)) = words.split_first() else {
        return false;
    };
    let mut args = command.args.iter();
    *program == command.program && wanted.iter().all(|word| args.any(|arg| arg == word))
}

impl System for ScriptedSystem {
    fn command_exists(&self, name: &str) -> bool {
        !self.without.iter().any(|missing| missing == name)
            && self
                .only
                .as_ref()
                .is_none_or(|only| only.iter().any(|known| known == name))
    }

    fn refresh_path(&self) {}

    fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
        self.calls.lock().unwrap().push(command.clone());
        Ok(self
            .rules
            .iter()
            .find(|(words, _)| matches(words, command))
            .map_or(&self.otherwise, |(_, result)| result)
            .clone())
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone().or_else(std::env::home_dir)
    }

    fn current_dir(&self) -> Option<PathBuf> {
        self.cwd.clone().or_else(|| std::env::current_dir().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_match_program_and_ordered_arguments_and_calls_are_recorded() {
        let system = ScriptedSystem::new()
            .only(&["pi"])
            .on("pi list", ok("listing"))
            .otherwise(failed("nope"));
        assert!(system.command_exists("pi") && !system.command_exists("mise"));
        let run = |program, args: &[&str]| {
            system
                .run(&CommandSpec::new(program, args.iter().copied()))
                .unwrap()
        };
        assert_eq!(run("pi", &["-l", "list", "--json"]), ok("listing"));
        assert_eq!(run("pi", &["install", "list-tools"]), failed("nope"));
        assert_eq!(run("npm", &["list"]), failed("nope"));
        assert_eq!(system.shown()[0], "pi -l list --json");
        assert_eq!(system.calls().len(), 3);
    }
}
