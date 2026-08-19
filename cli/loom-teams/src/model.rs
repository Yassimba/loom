//! Graph wire types and the exported JSON contract (`teams-slots/1`).
//! The export is the seam for the rest of the pipeline: an AI pass can
//! classify `events[*]` and feed the verdicts back via `--classifications`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---- Graph wire types ----

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInformation {
    pub schedule_id: String,
    #[serde(default)]
    pub availability_view: String,
    #[serde(default)]
    pub schedule_items: Vec<ScheduleItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleItem {
    #[serde(default)]
    pub status: String, // free | tentative | busy | oof | workingElsewhere
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub is_private: Option<bool>,
    #[serde(default)]
    pub location: Option<String>,
    pub start: GraphDateTime,
    pub end: GraphDateTime,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDateTime {
    pub date_time: String,
    #[serde(default)]
    pub time_zone: Option<String>,
}

// ---- Export contracts ----

pub const FIND_SCHEMA: &str = "loom-teams/find/2";
pub const EXPORT_SCHEMA: &str = "loom-teams/export/1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    pub schema: &'static str,
    pub me: String,
    pub query: Query,
    pub window: Window,
    pub people: Vec<Person>,
    /// Every schedule item in the window, deduplicated, with a stable id.
    pub events: Vec<Event>,
    /// Every in-hours candidate slot with per-person occupancy.
    pub grid: Vec<GridRow>,
    /// The ranked picks (top N).
    pub picks: Vec<Pick>,
}

/// `export` output: the raw calendar data primitive every other use case
/// (reports, LLM classification, slot finding) builds on.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawExport {
    pub schema: &'static str,
    pub me: String,
    pub window: Window,
    pub interval_minutes: u32,
    pub people: Vec<Person>,
    /// Every schedule item in the window, deduplicated, with a stable id.
    pub events: Vec<Event>,
    /// Graph's free/busy digit string per person (one digit per interval,
    /// starting at the window start): 0 free, 1 tentative, 2 busy, 3 oof,
    /// 4 working elsewhere. Covers people whose event details are hidden.
    pub availability: Vec<AvailabilityView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityView {
    pub who: String,
    pub view: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub people: Vec<String>,
    pub when: String,
    pub duration_minutes: u32,
    pub interval_minutes: u32,
    pub tz: String,
    pub hours: Hours,
    pub expanded: Hours,
    pub top: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Hours {
    pub start: i8,
    pub end: i8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub start_utc: String,
    pub end_utc: String,
    pub tz: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub query: String,
    pub email: String,
    pub name: String,
}

/// One calendar block, with the keyword heuristic's verdict attached.
/// `id` is stable across runs (hash of person|start|end|subject) so an AI
/// classification pass can be cached and replayed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub person: String,
    pub subject: Option<String>,
    pub status: String,
    pub is_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub start_utc: String,
    pub end_utc: String,
    pub generic_by_keyword: bool,
    /// Filled in when a `--classifications` file overrode the keyword verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRow {
    pub start_local: String,
    pub end_local: String,
    pub date: String,
    pub time: String,
    pub kind: String,
    pub bookable: bool,
    /// Same score the picks use — heatmap data for rendering.
    pub score: i32,
    pub per_person: Vec<PersonSlot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonSlot {
    pub who: String,
    pub kind: String,
    pub subjects: Vec<String>,
    pub event_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub rank: usize,
    pub start_local: String,
    pub end_local: String,
    pub label: String,
    /// Quality score (higher is better); sum of `factors`. Nothing is
    /// excluded — conflicts cost points instead of vetoing the slot.
    pub score: i32,
    /// Auditable score breakdown, literature-backed (see rank.rs).
    pub factors: Vec<Factor>,
    pub kind: String,
    pub in_hours: bool,
    pub reason: String,
    pub per_person: Vec<PersonSlot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Factor {
    pub name: &'static str,
    pub points: i32,
    pub note: String,
}

/// `--classifications` file: `{ "<eventId>": "generic" | "real" }`.
#[derive(Debug, Default, Deserialize)]
pub struct Classifications(pub HashMap<String, String>);

impl Classifications {
    pub fn verdict(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(String::as_str)
    }
}
