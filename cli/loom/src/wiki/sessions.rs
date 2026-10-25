//! Copies coding-agent sessions into a Vault as Markdown, so qmd can search them.
//!
//! Output: `<vault>/.raw/agent-sessions/<agent>/<project>/<date>-<id>.md`.
//! The folder is Git-ignored, because sessions can contain secrets. Each file
//! keeps the prompts, the answers, compaction summaries and one line per tool
//! call. Tool output and model thinking are dropped: they are large and noisy.
//! Subagent sessions are skipped, because the parent session reports them.
//!
//! `.sync-state.json` records the version (mtime or upstream update time) of
//! each source, so a run converts only the sessions that changed.
use super::product::{canonical_vault, qmd_index};
use super::registry::WikiRegistry;
use crate::{CommandSpec, System};
use anyhow::{Context, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

const OUT: &str = ".raw/agent-sessions";
const COLLECTION: &str = "agent-sessions";
const USER_CAP: usize = 4000;
const ASSISTANT_CAP: usize = 6000;
const TOOL_CAP: usize = 200;
const AMP_PAGE: usize = 500;

static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"sk-[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_\w{20,}|glpat-[\w-]{20,}",
        r"|npm_[A-Za-z0-9]{20,}|pypi-[\w-]{20,}|AKIA[0-9A-Z]{16}|xox[abprs]-[\w-]{10,}",
        r"|eyJ[\w-]{10,}\.[\w-]{10,}\.[\w-]{10,}",
        r#"|(?i:(?:api[_-]?key|token|secret|password|passwd)\s*[:=]\s*)['"]?[^\s'"]{8,}"#,
    ))
    .unwrap()
});
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?s)<system-reminder>.*?</system-reminder>|<local-command-caveat>.*?</local-command-caveat>",
        r"|<local-command-stdout>.*?</local-command-stdout>|<command-message>.*?</command-message>",
        r"|</?command-args>",
    ))
    .unwrap()
});
static SKILL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<skill name="([^"]+)".*?</skill>"#).unwrap());
static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<command-name>(.*?)</command-name>\s*").unwrap());

#[derive(Default)]
struct Session {
    agent: &'static str,
    id: String,
    cwd: String,
    started: String,
    ended: String,
    branch: String,
    model: String,
    turns: Vec<String>,
    prompts: usize,
}

impl Session {
    fn new(agent: &'static str, id: &str) -> Self {
        Self {
            agent,
            id: id.to_owned(),
            ..Self::default()
        }
    }

    fn add_user(&mut self, text: &str) {
        let text = clean_user(text);
        if text.is_empty() {
            return;
        }
        self.prompts += 1;
        self.turns
            .push(format!("## User\n\n{}", cap(&text, USER_CAP)));
    }

    /// Takes Claude-style content blocks; pi's `toolCall` and OpenCode's tool
    /// parts are mapped to the same shape by their readers.
    fn add_assistant(&mut self, blocks: &[Value]) {
        let mut texts = Vec::new();
        let mut tools = Vec::new();
        for block in blocks {
            match str_at(block, "type") {
                "text" if !str_at(block, "text").trim().is_empty() => {
                    texts.push(str_at(block, "text").trim().to_owned());
                }
                "tool_use" => tools.push(tool_line(str_at(block, "name"), &block["input"])),
                "toolCall" => tools.push(tool_line(str_at(block, "name"), &block["arguments"])),
                _ => {}
            }
        }
        if texts.is_empty() && tools.is_empty() {
            return;
        }
        let tools = tools.join("\n");
        // Tool-only turns join the previous answer, to keep the files short.
        if texts.is_empty() {
            if let Some(last) = self
                .turns
                .last_mut()
                .filter(|t| t.starts_with("## Assistant"))
            {
                last.push('\n');
                last.push_str(&tools);
                return;
            }
        }
        let body = cap(&texts.join("\n\n"), ASSISTANT_CAP);
        let parts: Vec<&str> = [body.as_str(), tools.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect();
        self.turns
            .push(format!("## Assistant\n\n{}", parts.join("\n\n")));
    }

    fn see(&mut self, cwd: &str, time: &str) {
        if self.cwd.is_empty() {
            self.cwd = cwd.to_owned();
        }
        if self.started.is_empty() {
            self.started = time.to_owned();
        }
        if !time.is_empty() {
            self.ended = time.to_owned();
        }
    }

    fn relative_path(&self) -> PathBuf {
        let date = self.started.get(..10).unwrap_or("undated");
        // Some ids (pi uses UUIDv7) start with a timestamp, so use the end.
        let chars: Vec<char> = self.id.chars().collect();
        let short: String = chars[chars.len().saturating_sub(12)..].iter().collect();
        Path::new(self.agent)
            .join(project_of(&self.cwd))
            .join(format!("{date}-{short}.md"))
    }

    fn render(&self) -> String {
        let title = self
            .turns
            .iter()
            .find_map(|turn| turn.strip_prefix("## User\n\n"))
            .unwrap_or_default();
        let title: String = title
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(100)
            .collect::<String>()
            .replace('"', "'");
        let front = format!(
            "---\ntype: agent-session\nagent: {}\nsession: {}\ntitle: \"{title}\"\nproject: {}\ncwd: {}\nbranch: {}\nmodel: {}\nstarted: {}\nended: {}\nprompts: {}\n---",
            self.agent,
            self.id,
            project_of(&self.cwd),
            self.cwd,
            self.branch,
            self.model,
            self.started,
            self.ended,
            self.prompts
        );
        redact(&format!(
            "{front}\n\n# {title}\n\n{}\n",
            self.turns.join("\n\n")
        ))
    }
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn redact(text: &str) -> String {
    SECRET.replace_all(text, "[REDACTED]").into_owned()
}

fn clean_user(text: &str) -> String {
    let text = SKILL.replace_all(text, "[skill: $1]");
    let text = NOISE.replace_all(&text, "");
    COMMAND.replace_all(&text, "$1 ").trim().to_owned()
}

fn cap(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{} […]", &text[..end]),
        None => text.to_owned(),
    }
}

fn tool_line(name: &str, args: &Value) -> String {
    const KEYS: [&str; 9] = [
        "command",
        "file_path",
        "filePath",
        "path",
        "pattern",
        "url",
        "query",
        "description",
        "prompt",
    ];
    let detail = KEYS
        .iter()
        .filter_map(|key| args.get(*key))
        .find_map(|value| match value {
            Value::String(text) if !text.is_empty() => Some(text.clone()),
            Value::Null | Value::String(_) => None,
            other => Some(other.to_string()),
        })
        .unwrap_or_default();
    let detail = cap(
        &detail.split_whitespace().collect::<Vec<_>>().join(" "),
        TOOL_CAP,
    );
    format!("- `{name}` {detail}").trim_end().to_owned()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| str_at(block, "type") == "text" && block["hidden"] != true)
            .map(|block| str_at(block, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn blocks(content: &Value) -> &[Value] {
    content.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn project_of(cwd: &str) -> String {
    let home = dirs::home_dir();
    let home_name = home.as_deref().and_then(Path::file_name);
    let parts: Vec<String> = Path::new(cwd.trim_start_matches("file://"))
        .components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) if name != "Users" && Some(name) != home_name => {
                Some(name.to_string_lossy().into_owned())
            }
            _ => None,
        })
        .collect();
    match parts.len() {
        0 => "home".into(),
        n => parts[n.saturating_sub(2)..].join("-"),
    }
}

/// Milliseconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_from_millis(millis: i64) -> String {
    let seconds = millis.div_euclid(1000);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil-from-days, from Howard Hinnant's date algorithms.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn json_lines(path: &Path) -> Result<Vec<Value>> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect())
}

fn parse_claude(path: &Path) -> Result<Session> {
    let id = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut session = Session::new("claude", &id);
    for entry in json_lines(path)? {
        let kind = str_at(&entry, "type");
        if !matches!(kind, "user" | "assistant")
            || entry["isMeta"] == true
            || entry["isSidechain"] == true
        {
            continue;
        }
        session.see(str_at(&entry, "cwd"), str_at(&entry, "timestamp"));
        if session.branch.is_empty() {
            session.branch = str_at(&entry, "gitBranch").to_owned();
        }
        let message = &entry["message"];
        if kind == "user" {
            session.add_user(&text_of(&message["content"]));
        } else {
            if session.model.is_empty() {
                session.model = str_at(message, "model").to_owned();
            }
            session.add_assistant(blocks(&message["content"]));
        }
    }
    Ok(session)
}

fn parse_pi(path: &Path) -> Result<Session> {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut session = Session::new("pi", stem.rsplit('_').next().unwrap_or_default());
    for entry in json_lines(path)? {
        let time = str_at(&entry, "timestamp");
        match str_at(&entry, "type") {
            "session" => session.see(str_at(&entry, "cwd"), time),
            "model_change" if session.model.is_empty() => {
                session.model = str_at(&entry, "modelId").to_owned();
            }
            "compaction" if !str_at(&entry, "summary").is_empty() => session.turns.push(format!(
                "## Compaction summary\n\n{}",
                cap(str_at(&entry, "summary"), ASSISTANT_CAP)
            )),
            "message" => {
                session.see("", time);
                let message = &entry["message"];
                match str_at(message, "role") {
                    "user" => session.add_user(&text_of(&message["content"])),
                    "assistant" => session.add_assistant(blocks(&message["content"])),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(session)
}

/// Main sessions sit directly in a project folder; subagent logs sit deeper.
fn session_files(root: &Path) -> Vec<PathBuf> {
    let Ok(projects) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = projects
        .flatten()
        .filter_map(|project| fs::read_dir(project.path()).ok())
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file())
        .collect();
    files.sort();
    files
}

fn file_version(path: &Path) -> String {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(UNIX_EPOCH);
    let nanos = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    nanos.to_string()
}

fn opencode_sessions(db: &Path, sink: &mut Sink) -> Result<()> {
    if !db.is_file() {
        return Ok(());
    }
    let connection =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("cannot open {}", db.display()))?;
    let mut list = connection.prepare(
        "select id, directory, time_created, time_updated from session where parent_id is null order by id",
    )?;
    let rows: Vec<(String, String, i64, i64)> = list
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut parts = connection.prepare(
        "select m.data, p.data, m.time_created from message m join part p on p.message_id = m.id \
         where m.session_id = ?1 order by m.time_created, m.id, p.id",
    )?;
    for (id, directory, created, updated) in rows {
        let key = format!("opencode:{id}");
        if sink.fresh(&key, &updated.to_string()) {
            continue;
        }
        let mut session = Session::new("opencode", &id);
        session.see(&directory, &iso_from_millis(created));
        session.ended = iso_from_millis(updated);
        let rows: Vec<(String, String)> = parts
            .query_map([&id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (message, part) in rows {
            let (message, part): (Value, Value) = (
                serde_json::from_str(&message)?,
                serde_json::from_str(&part)?,
            );
            let block = match str_at(&part, "type") {
                "text" if part["synthetic"] != true => part.clone(),
                "tool" => {
                    serde_json::json!({"type": "tool_use", "name": part["tool"], "input": part["state"]["input"]})
                }
                _ => continue,
            };
            if str_at(&message, "role") == "user" {
                session.add_user(str_at(&block, "text"));
            } else {
                if session.model.is_empty() {
                    session.model = str_at(&message, "modelID").to_owned();
                }
                session.add_assistant(std::slice::from_ref(&block));
            }
        }
        sink.write(&key, &updated.to_string(), &session)?;
    }
    Ok(())
}

fn parse_amp(thread: &Value) -> Session {
    let mut session = Session::new("amp", str_at(thread, "id"));
    let tree = &thread["env"]["initial"]["trees"][0];
    let cwd = match str_at(&tree["repository"], "url") {
        "" => str_at(tree, "uri").to_owned(),
        url => repository_path(url),
    };
    session.see(
        &cwd,
        &thread["created"]
            .as_i64()
            .map(iso_from_millis)
            .unwrap_or_default(),
    );
    session.ended = str_at(thread, "updatedAt").to_owned();
    for message in blocks(&thread["messages"]) {
        if str_at(message, "role") == "user" {
            session.add_user(&text_of(&message["content"]));
        } else {
            if session.model.is_empty() {
                session.model = str_at(&message["usage"], "model").to_owned();
            }
            session.add_assistant(blocks(&message["content"]));
        }
    }
    session
}

/// `host/path` of a Git remote, so the SSH and HTTPS remotes of one
/// repository give the same project: `ssh://git@host:2240/a/b.git` -> `host/a/b`.
fn repository_path(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    format!("{host}/{}", path.trim_end_matches(".git"))
}

/// Amp keeps threads on its server. Threads that Amp imported from a local
/// Claude Code or OpenCode session are skipped: the local reader has them.
fn amp_sessions(system: &dyn System, home: &Path, sink: &mut Sink) -> Result<()> {
    if !system.command_exists("amp") {
        return Ok(());
    }
    let imports: Value = fs::read(home.join(".local/share/amp/thread-imports.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let imported: BTreeSet<&str> = imports["imported"]
        .as_object()
        .into_iter()
        .flat_map(|hosts| hosts.values())
        .filter_map(Value::as_object)
        .flat_map(|agents| agents.values())
        .filter_map(Value::as_object)
        .flat_map(|sessions| sessions.values())
        .map(|record| str_at(record, "threadID"))
        .collect();
    let run = |args: Vec<String>| -> Result<Value> {
        let command = CommandSpec::new("amp", args);
        let result = system.run_unbounded(&command)?;
        anyhow::ensure!(
            result.success,
            "{} failed: {}",
            command.display(),
            crate::install::command_failure_message(&result)
        );
        serde_json::from_str(&result.stdout).context("amp returned invalid JSON")
    };
    let mut threads = Vec::new();
    loop {
        let page = run(
            ["threads", "list", "--json", "--include-archived", "--limit"]
                .map(String::from)
                .into_iter()
                .chain([
                    AMP_PAGE.to_string(),
                    "--offset".into(),
                    threads.len().to_string(),
                ])
                .collect(),
        )?;
        let page = page.as_array().cloned().unwrap_or_default();
        let done = page.len() < AMP_PAGE;
        threads.extend(page);
        if done {
            break;
        }
    }
    for thread in &threads {
        let (id, updated) = (str_at(thread, "id"), str_at(thread, "updated"));
        let key = format!("amp:{id}");
        if id.is_empty() || imported.contains(id) || sink.fresh(&key, updated) {
            continue;
        }
        // One broken thread must not stop the rest; it is retried next run.
        match run(vec!["threads".into(), "export".into(), id.into()]) {
            Ok(export) => sink.write(&key, updated, &parse_amp(&export))?,
            Err(error) => eprintln!("Amp thread {id} skipped: {error:#}"),
        }
    }
    Ok(())
}

struct Sink {
    out: PathBuf,
    state: BTreeMap<String, String>,
    written: usize,
}

impl Sink {
    fn state_path(out: &Path) -> PathBuf {
        out.join(".sync-state.json")
    }

    fn open(out: PathBuf) -> Self {
        let state = fs::read(Self::state_path(&out))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            out,
            state,
            written: 0,
        }
    }

    fn fresh(&self, key: &str, version: &str) -> bool {
        self.state.get(key).is_some_and(|known| known == version)
    }

    fn write(&mut self, key: &str, version: &str, session: &Session) -> Result<()> {
        if session.prompts > 0 {
            let path = self.out.join(session.relative_path());
            fs::create_dir_all(path.parent().context("session path has no parent")?)?;
            fs::write(&path, session.render())
                .with_context(|| format!("cannot write {}", path.display()))?;
            self.written += 1;
        }
        self.state.insert(key.to_owned(), version.to_owned());
        Ok(())
    }

    fn save(&self) -> Result<()> {
        fs::write(
            Self::state_path(&self.out),
            serde_json::to_vec_pretty(&self.state)?,
        )?;
        Ok(())
    }
}

fn file_sessions(
    root: &Path,
    agent: &str,
    parse: fn(&Path) -> Result<Session>,
    sink: &mut Sink,
) -> Result<()> {
    for path in session_files(root) {
        let (key, version) = (format!("{agent}:{}", path.display()), file_version(&path));
        if !sink.fresh(&key, &version) {
            sink.write(&key, &version, &parse(&path)?)?;
        }
    }
    Ok(())
}

fn ensure_ignored(vault: &Path) -> Result<()> {
    let path = vault.join(".gitignore");
    let before = fs::read_to_string(&path).unwrap_or_default();
    let line = format!("{OUT}/");
    if before.lines().any(|existing| existing.trim() == line) {
        return Ok(());
    }
    let separator = if before.is_empty() || before.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    fs::write(
        &path,
        format!("{before}{separator}\n# Agent session copies can contain secrets.\n{line}\n"),
    )
    .with_context(|| format!("cannot update {}", path.display()))
}

/// Refreshes the qmd collection. A Vault set up with `loom wiki --qmd` has its
/// own index; other Vaults use qmd's default index.
fn refresh_qmd(system: &dyn System, vault: &Path, own_index: bool) -> Result<()> {
    if !system.command_exists("qmd") {
        return Ok(());
    }
    let index = qmd_index(vault);
    let qmd = |args: &[&str]| {
        let prefix: &[&str] = if own_index { &["--index", &index] } else { &[] };
        CommandSpec::new("qmd", prefix.iter().chain(args).copied()).in_dir(vault)
    };
    if !system
        .run(&qmd(&["collection", "show", COLLECTION]))?
        .success
    {
        super::product::run_checked(
            system,
            &qmd(&[
                "collection",
                "add",
                OUT,
                "--name",
                COLLECTION,
                "--mask",
                "**/*.md",
            ]),
        )?;
    }
    super::product::run_checked(system, &qmd(&["update"]))?;
    // qmd stops itself before loom's 20-minute command limit; a large first
    // embed then goes on at the next sync.
    super::product::run_checked(
        system,
        &qmd(&["embed", "-c", COLLECTION, "--timeout", "15"]),
    )?;
    Ok(())
}

/// An OS lock on `.sync.lock`, so two sessions that start together do not
/// sync at once. The OS drops the lock when the process ends, also when a
/// session kills its hook process half-way.
struct Lock {
    _file: fs::File,
}

impl Lock {
    fn take(out: &Path) -> Result<Option<Self>> {
        fs::create_dir_all(out)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(out.join(".sync.lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => Err(error.into()),
        }
    }
}

/// Syncs every session source into one Vault. Returns the number of files
/// written, or `None` when another sync holds the lock.
pub fn sync_vault(
    system: &dyn System,
    home: &Path,
    vault: &Path,
    own_index: bool,
) -> Result<Option<usize>> {
    let out = vault.join(OUT);
    let Some(_lock) = Lock::take(&out)? else {
        return Ok(None);
    };
    ensure_ignored(vault)?;
    let mut sink = Sink::open(out);
    let sources = file_sessions(
        &home.join(".claude/projects"),
        "claude",
        parse_claude,
        &mut sink,
    )
    .and_then(|()| file_sessions(&home.join(".pi/agent/sessions"), "pi", parse_pi, &mut sink))
    .and_then(|()| opencode_sessions(&home.join(".local/share/opencode/opencode.db"), &mut sink))
    .and_then(|()| amp_sessions(system, home, &mut sink));
    // Keep the progress of the sources that finished, even if a later one failed.
    sink.save()?;
    sources?;
    refresh_qmd(system, vault, own_index)?;
    Ok(Some(sink.written))
}

/// `loom wiki sessions [VAULT]`: with a Vault, turn session sync on for it;
/// then sync every Vault that has it on.
pub fn run(system: &dyn System, vault: Option<&Path>, quiet: bool) -> Result<bool> {
    let home = system.home_dir().context("home directory is unavailable")?;
    let mut registry = WikiRegistry::load(&home)?;
    if let Some(vault) = vault {
        let vault = canonical_vault(vault)?;
        let record = registry
            .vaults
            .iter_mut()
            .find(|record| record.path == vault)
            .with_context(|| {
                format!(
                    "{} is not a registered Vault; run loom wiki adopt first",
                    vault.display()
                )
            })?;
        record.sessions = true;
        registry.save(&home)?;
    }
    let vaults: Vec<_> = registry
        .vaults
        .iter()
        .filter(|record| record.sessions)
        .collect();
    if vaults.is_empty() && !quiet {
        println!("No Vault syncs sessions. Run: loom wiki sessions <vault>");
    }
    let started = SystemTime::now();
    for record in vaults {
        let outcome = sync_vault(system, &home, &record.path, record.qmd)?;
        if quiet {
            continue;
        }
        match outcome {
            Some(written) => println!(
                "{}: {written} sessions updated in {:.1}s",
                record.path.display(),
                started.elapsed().unwrap_or_default().as_secs_f32()
            ),
            None => println!("{}: another session sync is running", record.path.display()),
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redaction_removes_token_shapes_and_keeps_text() {
        assert_eq!(
            redact("key sk-abcdefghijklmnopqrstuvwxyz1234"),
            "key [REDACTED]"
        );
        assert_eq!(redact("API_KEY=hunter2hunter2"), "[REDACTED]");
        assert_eq!(
            redact("npm_abcdefghijklmnopqrstuvwxyz0123456789"),
            "[REDACTED]"
        );
        assert_eq!(redact("pypi-AgEIcHlwaS5vcmcCJGFiY2RlZmdo"), "[REDACTED]");
        assert_eq!(redact("plain text stays"), "plain text stays");
    }

    #[test]
    fn user_text_loses_harness_noise() {
        assert_eq!(clean_user("<system-reminder>x</system-reminder>hi"), "hi");
        assert_eq!(
            clean_user(r#"<skill name="grill">long</skill> go"#),
            "[skill: grill] go"
        );
        assert_eq!(
            clean_user("<command-name>/add-dir</command-name>\n<command-args>/x</command-args>"),
            "/add-dir /x"
        );
    }

    #[test]
    fn helpers_match_the_published_file_layout() {
        assert_eq!(cap("abcdef", 3), "abc […]");
        assert_eq!(
            tool_line("Bash", &json!({"command": "ls  -la"})),
            "- `Bash` ls -la"
        );
        assert_eq!(iso_from_millis(1_769_109_605_876), "2026-01-22T19:20:05Z");
        assert_eq!(
            project_of("/srv/projects/personal/inner-poc"),
            "personal-inner-poc"
        );
        assert_eq!(
            repository_path("ssh://git@gitlab.example.com:2240/apps/turbine/turbine.git"),
            "gitlab.example.com/apps/turbine/turbine"
        );
        assert_eq!(
            repository_path("https://github.com/yassimba/loom"),
            "github.com/yassimba/loom"
        );
        let mut session = Session::new("pi", "019dfe76-e83f-737e-b25c-9bec4d1e0cf9");
        session.started = "2026-05-06T18:04:58.815Z".into();
        session.cwd = "/srv/projects/vision".into();
        assert_eq!(
            session.relative_path(),
            Path::new("pi/projects-vision/2026-05-06-9bec4d1e0cf9.md")
        );
    }

    #[test]
    fn tool_only_turns_join_the_previous_answer() {
        let mut session = Session::new("claude", "id");
        session.add_user("question");
        session.add_assistant(&[
            json!({"type": "text", "text": "Looking."}),
            json!({"type": "tool_use", "name": "Read", "input": {"file_path": "a.rs"}}),
        ]);
        session.add_assistant(&[
            json!({"type": "toolCall", "name": "bash", "arguments": {"command": "ls"}}),
        ]);
        session.add_assistant(&[json!({"type": "thinking", "thinking": "secret plan"})]);
        assert_eq!(session.prompts, 1);
        assert_eq!(
            session.turns[1],
            "## Assistant\n\nLooking.\n\n- `Read` a.rs\n- `bash` ls"
        );
        assert!(!session.render().contains("secret plan"));
    }

    #[test]
    fn amp_threads_use_repository_and_skip_hidden_text() {
        let thread = json!({
            "id": "T-01a172cf-2873-7050-861a-015d6eb18149",
            "created": 1_792_927_541_527_i64,
            "updatedAt": "2026-10-25T11:47:51.138Z",
            "env": {"initial": {"trees": [{"uri": "file:///home/user/workspace/repo", "repository": {"url": "https://github.com/yassimba/second-brain"}}]}},
            "messages": [
                {"role": "user", "content": [{"type": "text", "text": "Tell me about OpenHands"}, {"type": "text", "text": "mode note", "hidden": true}]},
                {"role": "assistant", "usage": {"model": "claude-opus-5-5"}, "content": [{"type": "text", "text": "It is an agent."}]}
            ]
        });
        let session = parse_amp(&thread);
        assert_eq!(
            session.relative_path(),
            Path::new("amp/yassimba-second-brain/2026-10-25-015d6eb18149.md")
        );
        let text = session.render();
        assert!(text.contains("model: claude-opus-5-5") && text.contains("It is an agent."));
        assert!(!text.contains("mode note"));
    }

    #[test]
    fn a_second_sync_waits_until_the_first_one_ends() {
        let out = std::env::temp_dir().join(format!("loom-sessions-lock-{}", std::process::id()));
        let first = Lock::take(&out).unwrap();
        assert!(first.is_some());
        assert!(Lock::take(&out).unwrap().is_none());
        drop(first);
        assert!(Lock::take(&out).unwrap().is_some());
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn files_sync_once_and_skip_subagents() {
        let root = std::env::temp_dir().join(format!("loom-sessions-{}", std::process::id()));
        let project = root.join("home/.claude/projects/-srv-app");
        fs::create_dir_all(project.join("abc/subagents")).unwrap();
        let line = json!({"type": "user", "cwd": "/srv/app", "timestamp": "2026-10-01T00:00:00Z", "message": {"content": "hello"}});
        fs::write(project.join("main-session.jsonl"), format!("{line}\n")).unwrap();
        fs::write(
            project.join("abc/subagents/agent-1.jsonl"),
            format!("{line}\n"),
        )
        .unwrap();
        let vault = root.join("vault");
        fs::create_dir_all(&vault).unwrap();
        let mut sink = Sink::open(vault.join(OUT));
        file_sessions(
            &root.join("home/.claude/projects"),
            "claude",
            parse_claude,
            &mut sink,
        )
        .unwrap();
        assert_eq!(sink.written, 1);
        file_sessions(
            &root.join("home/.claude/projects"),
            "claude",
            parse_claude,
            &mut sink,
        )
        .unwrap();
        assert_eq!(
            sink.written, 1,
            "an unchanged session is not converted again"
        );
        assert!(vault
            .join(OUT)
            .join("claude/srv-app/2026-10-01-main-session.md")
            .is_file());
        ensure_ignored(&vault).unwrap();
        ensure_ignored(&vault).unwrap();
        assert_eq!(
            fs::read_to_string(vault.join(".gitignore"))
                .unwrap()
                .matches(".raw/agent-sessions/")
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }
}
