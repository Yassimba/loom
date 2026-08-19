//! loom-teams: your Teams/Outlook calendar as data.
//!
//! Single binary over Microsoft Graph. `setup` opens Chrome so the user can
//! sign in themselves and caches a Graph bearer; `status` shows the session; `export`
//! dumps a window of calendar data as JSON (`loom-teams/export/1`) — the
//! primitive that reports, LLM meeting classification, and other use cases
//! build on; `find` is the first such use case: rank meeting slots, with AI
//! placeholder-vs-real verdicts fed back in via `--classifications`.

mod browser;
mod graph;
mod model;
mod rank;
mod token;
mod tz;

use anyhow::{bail, ensure, Context, Result};
use clap::{Args, Parser, Subcommand};
use model::{
    AvailabilityView, Classifications, Event, Export, Hours, Person, Query, RawExport,
    ScheduleInformation, Window,
};
use rank::RankInput;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "loom-teams",
    version,
    about = "Your Teams/Outlook calendar as data, via Microsoft Graph"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Open Chrome so you can sign in to Teams; caches a Graph bearer token.
    Setup,
    /// Show the cached session: who, scopes, expiry.
    Status,
    /// Export a window of calendar data as JSON (events + free/busy views).
    Export(ExportArgs),
    /// Find meeting slots for you plus the given people.
    Find(FindArgs),
}

/// Time window shared by export and find. Either a `--when` preset or an
/// explicit `--from`/`--to` date range.
#[derive(Args)]
struct WindowArgs {
    /// "next-week" (Mon–Fri) or a number of days starting tomorrow.
    #[arg(long, default_value = "next-week", conflicts_with_all = ["from", "to"])]
    when: String,
    /// Start date (YYYY-MM-DD, local midnight).
    #[arg(long)]
    from: Option<String>,
    /// End date, inclusive (YYYY-MM-DD). Defaults to one week after --from.
    #[arg(long, requires = "from")]
    to: Option<String>,
    /// IANA time zone; defaults to the machine zone.
    #[arg(long)]
    tz: Option<String>,
}

#[derive(Args)]
struct ExportArgs {
    /// Names (resolved via Graph people search) or emails. Defaults to just you.
    who: Vec<String>,
    #[command(flatten)]
    window: WindowArgs,
    /// Free/busy grid resolution in minutes.
    #[arg(long, default_value_t = 30)]
    interval: u32,
    /// Write the JSON here instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Run the token-refresh browser with a visible window.
    #[arg(long)]
    headed: bool,
}

#[derive(Args)]
struct FindArgs {
    /// Names (resolved via Graph people search) or email addresses.
    #[arg(required = true)]
    who: Vec<String>,
    #[command(flatten)]
    window: WindowArgs,
    /// Meeting length in minutes.
    #[arg(long, default_value_t = 30)]
    duration: u32,
    /// Free/busy grid resolution in minutes.
    #[arg(long, default_value_t = 30)]
    interval: u32,
    /// Preferred hours, e.g. 9-17.
    #[arg(long, default_value = "9-17")]
    hours: String,
    /// Fallback hours when nothing fits the preferred window, e.g. 7-20.
    #[arg(long, default_value = "7-20")]
    expanded: String,
    /// How many picks to return.
    #[arg(long, default_value_t = 10)]
    top: usize,
    /// Print the full JSON export to stdout instead of the human summary.
    #[arg(long)]
    json: bool,
    /// Also write the full JSON export to this file.
    #[arg(long)]
    out: Option<PathBuf>,
    /// JSON file mapping event ids to "generic" | "real" (AI verdicts).
    #[arg(long)]
    classifications: Option<PathBuf>,
    /// Run the token-refresh browser with a visible window.
    #[arg(long)]
    headed: bool,
}

/// The Chrome profile lives in the platform data dir; $LOOM_TEAMS_HOME
/// overrides it (undocumented escape hatch, mainly for development).
fn profile_dir() -> PathBuf {
    std::env::var("LOOM_TEAMS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("loom-teams")
        })
        .join(".profile")
}

// ---- Session ----

/// One `/me` call doubles as token probe and identity lookup.
async fn ensure_token(force_login: bool, headed: bool) -> Result<(graph::Graph, graph::Me)> {
    if !force_login {
        if let Some(cached) = token::load() {
            let client = graph::Graph::new(cached.token);
            if let Ok(me) = client.me().await {
                return Ok((client, me));
            }
        }
    }
    let profile = profile_dir();
    if !profile.exists() && !force_login {
        bail!("No cached Teams session. Run: loom-teams setup (and sign in once).");
    }
    let token = browser::acquire(&profile, headed || force_login).await?;
    let client = graph::Graph::new(token.clone());
    let me = client.me().await.context("Graph /me with fresh token")?;
    token::store(&token)?;
    Ok((client, me))
}

async fn session(headed: bool) -> Result<(graph::Graph, graph::Me)> {
    ensure_token(false, headed).await
}

// ---- Time window ----

struct SearchWindow {
    start_ms: i64,
    start_utc: String,
    end_utc: String,
    /// Human label for output: "next-week" or "2026-08-24..2026-08-28".
    label: String,
}

fn search_window(zone: &jiff::tz::TimeZone, args: &WindowArgs) -> Result<SearchWindow> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let (start_ymd, end_ymd, label) = match &args.from {
        Some(from) => {
            let to = match &args.to {
                Some(to) => to.clone(),
                None => tz::add_days_ymd(from, 6)?,
            };
            ensure!(
                from.as_str() <= to.as_str(),
                "--from must not be after --to"
            );
            // --to is inclusive; the window ends at the next local midnight.
            (
                from.clone(),
                tz::add_days_ymd(&to, 1)?,
                format!("{from}..{to}"),
            )
        }
        None if args.when == "next-week" => {
            let start = tz::next_week_monday(zone, now_ms);
            let end = tz::add_days_ymd(&start, 5)?;
            (start, end, args.when.clone())
        }
        None => {
            let days: i64 = args
                .when
                .parse()
                .context("--when must be \"next-week\" or a number of days")?;
            ensure!(days > 0, "--when day count must be positive");
            let today = tz::parts_in_zone(now_ms, zone).date;
            let start = tz::add_days_ymd(&today, 1)?;
            let end = tz::add_days_ymd(&start, days)?;
            (start, end, args.when.clone())
        }
    };
    let start = tz::zoned_local_to_utc(&start_ymd, zone)?;
    let end = tz::zoned_local_to_utc(&end_ymd, zone)?;
    Ok(SearchWindow {
        start_ms: start.as_millisecond(),
        start_utc: tz::to_graph_utc(start.as_millisecond()),
        end_utc: tz::to_graph_utc(end.as_millisecond()),
        label,
    })
}

fn zone_of(args: &WindowArgs) -> Result<(String, jiff::tz::TimeZone)> {
    let name = args.tz.clone().unwrap_or_else(tz::system_time_zone);
    let zone = tz::tz(&name)?;
    Ok((name, zone))
}

fn parse_hours(flag: &str, spec: &str) -> Result<Hours> {
    let (start, end) = spec
        .split_once('-')
        .with_context(|| format!("{flag} must be like 9-17"))?;
    let hours = Hours {
        start: start
            .trim()
            .parse()
            .with_context(|| format!("{flag} must be like 9-17"))?,
        end: end
            .trim()
            .parse()
            .with_context(|| format!("{flag} must be like 9-17"))?,
    };
    ensure!(hours.start < hours.end, "{flag} start must be before end");
    Ok(hours)
}

// ---- Shared export pieces ----

/// Deduplicate every schedule item into the export's event list.
fn collect_events(
    schedules: &[ScheduleInformation],
    zone: &jiff::tz::TimeZone,
    classifications: &Classifications,
) -> Vec<Event> {
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    for s in schedules {
        for item in &s.schedule_items {
            let id = rank::item_id(item, &s.schedule_id);
            let start_ms = tz::graph_datetime_to_ms(
                &item.start.date_time,
                item.start.time_zone.as_deref(),
                zone,
            );
            let end_ms =
                tz::graph_datetime_to_ms(&item.end.date_time, item.end.time_zone.as_deref(), zone);
            let (Some(start_ms), Some(end_ms)) = (start_ms, end_ms) else {
                continue;
            };
            events.entry(id.clone()).or_insert_with(|| Event {
                id: id.clone(),
                person: s.schedule_id.clone(),
                subject: item.subject.clone(),
                status: item.status.clone(),
                is_private: item.is_private.unwrap_or(false),
                location: item.location.clone(),
                start_utc: format!("{}Z", tz::to_graph_utc(start_ms)),
                end_utc: format!("{}Z", tz::to_graph_utc(end_ms)),
                generic_by_keyword: rank::is_generic_subject(item.subject.as_deref()),
                classification: classifications.verdict(&id).map(String::from),
            });
        }
    }
    events.into_values().collect()
}

/// Graph returns an empty availabilityView when it cannot read a calendar
/// (bad address, external mailbox, hidden free/busy). Say so instead of
/// letting those people silently rank as "unknown" everywhere.
fn warn_unreadable(schedules: &[ScheduleInformation]) {
    for s in schedules {
        if s.availability_view.is_empty() {
            eprintln!(
                "warning: no free/busy for {} — treating every slot as unknown",
                s.schedule_id
            );
        }
    }
}

fn write_json<T: serde::Serialize>(
    value: &T,
    out: Option<&PathBuf>,
    to_stdout: bool,
) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(out) = out {
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::write(out, &json)?;
        eprintln!("wrote {}", out.display());
    }
    if to_stdout {
        println!("{json}");
    }
    Ok(())
}

// ---- Commands ----

async fn run_setup() -> Result<()> {
    let (_, me) = ensure_token(true, true).await?;
    println!("Signed in as {}", me.user_principal_name);
    Ok(())
}

fn run_status() -> Result<()> {
    println!("profile:  {}", profile_dir().display());
    let Some(cached) = token::load() else {
        println!("session:  none (no valid cached token) — run: loom-teams setup");
        return Ok(());
    };
    let claims = &cached.claims;
    let user = claims["upn"]
        .as_str()
        .or(claims["unique_name"].as_str())
        .unwrap_or("unknown");
    println!("session:  {user}");
    println!("app:      {}", token::app_name(claims));
    println!("scopes:   {}", token::calendar_scopes(claims).join(" "));
    if let Some(exp) = token::exp_epoch(claims) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        println!("expires:  in {} min", exp.saturating_sub(now) / 60);
    }
    Ok(())
}

async fn run_export(args: ExportArgs) -> Result<()> {
    let (zone_name, zone) = zone_of(&args.window)?;
    let (client, me) = session(args.headed).await?;
    let people = client.resolve_people(&me, &args.who).await?;
    let window = search_window(&zone, &args.window)?;

    let emails: Vec<String> = people.iter().map(|p| p.email.clone()).collect();
    let schedules = client
        .get_schedule(
            &emails,
            &window.start_utc,
            &window.end_utc,
            args.interval,
            &zone_name,
        )
        .await?;
    warn_unreadable(&schedules);

    let raw = RawExport {
        schema: model::EXPORT_SCHEMA,
        me: me.user_principal_name,
        window: Window {
            start_utc: format!("{}Z", window.start_utc),
            end_utc: format!("{}Z", window.end_utc),
            tz: zone_name,
        },
        interval_minutes: args.interval,
        people,
        events: collect_events(&schedules, &zone, &Classifications::default()),
        availability: schedules
            .iter()
            .map(|s| AvailabilityView {
                who: s.schedule_id.clone(),
                view: s.availability_view.clone(),
            })
            .collect(),
    };
    write_json(&raw, args.out.as_ref(), args.out.is_none())
}

async fn run_find(args: FindArgs) -> Result<()> {
    let (zone_name, zone) = zone_of(&args.window)?;
    let hours = parse_hours("--hours", &args.hours)?;
    let expanded = parse_hours("--expanded", &args.expanded)?;
    let classifications = load_classifications(args.classifications.as_ref())?;

    let (client, me) = session(args.headed).await?;
    let people = client.resolve_people(&me, &args.who).await?;
    let window = search_window(&zone, &args.window)?;

    let emails: Vec<String> = people.iter().map(|p| p.email.clone()).collect();
    let schedules = client
        .get_schedule(
            &emails,
            &window.start_utc,
            &window.end_utc,
            args.interval,
            &zone_name,
        )
        .await?;
    warn_unreadable(&schedules);

    let analysis = rank::analyze(&RankInput {
        schedules: &schedules,
        window_start_ms: window.start_ms,
        zone: &zone,
        interval: args.interval,
        duration: args.duration,
        preferred: hours,
        expanded,
        top: args.top,
        classifications: &classifications,
    });

    let export = Export {
        schema: model::FIND_SCHEMA,
        me: me.user_principal_name.clone(),
        query: Query {
            people: args.who.clone(),
            when: window.label.clone(),
            duration_minutes: args.duration,
            interval_minutes: args.interval,
            tz: zone_name.clone(),
            hours,
            expanded,
            top: args.top,
        },
        window: Window {
            start_utc: format!("{}Z", window.start_utc),
            end_utc: format!("{}Z", window.end_utc),
            tz: zone_name,
        },
        people: people.clone(),
        events: collect_events(&schedules, &zone, &classifications),
        grid: analysis.grid,
        picks: analysis.picks,
    };

    write_json(&export, args.out.as_ref(), args.json)?;
    if !args.json {
        print_summary(&export, &people, args.duration);
    }
    Ok(())
}

fn load_classifications(path: Option<&PathBuf>) -> Result<Classifications> {
    match path {
        None => Ok(Classifications::default()),
        Some(p) => {
            let raw = std::fs::read_to_string(p)
                .with_context(|| format!("reading classifications {}", p.display()))?;
            Ok(serde_json::from_str(&raw)
                .context("classifications must be {\"<eventId>\": \"generic\"|\"real\"}")?)
        }
    }
}

fn print_summary(export: &Export, people: &[Person], duration: u32) {
    let monday = export
        .grid
        .first()
        .map(|g| g.date.clone())
        .unwrap_or_default();
    let monday_rows: Vec<_> = export.grid.iter().filter(|g| g.date == monday).collect();
    let monday_open: Vec<_> = monday_rows.iter().filter(|g| g.bookable).collect();
    println!("Why not Monday ({monday})?");
    if monday_rows.is_empty() {
        println!("  no Monday rows in the window");
    } else if let Some(first) = monday_open.first() {
        println!("  it is bookable from {} ({})", first.time, first.kind);
    } else {
        println!("  every in-hours slot is a real meeting or OOF for at least one of you:");
        for g in &monday_rows {
            let who_busy: Vec<String> = g
                .per_person
                .iter()
                .filter(|p| p.kind == "busy" || p.kind == "oof")
                .map(|p| {
                    let short = p.who.split('@').next().unwrap_or(&p.who);
                    let what = if p.subjects.is_empty() {
                        p.kind.clone()
                    } else {
                        p.subjects.join(", ")
                    };
                    format!("{short}: {what}")
                })
                .collect();
            println!("  {}  {}", g.time, who_busy.join(" · "));
        }
    }
    if let Some(first) = export.grid.iter().find(|g| g.bookable) {
        println!(
            "\nFirst bookable in hours: {} {} ({})",
            first.date, first.time, first.kind
        );
    }

    if export.picks.is_empty() {
        println!("\nNo candidate slots in the window.");
        return;
    }
    let names: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
    println!(
        "\nTop {} by score for {} ({}, {}m):\n",
        export.picks.len(),
        names.join(", "),
        export.query.when,
        duration
    );
    for pick in &export.picks {
        println!(
            "{}. {}  (score {:+}, {})",
            pick.rank, pick.label, pick.score, pick.kind
        );
        println!("   {}", pick.reason);
        for f in &pick.factors {
            println!(
                "   {:>4}  {}: {}",
                format!("{:+}", f.points),
                f.name,
                f.note
            );
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Setup => run_setup().await,
        Command::Status => run_status(),
        Command::Export(args) => run_export(args).await,
        Command::Find(args) => run_find(args).await,
    };
    if let Err(err) = result {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
