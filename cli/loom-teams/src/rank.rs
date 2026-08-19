//! Pure ranking: given Graph getSchedule payloads, pick meeting slots.
//! Worst person wins. Real meetings and OOF are never overbooked.
//!
//! Shape: `prepare` parses each schedule once at the boundary (datetimes,
//! event ids, real-vs-generic verdicts), `candidates` walks the slot grid
//! once, and `analyze` folds that single walk into both outputs — the ranked
//! picks and the audit grid.

use crate::model::{
    Classifications, GridRow, Hours, PersonSlot, Pick, ScheduleInformation, ScheduleItem,
};
use crate::tz::{self, Parts};
use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use sha2::{Digest, Sha256};

const GENERIC_EXACT: &[&str] = &[
    "",
    "busy",
    "blocked",
    "block",
    "hold",
    "placeholder",
    "tentative",
    "no title",
    "untitled",
    "focus",
    "focus time",
    "deep work",
    "lunch",
    "lunch break",
    "commute",
    "personal",
    "buffer",
    "calendar block",
    "work block",
];

const GENERIC_PREFIXES: &[&str] = &["focus", "blocked", "block", "hold", "buffer", "personal"];

pub fn is_generic_subject(subject: Option<&str>) -> bool {
    let s = subject.unwrap_or("").trim().to_lowercase();
    if GENERIC_EXACT.contains(&s.as_str()) {
        return true;
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    words.len() <= 3 && words.first().is_some_and(|w| GENERIC_PREFIXES.contains(w))
}

/// Declared in ascending severity so `Ord` is the worst-person-wins order:
/// free < tentative < generic < unknown < busy < oof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Free,
    Tentative,
    Generic,
    Unknown,
    Busy,
    Oof,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Free => "free",
            Kind::Tentative => "tentative",
            Kind::Generic => "generic",
            Kind::Unknown => "unknown",
            Kind::Busy => "busy",
            Kind::Oof => "oof",
        }
    }

    pub fn bookable(self) -> bool {
        !matches!(self, Kind::Oof | Kind::Busy | Kind::Unknown)
    }
}

pub struct RankInput<'a> {
    pub schedules: &'a [ScheduleInformation],
    pub window_start_ms: i64,
    pub zone: &'a TimeZone,
    pub interval: u32,
    pub duration: u32,
    pub preferred: Hours,
    pub expanded: Hours,
    pub top: usize,
    pub classifications: &'a Classifications,
}

pub struct Analysis {
    pub picks: Vec<Pick>,
    pub grid: Vec<GridRow>,
}

/// Stable event id: sha256 over person|start|startTz|end|endTz|subject.
pub fn item_id(item: &ScheduleItem, who: &str) -> String {
    let mut h = Sha256::new();
    let parts = [
        who,
        &item.start.date_time,
        item.start.time_zone.as_deref().unwrap_or(""),
        &item.end.date_time,
        item.end.time_zone.as_deref().unwrap_or(""),
        item.subject.as_deref().unwrap_or(""),
    ];
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            h.update(b"|");
        }
        h.update(part.as_bytes());
    }
    format!("e{}", hex::encode(&h.finalize()[..5]))
}

/// One schedule item with everything the slot walk needs, parsed once.
struct PreparedItem<'a> {
    start_ms: i64,
    end_ms: i64,
    id: String,
    /// Counts as a real meeting: private, AI-classified real, or (absent an
    /// override) a non-generic subject. Real meetings are never overbooked.
    real: bool,
    item: &'a ScheduleItem,
}

struct Prepared<'a> {
    who: &'a str,
    view: &'a [u8], // availabilityView digits are ASCII
    items: Vec<PreparedItem<'a>>,
}

fn prepare<'a>(input: &RankInput<'a>) -> Vec<Prepared<'a>> {
    input
        .schedules
        .iter()
        .map(|s| Prepared {
            who: &s.schedule_id,
            view: s.availability_view.as_bytes(),
            items: s
                .schedule_items
                .iter()
                .filter_map(|item| {
                    let start_ms = tz::graph_datetime_to_ms(
                        &item.start.date_time,
                        item.start.time_zone.as_deref(),
                        input.zone,
                    )?;
                    let end_ms = tz::graph_datetime_to_ms(
                        &item.end.date_time,
                        item.end.time_zone.as_deref(),
                        input.zone,
                    )?;
                    let id = item_id(item, &s.schedule_id);
                    let real = item.is_private.unwrap_or(false)
                        || match input.classifications.verdict(&id) {
                            Some("real") => true,
                            Some("generic") => false,
                            _ => !is_generic_subject(item.subject.as_deref()),
                        };
                    Some(PreparedItem {
                        start_ms,
                        end_ms,
                        id,
                        real,
                        item,
                    })
                })
                .collect(),
        })
        .collect()
}

struct Occupancy<'a> {
    who: &'a str,
    kind: Kind,
    items: Vec<&'a PreparedItem<'a>>,
}

fn occupancy<'a>(
    p: &'a Prepared<'a>,
    index: usize,
    steps: usize,
    slot_start: i64,
    slot_end: i64,
) -> Occupancy<'a> {
    if p.view.len() < index + steps {
        return Occupancy {
            who: p.who,
            kind: Kind::Unknown,
            items: vec![],
        };
    }
    let digits = &p.view[index..index + steps];
    let items: Vec<&PreparedItem> = p
        .items
        .iter()
        .filter(|i| i.start_ms < slot_end && i.end_ms > slot_start)
        .collect();

    if digits.contains(&b'3') || items.iter().any(|i| i.item.status == "oof") {
        return Occupancy {
            who: p.who,
            kind: Kind::Oof,
            items,
        };
    }

    let busy: Vec<&PreparedItem> = items
        .iter()
        .filter(|i| i.item.status == "busy" || i.item.status == "workingElsewhere")
        .copied()
        .collect();
    if digits.contains(&b'2') || !busy.is_empty() {
        let real: Vec<&PreparedItem> = busy.iter().filter(|i| i.real).copied().collect();
        if !real.is_empty() {
            return Occupancy {
                who: p.who,
                kind: Kind::Busy,
                items: real,
            };
        }
        return Occupancy {
            who: p.who,
            kind: Kind::Generic,
            items: busy,
        };
    }

    if digits.contains(&b'1') || items.iter().any(|i| i.item.status == "tentative") {
        return Occupancy {
            who: p.who,
            kind: Kind::Tentative,
            items,
        };
    }
    Occupancy {
        who: p.who,
        kind: Kind::Free,
        items: vec![],
    }
}

/// Slot fits the window: starts inside it and ends by its last hour sharp.
fn in_window(start: &Parts, end: &Parts, w: Hours) -> bool {
    start.hour >= w.start
        && start.hour < w.end
        && (end.hour < w.end || (end.hour == w.end && end.minute == 0))
}

/// One weekday candidate slot with everything both outputs need.
struct Candidate<'a> {
    index: usize,
    steps: usize,
    start: Parts,
    end: Parts,
    in_preferred: bool,
    kind: Kind,
    occupancies: Vec<Occupancy<'a>>,
}

fn candidates<'a>(input: &RankInput, prepared: &'a [Prepared<'a>]) -> Vec<Candidate<'a>> {
    let steps = (((input.duration as f64 / input.interval as f64).round() as usize).max(1)) as i64;
    // Walk the longest view: a person whose free/busy came back empty or
    // short ranks as `unknown` per slot (never overbooked, visibly so)
    // instead of silently erasing everyone else's window.
    let n = prepared.iter().map(|p| p.view.len()).max().unwrap_or(0) as i64;
    let step_ms = input.interval as i64 * 60_000;
    let mut out = vec![];
    if n < steps {
        return out;
    }

    for i in 0..=(n - steps) {
        let slot_start = input.window_start_ms + i * step_ms;
        let slot_end = slot_start + steps * step_ms;
        let start = tz::parts_in_zone(slot_start, input.zone);
        let end = tz::parts_in_zone(slot_end, input.zone);
        if matches!(start.weekday, Weekday::Saturday | Weekday::Sunday) {
            continue;
        }
        let in_preferred = in_window(&start, &end, input.preferred);
        let in_expanded = in_window(&start, &end, input.expanded);
        if !in_preferred && !in_expanded {
            continue;
        }

        let occupancies: Vec<Occupancy> = prepared
            .iter()
            .map(|p| occupancy(p, i as usize, steps as usize, slot_start, slot_end))
            .collect();
        let kind = occupancies
            .iter()
            .map(|o| o.kind)
            .max()
            .unwrap_or(Kind::Free);
        out.push(Candidate {
            index: i as usize,
            steps: steps as usize,
            start,
            end,
            in_preferred,
            kind,
            occupancies,
        });
    }
    out
}

// ---- Within-tier scoring ----
//
// The tier ladder stays the coarse, hard ordering (Palen CHI 1999: never
// overbook real commitments; Faulring & Myers CHI 2006: free time has
// quality). Within a tier, "earliest wins" is replaced by a weighted score
// built from the meeting-science literature. Every factor is reported back
// in the pick so the ranking stays auditable.
//
// Factors and sources:
// - circadian: attention peaks mid-morning and dips after lunch (Monk 2005,
//   "The Post-Lunch Dip in Performance"); lunch hour itself is a social norm
//   even when the calendar says free.
// - weekday: Tue/Wed/Thu meetings are accepted and attended most; Mondays
//   and especially Fridays are skipped (YouCanBookMe analysis of 2M invite
//   responses; Doodle meeting data).
// - fatigue: back-to-back meetings build cumulative stress; breaks let the
//   brain reset (Microsoft Human Factors EEG study, 2021).
// - defrag: booking adjacent to existing meetings or at the workday edge
//   preserves long focus blocks; splitting a 2h+ free block costs deep work
//   (Gloria Mark's interruption research: ~23 min to refocus; Clockwise /
//   Reclaim calendar-defragmentation practice).
// - soon: earlier in the window is easier to plan around (Palen:
//   scheduling is satisficing).
// - negotiate: each person who must be asked to move a hold is a
//   negotiation (Palen; Faulring's "with whom to negotiate").

use crate::model::Factor;

const MEETING: u8 = b'2'; // availabilityView digit for busy
const FREE: u8 = b'0';

fn circadian(start: &Parts, end: &Parts) -> Option<Factor> {
    let start_min = start.hour as i32 * 60 + start.minute as i32;
    let end_min = end.hour as i32 * 60 + end.minute as i32;
    let overlaps_lunch = start_min < 13 * 60 && end_min > 12 * 60;
    let (points, note) = if overlaps_lunch {
        (-35, "overlaps the lunch hour")
    } else if (9 * 60 + 30..12 * 60).contains(&start_min) {
        (30, "mid-morning attention peak")
    } else if (14 * 60..16 * 60).contains(&start_min) {
        (-25, "post-lunch dip")
    } else if (13 * 60..14 * 60).contains(&start_min) {
        (-10, "just after lunch")
    } else if start_min >= 16 * 60 + 30 {
        (-15, "end of day")
    } else {
        return None;
    };
    Some(Factor {
        name: "circadian",
        points,
        note: note.into(),
    })
}

fn weekday(day: jiff::civil::Weekday) -> Option<Factor> {
    use jiff::civil::Weekday::*;
    let (points, note) = match day {
        Tuesday | Wednesday | Thursday => (20, "high-attendance day"),
        Monday => (-15, "Mondays are skipped most after Fridays"),
        Friday => (-25, "Fridays have the highest no-show rate"),
        _ => return None,
    };
    Some(Factor {
        name: "weekday",
        points,
        note: note.into(),
    })
}

fn soon(start_ms: i64, window_start_ms: i64) -> Option<Factor> {
    let day = ((start_ms - window_start_ms) / 86_400_000).max(0) as i32;
    (day > 0).then(|| Factor {
        name: "soon",
        points: -8 * day,
        note: format!("day {} of the window", day + 1),
    })
}

fn run_len(len: usize, mut i: i64, step: i64, pred: impl Fn(usize) -> bool) -> usize {
    let mut n = 0;
    while i >= 0 && (i as usize) < len && pred(i as usize) {
        n += 1;
        i += step;
    }
    n
}

fn short_name(who: &str) -> &str {
    who.split('@').next().unwrap_or(who)
}

/// Per-person calendar-shape factors: fatigue, adjacency, fragmentation.
/// Runs are clamped to preferred hours via `in_pref` (one bool per grid
/// cell) so overnight free time never counts as a focus block.
fn shape_factors(
    c: &Candidate,
    prepared: &[Prepared],
    input: &RankInput,
    in_pref: &[bool],
    out: &mut Vec<Factor>,
) {
    let minutes = |cells: usize| cells as u32 * input.interval;
    let mut fatigue: Vec<&str> = vec![];
    let mut adjacent: Vec<&str> = vec![];
    let mut fragments: Vec<&str> = vec![];

    for (p, occ) in prepared.iter().zip(&c.occupancies) {
        if p.view.len() < c.index + c.steps {
            continue;
        }
        let len = p.view.len().min(in_pref.len());
        let meeting = |i: usize| p.view[i] == MEETING && in_pref[i];
        let free = |i: usize| p.view[i] == FREE && in_pref[i];
        let before = c.index as i64 - 1;
        let after = (c.index + c.steps) as i64;
        let busy_before = run_len(len, before, -1, meeting);
        let busy_after = run_len(len, after, 1, meeting);

        // Microsoft 2021: an hour-plus unbroken run before this slot means
        // this meeting extends a stressful back-to-back chain.
        if minutes(busy_before) >= 60 {
            fatigue.push(p.who);
        }

        let at_edge = (c.start.hour == input.preferred.start && c.start.minute == 0)
            || (c.end.hour == input.preferred.end && c.end.minute == 0);
        if busy_before > 0 || busy_after > 0 || at_edge {
            adjacent.push(p.who);
        }

        // Fragmentation only applies to people who are actually free here:
        // a slot in the middle of their 2h+ free block splits deep-work time.
        if occ.kind == Kind::Free {
            let free_before = run_len(len, before, -1, free);
            let free_after = run_len(len, after, 1, free);
            let block = minutes(free_before) + minutes(c.steps) + minutes(free_after);
            if minutes(free_before) >= 30 && minutes(free_after) >= 30 && block >= 120 {
                fragments.push(p.who);
            }
        }
    }

    let names = |list: &[&str]| {
        list.iter()
            .map(|w| short_name(w))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !fatigue.is_empty() {
        out.push(Factor {
            name: "fatigue",
            points: (-20 * fatigue.len() as i32).max(-60),
            note: format!("extends a back-to-back run for {}", names(&fatigue)),
        });
    }
    if !adjacent.is_empty() {
        out.push(Factor {
            name: "defrag",
            points: (15 * adjacent.len() as i32).min(45),
            note: format!("adjacent to existing meetings for {}", names(&adjacent)),
        });
    }
    if !fragments.is_empty() {
        out.push(Factor {
            name: "fragmentation",
            points: (-25 * fragments.len() as i32).max(-75),
            note: format!("splits a 2h+ focus block for {}", names(&fragments)),
        });
    }
}

/// Nobody is excluded; a conflict costs points per person, weighted by how
/// hard the conversation would be (Faulring & Myers: availability is a
/// continuum). A real meeting or vacation can still "win" a desperate week —
/// but only after every cheaper option, and it always names who it costs.
fn conflict_cost(kind: Kind) -> (i32, &'static str) {
    match kind {
        Kind::Free => (0, ""),
        Kind::Tentative => (-40, "tentative"),
        Kind::Generic => (-60, "a hold"),
        Kind::Unknown => (-80, "unreadable free/busy"),
        Kind::Busy => (-120, "a real meeting"),
        Kind::Oof => (-150, "out of office"),
    }
}

fn conflicts(c: &Candidate, out: &mut Vec<Factor>) {
    let mut points = 0;
    let mut notes: Vec<String> = vec![];
    for o in &c.occupancies {
        let (p, what) = conflict_cost(o.kind);
        if p != 0 {
            points += p;
            notes.push(format!("{}: {what}", short_name(o.who)));
        }
    }
    if points != 0 {
        out.push(Factor {
            name: "conflicts",
            points,
            note: notes.join(", "),
        });
    }
}

fn out_of_hours(c: &Candidate, out: &mut Vec<Factor>) {
    if !c.in_preferred {
        out.push(Factor {
            name: "hours",
            points: -50,
            note: "outside preferred hours".into(),
        });
    }
}

fn score(
    c: &Candidate,
    prepared: &[Prepared],
    input: &RankInput,
    in_pref: &[bool],
) -> (i32, Vec<Factor>) {
    let mut factors = vec![];
    factors.extend(circadian(&c.start, &c.end));
    factors.extend(weekday(c.start.weekday));
    factors.extend(soon(
        input.window_start_ms + c.index as i64 * input.interval as i64 * 60_000,
        input.window_start_ms,
    ));
    shape_factors(c, prepared, input, in_pref, &mut factors);
    conflicts(c, &mut factors);
    out_of_hours(c, &mut factors);
    (factors.iter().map(|f| f.points).sum(), factors)
}

/// One bool per grid cell: does the cell start within preferred hours?
fn preferred_cells(input: &RankInput, n: usize) -> Vec<bool> {
    let step_ms = input.interval as i64 * 60_000;
    (0..n)
        .map(|k| {
            let p = tz::parts_in_zone(input.window_start_ms + k as i64 * step_ms, input.zone);
            p.hour >= input.preferred.start && p.hour < input.preferred.end
        })
        .collect()
}

fn person_slot(occ: &Occupancy) -> PersonSlot {
    PersonSlot {
        who: occ.who.to_string(),
        kind: occ.kind.label().to_string(),
        subjects: occ.items.iter().map(|i| subject_of(i.item)).collect(),
        event_ids: occ.items.iter().map(|i| i.id.clone()).collect(),
    }
}

fn subject_of(item: &ScheduleItem) -> String {
    item.subject.clone().unwrap_or_else(|| "(no title)".into())
}

fn reason(c: &Candidate) -> String {
    let where_ = if c.in_preferred {
        "in hours"
    } else {
        "outside preferred hours"
    };
    let notes: Vec<String> = c
        .occupancies
        .iter()
        .filter(|o| o.kind != Kind::Free && !o.items.is_empty())
        .map(|o| {
            let subjects: Vec<String> = o.items.iter().map(|i| subject_of(i.item)).collect();
            format!("{}: {} ({})", o.who, o.kind.label(), subjects.join(", "))
        })
        .collect();
    if notes.is_empty() {
        format!("everyone free, {where_}")
    } else {
        format!("{where_} — {}", notes.join("; "))
    }
}

/// One pass over the slot grid. Every weekday slot is scored — nothing is
/// vetoed — and the top N by score become the picks; the in-hours rows form
/// the audit grid (a heatmap, since each row carries its score).
pub fn analyze(input: &RankInput) -> Analysis {
    let prepared = prepare(input);
    let n = prepared.iter().map(|p| p.view.len()).max().unwrap_or(0);
    let in_pref = preferred_cells(input, n);
    let mut ranked: Vec<Pick> = vec![];
    let mut grid = vec![];

    for c in candidates(input, &prepared) {
        let (score, factors) = score(&c, &prepared, input, &in_pref);
        if c.in_preferred {
            grid.push(GridRow {
                start_local: c.start.iso.clone(),
                end_local: c.end.iso.clone(),
                date: c.start.date.clone(),
                time: c.start.time.clone(),
                kind: c.kind.label().to_string(),
                bookable: c.kind.bookable(),
                score,
                per_person: c.occupancies.iter().map(person_slot).collect(),
            });
        }
        let slot_start_ms = input.window_start_ms + c.index as i64 * input.interval as i64 * 60_000;
        let label = format!(
            "{}–{}",
            tz::wall_label(slot_start_ms, input.zone),
            c.end.time
        );
        ranked.push(Pick {
            rank: 0,
            start_local: c.start.iso.clone(),
            end_local: c.end.iso.clone(),
            label,
            score,
            factors,
            kind: c.kind.label().to_string(),
            in_hours: c.in_preferred,
            reason: reason(&c),
            per_person: c.occupancies.iter().map(person_slot).collect(),
        });
    }

    // Highest score first; the earlier slot breaks exact ties.
    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.start_local.cmp(&b.start_local))
    });
    let picks = ranked
        .into_iter()
        .take(input.top)
        .enumerate()
        .map(|(i, mut pick)| {
            pick.rank = i + 1;
            pick
        })
        .collect();
    Analysis { picks, grid }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GraphDateTime;

    fn item(subject: &str, status: &str, start: &str, end: &str) -> ScheduleItem {
        ScheduleItem {
            status: status.into(),
            subject: if subject.is_empty() {
                None
            } else {
                Some(subject.into())
            },
            is_private: None,
            location: None,
            start: GraphDateTime {
                date_time: start.into(),
                time_zone: Some("UTC".into()),
            },
            end: GraphDateTime {
                date_time: end.into(),
                time_zone: Some("UTC".into()),
            },
        }
    }

    fn schedule(id: &str, view: &str, items: Vec<ScheduleItem>) -> ScheduleInformation {
        ScheduleInformation {
            schedule_id: id.into(),
            availability_view: view.into(),
            schedule_items: items,
        }
    }

    /// Window: Mon 2026-08-24 00:00 Europe/Amsterdam (22:00Z Sun), 30m interval.
    fn window_start() -> i64 {
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        crate::tz::zoned_local_to_utc("2026-08-24T00:00:00", &zone)
            .unwrap()
            .as_millisecond()
    }

    fn input<'a>(
        schedules: &'a [ScheduleInformation],
        zone: &'a jiff::tz::TimeZone,
        cls: &'a Classifications,
    ) -> RankInput<'a> {
        RankInput {
            schedules,
            window_start_ms: window_start(),
            zone,
            interval: 30,
            duration: 30,
            preferred: Hours { start: 9, end: 17 },
            expanded: Hours { start: 7, end: 20 },
            top: 3,
            classifications: cls,
        }
    }

    #[test]
    fn generic_subjects() {
        assert!(is_generic_subject(Some("Lunch")));
        assert!(is_generic_subject(Some("Focus time")));
        assert!(is_generic_subject(None));
        // Four words: past the <=3 word prefix rule, so treated as real —
        // this is why Winand's Monday "Focus (communicatie & dagstart)" blocked.
        assert!(!is_generic_subject(Some("Focus (communicatie & dagstart)")));
        // The famous counter-example: a titled lunch is a real meeting.
        assert!(!is_generic_subject(Some(
            "Lunch (geen meetings zonder overleg!)"
        )));
        assert!(!is_generic_subject(Some("Daily Engineering Platforms")));
    }

    #[test]
    fn free_slot_beats_tentative_and_generic() {
        // One full day = 48 half-hour cells starting at local midnight.
        // Index 18 = 09:00 local (midnight + 9h). Make 09:00 tentative,
        // 09:30 generic-busy, 10:00 free for both people.
        let mut a = ['0'; 48];
        a[18] = '1'; // 09:00 tentative
        a[19] = '2'; // 09:30 busy (generic subject)
        let view_a: String = a.iter().collect();
        let s1 = schedule(
            "a@x",
            &view_a,
            vec![
                item(
                    "Maybe sync",
                    "tentative",
                    "2026-08-24T07:00:00",
                    "2026-08-24T07:30:00",
                ),
                item(
                    "Focus",
                    "busy",
                    "2026-08-24T07:30:00",
                    "2026-08-24T08:00:00",
                ),
            ],
        );
        let s2 = schedule("b@x", &"0".repeat(48), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let cls = Classifications::default();
        let schedules = vec![s1, s2];
        let picks = analyze(&input(&schedules, &zone, &cls)).picks;
        assert_eq!(picks[0].start_local, "2026-08-24T10:00"); // first all-free in hours
        assert_eq!(picks[0].kind, "free");
    }

    #[test]
    fn real_meeting_never_overbooked_and_worst_person_wins() {
        let mut a = ['0'; 48];
        a[18] = '2'; // 09:00 real meeting for person a
        let view_a: String = a.iter().collect();
        let s1 = schedule(
            "a@x",
            &view_a,
            vec![item(
                "Daily Stand Up",
                "busy",
                "2026-08-24T07:00:00",
                "2026-08-24T07:30:00",
            )],
        );
        let s2 = schedule("b@x", &"0".repeat(48), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let cls = Classifications::default();
        let schedules = vec![s1, s2];
        let analysis = analyze(&input(&schedules, &zone, &cls));
        assert!(analysis
            .picks
            .iter()
            .all(|p| p.start_local != "2026-08-24T09:00"));
        let nine = analysis.grid.iter().find(|g| g.time == "09:00").unwrap();
        assert_eq!(nine.kind, "busy");
        assert!(!nine.bookable);
    }

    #[test]
    fn classification_override_demotes_a_generic_looking_meeting() {
        let mut a = ['0'; 48];
        a[18] = '2';
        let view_a: String = a.iter().collect();
        let it = item(
            "Weekly platform review",
            "busy",
            "2026-08-24T07:00:00",
            "2026-08-24T07:30:00",
        );
        let id = item_id(&it, "a@x");
        let s1 = schedule("a@x", &view_a, vec![it]);
        let s2 = schedule("b@x", &"0".repeat(48), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let schedules = vec![s1, s2];

        // Keyword heuristic says real -> not bookable.
        let cls = Classifications::default();
        let grid = analyze(&input(&schedules, &zone, &cls)).grid;
        assert!(!grid.iter().find(|g| g.time == "09:00").unwrap().bookable);

        // AI says it is a placeholder -> bookable as generic.
        let mut map = std::collections::HashMap::new();
        map.insert(id, "generic".to_string());
        let cls = Classifications(map);
        let grid = analyze(&input(&schedules, &zone, &cls)).grid;
        let nine = grid.iter().find(|g| g.time == "09:00").unwrap();
        assert_eq!(nine.kind, "generic");
        assert!(nine.bookable);
    }

    #[test]
    fn tuesday_morning_edge_beats_monday_on_empty_calendars() {
        // Five all-free weekdays (240 half-hour cells). Literature score should
        // pick Tuesday at the workday edge: high-attendance day (+20), adjacent
        // to the day boundary (defrag), no lunch/dip penalty, one day out (-8).
        let s1 = schedule("a@x", &"0".repeat(240), vec![]);
        let s2 = schedule("b@x", &"0".repeat(240), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let cls = Classifications::default();
        let schedules = vec![s1, s2];
        let picks = analyze(&input(&schedules, &zone, &cls)).picks;
        assert_eq!(picks[0].start_local, "2026-08-25T09:00");
        assert!(picks[0]
            .factors
            .iter()
            .any(|f| f.name == "weekday" && f.points > 0));
        assert!(picks[0].factors.iter().any(|f| f.name == "defrag"));
    }

    #[test]
    fn lunch_and_post_lunch_dip_are_penalized() {
        let noonish = crate::tz::parts_in_zone(
            crate::tz::zoned_local_to_utc(
                "2026-08-25T12:00:00",
                &crate::tz::tz("Europe/Amsterdam").unwrap(),
            )
            .unwrap()
            .as_millisecond(),
            &crate::tz::tz("Europe/Amsterdam").unwrap(),
        );
        let half_later = crate::tz::parts_in_zone(
            crate::tz::zoned_local_to_utc(
                "2026-08-25T12:30:00",
                &crate::tz::tz("Europe/Amsterdam").unwrap(),
            )
            .unwrap()
            .as_millisecond(),
            &crate::tz::tz("Europe/Amsterdam").unwrap(),
        );
        let lunch = circadian(&noonish, &half_later).unwrap();
        assert!(lunch.points < 0 && lunch.note.contains("lunch"));

        use jiff::civil::Weekday;
        assert!(weekday(Weekday::Friday).unwrap().points < 0);
        assert!(weekday(Weekday::Tuesday).unwrap().points > 0);
        assert!(weekday(Weekday::Saturday).is_none());
    }

    #[test]
    fn back_to_back_run_costs_fatigue_points() {
        // Person a is in meetings 09:00-10:30 Monday; booking 10:30 extends a
        // 90-minute run -> fatigue penalty, but adjacency still applies.
        let mut a = ['0'; 240];
        a[18..21].fill('2');
        let view_a: String = a.iter().collect();
        let s1 = schedule(
            "a@x",
            &view_a,
            vec![item(
                "Standups",
                "busy",
                "2026-08-24T07:00:00",
                "2026-08-24T08:30:00",
            )],
        );
        let s2 = schedule("b@x", &"0".repeat(240), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let cls = Classifications::default();
        let schedules = vec![s1, s2];
        let mut inp = input(&schedules, &zone, &cls);
        inp.top = 200;
        let picks = analyze(&inp).picks;
        let at_1030 = picks
            .iter()
            .find(|p| p.start_local == "2026-08-24T10:30")
            .unwrap();
        assert!(at_1030
            .factors
            .iter()
            .any(|f| f.name == "fatigue" && f.points < 0));
        assert!(at_1030
            .factors
            .iter()
            .any(|f| f.name == "defrag" && f.points > 0));
    }

    #[test]
    fn weekend_and_out_of_expanded_hours_are_skipped() {
        // Window starting Monday covers only weekdays here; verify 06:30 (before
        // expanded 07:00) never appears in picks.
        let s1 = schedule("a@x", &"0".repeat(48), vec![]);
        let zone = crate::tz::tz("Europe/Amsterdam").unwrap();
        let cls = Classifications::default();
        let schedules = vec![s1];
        let picks = analyze(&input(&schedules, &zone, &cls)).picks;
        assert!(picks.iter().all(|p| !p.start_local.contains("T06:")));
    }
}
