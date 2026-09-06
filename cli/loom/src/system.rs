use crate::CommandSpec;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MANAGER_COMMAND_TIMEOUT: Duration = Duration::from_secs(20 * 60);
pub const PROBE_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait System {
    fn command_exists(&self, name: &str) -> bool;
    fn refresh_path(&self);
    fn github_token(&self) -> Option<String> {
        None
    }
    fn run(&self, command: &CommandSpec) -> Result<CommandResult>;
    fn spawn_detached(&self, command: &CommandSpec) -> Result<()> {
        let result = self.run(command)?;
        anyhow::ensure!(
            result.success,
            "{}",
            crate::install::command_failure_message(&result)
        );
        Ok(())
    }
    fn run_probe(&self, command: &CommandSpec) -> Result<CommandResult> {
        self.run_controlled(command, PROBE_COMMAND_TIMEOUT, &AtomicBool::new(false))
    }
    /// Execute with a deadline and optional cooperative cancellation. Test
    /// systems can keep implementing only `run`; real child processes use
    /// the controlled implementation below.
    fn run_controlled(
        &self,
        command: &CommandSpec,
        _timeout: Duration,
        _cancelled: &AtomicBool,
    ) -> Result<CommandResult> {
        self.run(command)
    }
    /// Bounded streaming output; the callback must not block or disclose arbitrary tool output.
    fn run_streamed(
        &self,
        command: &CommandSpec,
        timeout: Duration,
        cancelled: &AtomicBool,
        input: Option<&[u8]>,
        output: &(dyn Fn(&[u8]) + Sync),
    ) -> Result<CommandResult> {
        anyhow::ensure!(
            input.is_none(),
            "this system does not support private stdin"
        );
        let result = self.run_controlled(command, timeout, cancelled)?;
        output(result.stdout.as_bytes());
        output(result.stderr.as_bytes());
        Ok(result)
    }
    /// The home directory skill trees are detected under. Injectable so
    /// tests can point the installer at a temp home.
    fn home_dir(&self) -> Option<PathBuf> {
        env::home_dir()
    }
    /// The directory `loom` was launched from. Skill project scope resolves
    /// its worktree root from here; injectable for installer tests.
    fn current_dir(&self) -> Option<PathBuf> {
        env::current_dir().ok()
    }
}

/// Holds the PATH this process resolves tools against. It starts as the
/// inherited PATH and grows as installers register new tool directories.
/// Kept here rather than in the process environment: the wizard refreshes
/// it from the install worker thread while the UI thread keeps running, and
/// `env::set_var` racing environment reads is undefined behavior on Unix.
pub struct RealSystem {
    path: Mutex<OsString>,
}

impl Default for RealSystem {
    fn default() -> Self {
        Self {
            path: Mutex::new(env::var_os("PATH").unwrap_or_default()),
        }
    }
}

impl RealSystem {
    fn path_value(&self) -> OsString {
        self.path.lock().expect("PATH lock poisoned").clone()
    }
}

/// Locate `name` on `path` the way the shell would, honoring PATHEXT on
/// Windows so `.cmd`/`.bat` shims (npm, pi) are found.
fn resolve_program(path: &OsStr, name: &str) -> Option<PathBuf> {
    let extensions = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    } else {
        vec![String::new()]
    };
    env::split_paths(path).find_map(|directory| {
        extensions.iter().find_map(|extension| {
            let candidate = Path::new(&directory).join(format!("{name}{extension}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

/// Build a std Command for the spec. On Windows the program must be resolved
/// to its full path first: CreateProcess only appends `.exe`, so a bare
/// `npm`/`pi` never finds the `.cmd` shims those tools install as.
/// The child also gets `path` as its PATH; on Unix std looks the bare
/// program name up against that override.
fn command_for(path: &OsStr, spec: &CommandSpec) -> Command {
    let program = if cfg!(windows) {
        resolve_program(path, &spec.program).unwrap_or_else(|| PathBuf::from(&spec.program))
    } else {
        PathBuf::from(&spec.program)
    };
    let mut command = Command::new(program);
    command.args(&spec.args);
    command.env("PATH", path);
    command.envs(spec.private_env.iter().map(|(name, value)| (name, value)));
    if let Some(directory) = &spec.cwd {
        command.current_dir(directory);
    }
    #[cfg(unix)]
    // Put every managed command in its own process group. Cancelling the
    // wrapper shell must also terminate package-manager and pipeline children.
    command.process_group(0);
    command
}

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        // A negative pid addresses the process group created in command_for.
        let _ = kill(-(child.id() as i32), 9);
    }
    #[cfg(windows)]
    {
        // taskkill /T is the native process-tree primitive available on every
        // supported Windows runner and host.
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// PATH entries persisted to the Windows registry. Installers (Herdr, npm)
/// append their bin directory there, but this process only sees the PATH it
/// was started with, so re-read the registry after installers run.
fn registry_path_entries(system: &dyn System) -> Vec<PathBuf> {
    let command = CommandSpec::new(
        "powershell",
        [
            "-NoProfile",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path', 'User'); \
             [Environment]::GetEnvironmentVariable('Path', 'Machine')",
        ],
    );
    let Ok(output) = system.run_probe(&command) else {
        return Vec::new();
    };
    if !output.success {
        return Vec::new();
    }
    output
        .stdout
        .lines()
        .flat_map(|line| line.split(';'))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(PathBuf::from)
        .collect()
}

impl System for RealSystem {
    fn command_exists(&self, name: &str) -> bool {
        resolve_program(&self.path_value(), name).is_some()
    }

    fn github_token(&self) -> Option<String> {
        if !self.command_exists("gh") {
            return None;
        }
        let result = self
            .run_probe(&CommandSpec::new(
                "gh",
                ["auth", "token", "--hostname", "github.com"],
            ))
            .ok()?;
        result
            .success
            .then(|| result.stdout.trim().to_owned())
            .filter(|token| !token.is_empty())
    }

    fn refresh_path(&self) {
        let mut paths = env::split_paths(&self.path_value()).collect::<Vec<_>>();
        if let Some(home) = env::home_dir() {
            paths.push(home.join(".local").join("bin"));
            paths.push(home.join(".cargo").join("bin"));
            // mise-managed tools resolve through its shims until the user's
            // shell activation takes over.
            paths.push(home.join(".local").join("share").join("mise").join("shims"));
            if cfg!(windows) {
                paths.push(home.join("AppData").join("Roaming").join("npm"));
                paths.push(
                    home.join("AppData")
                        .join("Local")
                        .join("mise")
                        .join("shims"),
                );
            }
        }
        if cfg!(windows) {
            paths.extend(registry_path_entries(self));
        }
        // Probe npm against the merged candidates, so an npm that only just
        // appeared in the registry PATH still reports its global prefix.
        let merged = env::join_paths(paths.iter().cloned()).unwrap_or_else(|_| self.path_value());
        let probe_system = Self {
            path: Mutex::new(merged),
        };
        if let Ok(output) = probe_system.run_probe(&CommandSpec::new("npm", ["prefix", "--global"]))
        {
            if output.success {
                let prefix = output.stdout.trim().to_owned();
                if !prefix.is_empty() {
                    let prefix = PathBuf::from(prefix);
                    paths.push(if cfg!(windows) {
                        prefix
                    } else {
                        prefix.join("bin")
                    });
                }
            }
        }
        paths.retain(|path| path.is_dir());
        let mut seen = HashSet::new();
        paths.retain(|path| seen.insert(path.clone()));
        if let Ok(path) = env::join_paths(paths) {
            *self.path.lock().expect("PATH lock poisoned") = path;
        }
    }

    fn run(&self, command: &CommandSpec) -> Result<CommandResult> {
        self.run_controlled(command, MANAGER_COMMAND_TIMEOUT, &AtomicBool::new(false))
    }

    fn spawn_detached(&self, command: &CommandSpec) -> Result<()> {
        command_for(&self.path_value(), command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("could not start {}", command.program))?;
        Ok(())
    }

    fn run_controlled(
        &self,
        command: &CommandSpec,
        timeout: Duration,
        cancelled: &AtomicBool,
    ) -> Result<CommandResult> {
        self.run_streamed(command, timeout, cancelled, None, &|_| {})
    }

    fn run_streamed(
        &self,
        command: &CommandSpec,
        timeout: Duration,
        cancelled: &AtomicBool,
        input: Option<&[u8]>,
        output: &(dyn Fn(&[u8]) + Sync),
    ) -> Result<CommandResult> {
        anyhow::ensure!(
            !cancelled.load(Ordering::Relaxed),
            "cancelled while running command"
        );
        let mut child = command_for(&self.path_value(), command)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not start {}", command.program))?;
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let stdin = child.stdin.take();
        std::thread::scope(|scope| {
            let stdout_reader = scope.spawn(|| capture_stream(stdout, output));
            let stderr_reader = scope.spawn(|| capture_stream(stderr, output));
            let writer = scope.spawn(move || {
                if let (Some(mut stdin), Some(input)) = (stdin, input) {
                    stdin.write_all(input)?;
                }
                Ok::<_, std::io::Error>(())
            });
            let started = Instant::now();
            let status = (|| loop {
                anyhow::ensure!(
                    !cancelled.load(Ordering::Relaxed),
                    "cancelled while running command"
                );
                anyhow::ensure!(
                    started.elapsed() < timeout,
                    "timed out after {}s while running command",
                    timeout.as_secs()
                );
                if let Some(status) = child.try_wait()? {
                    break Ok::<_, anyhow::Error>(status);
                }
                std::thread::sleep(Duration::from_millis(50));
            })();
            // Also close pipes held by descendants after the wrapper exits.
            terminate_process_tree(&mut child);
            let stdout = stdout_reader.join().unwrap_or_default();
            let stderr = stderr_reader.join().unwrap_or_default();
            let written = writer.join();
            let status = status?;
            anyhow::ensure!(
                matches!(written, Ok(Ok(()))),
                "could not send private command input"
            );
            Ok(CommandResult {
                success: status.success(),
                stdout: String::from_utf8_lossy(&stdout).into_owned(),
                stderr: String::from_utf8_lossy(&stderr).into_owned(),
            })
        })
    }
}

fn capture_stream(mut reader: impl Read, output: &(dyn Fn(&[u8]) + Sync)) -> Vec<u8> {
    const LIMIT: usize = 4 * 1024 * 1024;
    let mut captured = std::collections::VecDeque::new();
    let mut chunk = [0; 4096];
    while let Ok(count) = reader.read(&mut chunk) {
        if count == 0 {
            break;
        }
        output(&chunk[..count]);
        if captured.len() + count > LIMIT {
            captured.drain(..count.min(captured.len()));
        }
        captured.extend(&chunk[..count]);
    }
    captured.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn streamed_commands_report_partial_output_and_cancel_with_private_stdin() {
        let system = RealSystem::default();
        let cancelled = AtomicBool::new(false);
        let received = Mutex::new(Vec::new());
        let command = CommandSpec::new(
            "sh",
            [
                "-c",
                "read value; printf 'partial\\r'; printf 'stderr' >&2; sleep 5",
            ],
        );
        assert!(!command.display().contains("SECRET-FIXTURE"));
        let started = Instant::now();
        let error = system
            .run_streamed(
                &command,
                Duration::from_secs(3),
                &cancelled,
                Some(b"SECRET-FIXTURE\n"),
                &|chunk| {
                    received.lock().unwrap().extend_from_slice(chunk);
                    cancelled.store(true, Ordering::Relaxed);
                },
            )
            .unwrap_err()
            .to_string();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!received.lock().unwrap().is_empty());
        assert!(error.contains("cancelled"));
        assert!(!error.contains("SECRET-FIXTURE"));
    }

    #[test]
    fn capture_drains_large_streams_with_bounded_memory() {
        let bytes = vec![b'x'; 5 * 1024 * 1024];
        let seen = std::sync::atomic::AtomicUsize::new(0);
        let result = capture_stream(std::io::Cursor::new(&bytes), &|chunk| {
            seen.fetch_add(chunk.len(), Ordering::Relaxed);
        });
        assert_eq!(seen.load(Ordering::Relaxed), bytes.len());
        assert!(result.len() <= 4 * 1024 * 1024);
    }

    #[cfg(unix)]
    #[test]
    fn real_commands_honor_deadlines_and_cancellation() {
        let system = RealSystem::default();
        let cancelled = AtomicBool::new(false);
        let timeout = system
            .run_controlled(
                &CommandSpec::new("sh", ["-c", "sleep 2"]),
                Duration::from_millis(20),
                &cancelled,
            )
            .unwrap_err()
            .to_string();
        assert!(timeout.contains("timed out after 0s"));

        cancelled.store(true, Ordering::Relaxed);
        let cancellation = system
            .run_controlled(
                &CommandSpec::new("sh", ["-c", "sleep 2"]),
                Duration::from_secs(2),
                &cancelled,
            )
            .unwrap_err()
            .to_string();
        assert!(cancellation.contains("cancelled while running"));
    }

    #[cfg(unix)]
    #[test]
    fn controlled_commands_terminate_descendants() {
        let system = RealSystem::default();
        let marker = env::temp_dir().join(format!(
            "loom-cancelled-descendant-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let script = format!("(sleep 0.2; touch {}) & sleep 2", marker.display());

        let error = system
            .run_controlled(
                &CommandSpec::new("sh", ["-c", &script]),
                Duration::from_millis(20),
                &AtomicBool::new(false),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("timed out"));
        std::thread::sleep(Duration::from_millis(350));
        assert!(!marker.exists(), "a descendant survived cancellation");
    }
}
