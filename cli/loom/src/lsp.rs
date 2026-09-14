//! A minimal JSON-RPC client for the language servers already on PATH, used
//! only to answer `loom contracts related`. One request at a time, one process
//! per query, no persistent state: the client is a question, not a session pool.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A server that cannot answer within this budget is treated as unavailable.
const BUDGET: Duration = Duration::from_secs(60);

/// One place the queried declaration is used.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Location {
    pub path: PathBuf,
    pub line: usize,
    pub name: String,
}

/// Framed JSON-RPC over a reader/writer pair, so tests can drive a transcript
/// instead of a child process.
struct Session<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    next_id: i64,
}

impl<R: BufRead, W: Write> Session<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_id: 1,
        }
    }

    fn send(&mut self, message: &Value) -> Result<()> {
        let body = serde_json::to_vec(message).expect("request serializes");
        write!(self.writer, "Content-Length: {}\r\n\r\n", body.len())?;
        self.writer.write_all(&body)?;
        self.writer.flush()?;
        Ok(())
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    /// Send one request and read until its answer arrives, ignoring the
    /// notifications and server-to-client requests that pass in the meantime.
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let message = self.read_message()?;
            if message.get("id").and_then(Value::as_i64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let text = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                bail!("{method} failed: {text}");
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn read_message(&mut self) -> Result<Value> {
        let mut length = None;
        loop {
            let mut header = String::new();
            if self.reader.read_line(&mut header)? == 0 {
                bail!("language server closed the connection");
            }
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some(value) = header.strip_prefix("Content-Length:") {
                length = Some(value.trim().parse().context("invalid Content-Length")?);
            }
        }
        let length = length.context("message without Content-Length")?;
        let mut body = vec![0; length];
        std::io::Read::read_exact(&mut self.reader, &mut body)?;
        serde_json::from_slice(&body).context("language server sent invalid JSON")
    }
}

fn path_from_uri(uri: &str, root: &Path) -> PathBuf {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    // Percent-encoding appears in paths with spaces; only that escape matters here.
    let path = PathBuf::from(path.replace("%20", " "));
    path.strip_prefix(root).unwrap_or(&path).to_path_buf()
}

fn location(uri: Option<&str>, range: Option<&Value>, name: &str, root: &Path) -> Option<Location> {
    let line = range?.get("start")?.get("line")?.as_u64()?;
    Some(Location {
        path: path_from_uri(uri?, root),
        // LSP counts lines from zero; every other Loom surface counts from one.
        line: line as usize + 1,
        name: name.to_owned(),
    })
}

/// `textDocument/references` answers a flat list of locations.
fn parse_references(result: &Value, root: &Path) -> Vec<Location> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            location(
                entry.get("uri").and_then(Value::as_str),
                entry.get("range"),
                "",
                root,
            )
        })
        .collect()
}

/// `callHierarchy/incomingCalls` answers callers, each naming its declaration.
fn parse_incoming_calls(result: &Value, root: &Path) -> Vec<Location> {
    result
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let from = entry.get("from")?;
            location(
                from.get("uri").and_then(Value::as_str),
                from.get("selectionRange").or_else(|| from.get("range")),
                from.get("name").and_then(Value::as_str).unwrap_or(""),
                root,
            )
        })
        .collect()
}

/// Kill the server when the budget runs out; the pending read then sees EOF.
/// Whoever wins the `swap` owns the outcome: the watchdog kills only if the
/// conversation has not finished, and the caller learns it timed out only if
/// the watchdog got there first.
fn watchdog(child: &Arc<Mutex<Child>>, settled: &Arc<AtomicBool>) {
    let child = Arc::clone(child);
    let settled = Arc::clone(settled);
    std::thread::spawn(move || {
        std::thread::sleep(BUDGET);
        if !settled.swap(true, Ordering::SeqCst) {
            let _ = child.lock().expect("server handle poisoned").kill();
        }
    });
}

/// One question for a server: which declaration, in which opened file.
pub struct Query<'a> {
    pub root: &'a Path,
    /// Repository-relative, as every Loom surface reports paths.
    pub file: &'a Path,
    pub text: &'a str,
    pub language: &'a str,
    pub line: usize,
    pub character: usize,
    /// Callers of the declaration, rather than every reference to it.
    pub callers: bool,
}

/// Ask one language server where a declaration is used. Returns the locations
/// and whether a `callers` request had to fall back to plain references.
pub fn query(program: &str, arguments: &[&str], ask: &Query) -> Result<(Vec<Location>, bool)> {
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(ask.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start {program}"))?;
    let writer = child.stdin.take().expect("stdin is piped");
    let reader = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut errors = child.stderr.take().expect("stderr is piped");
    let child = Arc::new(Mutex::new(child));
    let settled = Arc::new(AtomicBool::new(false));
    watchdog(&child, &settled);

    let outcome = converse(Session::new(reader, writer), ask);
    let timed_out = settled.swap(true, Ordering::SeqCst);
    let mut handle = child.lock().expect("server handle poisoned");
    let _ = handle.kill();
    let _ = handle.wait();
    outcome.with_context(|| {
        if timed_out {
            return format!("{program} did not answer within {}s", BUDGET.as_secs());
        }
        // A server that dies early explains itself on stderr; the first line is the reason.
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut errors, &mut text);
        match text.lines().find(|line| !line.trim().is_empty()) {
            Some(reason) => format!("{program} stopped: {reason}"),
            None => format!("{program} stopped without answering"),
        }
    })
}

/// The whole conversation: initialize, open the file, ask, shut down.
fn converse<R: BufRead, W: Write>(
    mut session: Session<R, W>,
    ask: &Query,
) -> Result<(Vec<Location>, bool)> {
    let root = ask.root;
    let uri = format!("file://{}", root.join(ask.file).display());
    // `root` is used a dozen times below; the alias keeps those lines short.
    let position = json!({"line": ask.line - 1, "character": ask.character});
    session.request(
        "initialize",
        json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", root.display()),
            "capabilities": {"textDocument": {"references": {}, "callHierarchy": {}}},
            "workspaceFolders": [{
                "uri": format!("file://{}", root.display()),
                "name": root.file_name().unwrap_or_default().to_string_lossy(),
            }],
        }),
    )?;
    session.notify("initialized", json!({}))?;
    session.notify(
        "textDocument/didOpen",
        json!({"textDocument": {
            "uri": uri, "languageId": ask.language, "version": 1, "text": ask.text,
        }}),
    )?;

    let document = json!({"textDocument": {"uri": uri}, "position": position});
    let mut fell_back = false;
    let mut locations = Vec::new();
    if ask.callers {
        // A server without call hierarchy answers references instead, and says so.
        match session.request("textDocument/prepareCallHierarchy", document.clone()) {
            Ok(item) if item.get(0).is_some() => {
                let calls = session.request(
                    "callHierarchy/incomingCalls",
                    json!({"item": item[0].clone()}),
                )?;
                locations = parse_incoming_calls(&calls, root);
            }
            _ => fell_back = true,
        }
    }
    if !ask.callers || fell_back {
        let mut params = document;
        params["context"] = json!({"includeDeclaration": false});
        locations = parse_references(&session.request("textDocument/references", params)?, root);
    }

    session.request("shutdown", Value::Null)?;
    session.notify("exit", Value::Null)?;
    locations.sort();
    locations.dedup();
    Ok((locations, fell_back))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transcript stands in for a server: framed replies in, framed requests out.
    fn transcript(replies: &[Value]) -> Session<std::io::Cursor<Vec<u8>>, Vec<u8>> {
        let mut bytes = Vec::new();
        for reply in replies {
            let body = serde_json::to_vec(reply).expect("reply serializes");
            bytes.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
            bytes.extend_from_slice(&body);
        }
        Session::new(std::io::Cursor::new(bytes), Vec::new())
    }

    #[test]
    fn a_request_skips_notifications_and_frames_its_own_message() {
        let mut session = transcript(&[
            json!({"jsonrpc": "2.0", "method": "$/progress", "params": {"kind": "begin"}}),
            json!({"jsonrpc": "2.0", "id": 1, "result": [{
                "uri": "file:///repo/src/main.rs",
                "range": {"start": {"line": 41, "character": 4}},
            }]}),
        ]);
        let result = session
            .request("textDocument/references", json!({}))
            .expect("the answer arrives after the notification");
        assert_eq!(
            parse_references(&result, Path::new("/repo")),
            vec![Location {
                path: PathBuf::from("src/main.rs"),
                line: 42,
                name: String::new(),
            }]
        );
        let sent = String::from_utf8(session.writer).expect("requests are UTF-8");
        assert!(sent.starts_with("Content-Length: "), "framed: {sent}");
        assert!(sent.contains("\"id\":1"), "identified: {sent}");
    }

    #[test]
    fn an_error_response_is_an_error_not_an_empty_answer() {
        let mut session = transcript(&[
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "unsupported"}}),
        ]);
        let error = session
            .request("textDocument/prepareCallHierarchy", json!({}))
            .expect_err("an error response fails the request");
        assert!(format!("{error:#}").contains("unsupported"), "{error:#}");
    }

    #[test]
    fn callers_fall_back_to_references_when_the_server_has_no_call_hierarchy() {
        let session = transcript(&[
            json!({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}}),
            json!({"jsonrpc": "2.0", "id": 2, "error": {"code": -32601, "message": "unsupported"}}),
            json!({"jsonrpc": "2.0", "id": 3, "result": [{
                "uri": "file:///repo/pkg/pay.py",
                "range": {"start": {"line": 8, "character": 11}},
            }]}),
            json!({"jsonrpc": "2.0", "id": 4, "result": null}),
        ]);
        let (locations, fell_back) = converse(
            session,
            &Query {
                root: Path::new("/repo"),
                file: Path::new("pkg/pay.py"),
                text: "def charge(amount):\n",
                language: "python",
                line: 1,
                character: 4,
                callers: true,
            },
        )
        .expect("the fallback answers");
        assert!(fell_back, "the fallback is reported, not hidden");
        assert_eq!(locations.len(), 1, "{locations:?}");
        assert_eq!(locations[0].line, 9);
    }

    #[test]
    fn incoming_calls_carry_the_calling_declaration_name() {
        let calls = json!([{"from": {
            "name": "checkout",
            "uri": "file:///repo/src/pay.rs",
            "selectionRange": {"start": {"line": 9, "character": 3}},
        }}]);
        assert_eq!(
            parse_incoming_calls(&calls, Path::new("/repo")),
            vec![Location {
                path: PathBuf::from("src/pay.rs"),
                line: 10,
                name: "checkout".into(),
            }]
        );
    }
}
