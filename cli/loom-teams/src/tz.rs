//! IANA <-> UTC conversions and Graph datetime parsing.
//! Graph availabilityView is indexed from a UTC window; every wall-clock label
//! must go through the requested IANA zone (the "two hours early" bug).

use anyhow::{Context, Result};
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::Timestamp;

/// Windows time zone names Graph may return on scheduleItems.
const WINDOWS_TO_IANA: &[(&str, &str)] = &[
    ("UTC", "UTC"),
    ("tzone://Microsoft/Utc", "UTC"),
    ("W. Europe Standard Time", "Europe/Amsterdam"),
    ("Central Europe Standard Time", "Europe/Warsaw"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("Romance Standard Time", "Europe/Paris"),
    ("GMT Standard Time", "Europe/London"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Eastern Standard Time", "America/New_York"),
];

pub fn to_iana(zone: &str) -> &str {
    if zone.is_empty() {
        return "UTC";
    }
    if zone.contains('/') {
        return zone;
    }
    WINDOWS_TO_IANA
        .iter()
        .find(|(win, _)| *win == zone)
        .map(|(_, iana)| *iana)
        .unwrap_or(zone)
}

pub fn system_time_zone() -> String {
    TimeZone::system().iana_name().unwrap_or("UTC").to_string()
}

pub fn tz(zone: &str) -> Result<TimeZone> {
    TimeZone::get(to_iana(zone)).with_context(|| format!("unknown time zone {zone}"))
}

/// Wall-clock parts of an instant in a zone.
pub struct Parts {
    pub date: String, // YYYY-MM-DD
    pub time: String, // HH:MM
    pub iso: String,  // YYYY-MM-DDTHH:MM
    pub hour: i8,
    pub minute: i8,
    pub weekday: jiff::civil::Weekday,
}

pub fn parts_in_zone(instant_ms: i64, zone: &TimeZone) -> Parts {
    let ts = Timestamp::from_millisecond(instant_ms).expect("instant in range");
    let z = ts.to_zoned(zone.clone());
    let date = format!("{:04}-{:02}-{:02}", z.year(), z.month(), z.day());
    let time = format!("{:02}:{:02}", z.hour(), z.minute());
    Parts {
        iso: format!("{date}T{time}"),
        hour: z.hour(),
        minute: z.minute(),
        weekday: z.weekday(),
        date,
        time,
    }
}

/// Instant at which `zone` shows local wall time `YYYY-MM-DD[THH:MM[:SS]]`.
pub fn zoned_local_to_utc(iso_local: &str, zone: &TimeZone) -> Result<Timestamp> {
    let padded = match iso_local.len() {
        10 => format!("{iso_local}T00:00:00"),
        16 => format!("{iso_local}:00"),
        _ => iso_local[..19.min(iso_local.len())].to_string(),
    };
    let dt: DateTime = padded
        .parse()
        .with_context(|| format!("bad local datetime {iso_local}"))?;
    Ok(dt.to_zoned(zone.clone())?.timestamp())
}

/// Parse a Graph `{dateTime, timeZone}` pair into epoch ms, honouring the
/// item's own zone (Windows or IANA), falling back to the request zone.
/// jiff parses Graph's `.0000000` fractions and `Z`/offset suffixes itself.
pub fn graph_datetime_to_ms(
    date_time: &str,
    item_zone: Option<&str>,
    fallback: &TimeZone,
) -> Option<i64> {
    if let Ok(ts) = date_time.parse::<Timestamp>() {
        return Some(ts.as_millisecond());
    }
    let dt: DateTime = date_time.parse().ok()?;
    let zone = match item_zone {
        Some(z) if !z.is_empty() => tz(z).ok()?,
        _ => fallback.clone(),
    };
    Some(dt.to_zoned(zone).ok()?.timestamp().as_millisecond())
}

fn zoned(instant_ms: i64, zone: &TimeZone) -> jiff::Zoned {
    Timestamp::from_millisecond(instant_ms)
        .expect("instant in range")
        .to_zoned(zone.clone())
}

/// Graph request datetime: UTC, seconds precision, no suffix.
pub fn to_graph_utc(instant_ms: i64) -> String {
    zoned(instant_ms, &TimeZone::UTC)
        .strftime("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

/// Wall-clock slot label, e.g. "Tue 25 Aug 09:30".
pub fn wall_label(instant_ms: i64, zone: &TimeZone) -> String {
    zoned(instant_ms, zone)
        .strftime("%a %-d %b %H:%M")
        .to_string()
}

pub fn add_days_ymd(ymd: &str, days: i64) -> Result<String> {
    let d: jiff::civil::Date = ymd.parse().with_context(|| format!("bad date {ymd}"))?;
    let d2 = d.checked_add(jiff::Span::new().days(days))?;
    Ok(format!(
        "{:04}-{:02}-{:02}",
        d2.year(),
        d2.month(),
        d2.day()
    ))
}

/// Monday of next week, as seen from `zone` today.
pub fn next_week_monday(zone: &TimeZone, now_ms: i64) -> String {
    let p = parts_in_zone(now_ms, zone);
    let today: jiff::civil::Date = p.date.parse().expect("own format");
    // days until next Monday, always at least 7 - matching the prototype:
    // Monday today => next Monday is in 7 days.
    let dow = today.weekday().to_monday_zero_offset() as i64; // Mon=0..Sun=6
    let delta = if dow == 0 { 7 } else { 7 - dow };
    let monday = today
        .checked_add(jiff::Span::new().days(delta))
        .expect("date in range");
    format!(
        "{:04}-{:02}-{:02}",
        monday.year(),
        monday.month(),
        monday.day()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amsterdam_summer_offset() {
        let zone = tz("Europe/Amsterdam").unwrap();
        let ts = zoned_local_to_utc("2026-08-24T00:00:00", &zone).unwrap();
        // CEST = UTC+2 -> local midnight is 22:00 UTC the day before.
        assert_eq!(to_graph_utc(ts.as_millisecond()), "2026-08-23T22:00:00");
    }

    #[test]
    fn wall_label_matches_old_hand_rolled_format() {
        let zone = tz("Europe/Amsterdam").unwrap();
        let ms = zoned_local_to_utc("2026-08-25T09:30:00", &zone)
            .unwrap()
            .as_millisecond();
        assert_eq!(wall_label(ms, &zone), "Tue 25 Aug 09:30");
    }

    #[test]
    fn windows_zone_maps() {
        assert_eq!(to_iana("W. Europe Standard Time"), "Europe/Amsterdam");
        assert_eq!(to_iana("Europe/Amsterdam"), "Europe/Amsterdam");
    }

    #[test]
    fn graph_item_datetime_with_windows_zone() {
        let fallback = tz("UTC").unwrap();
        let ms = graph_datetime_to_ms(
            "2026-08-24T11:00:00.0000000",
            Some("W. Europe Standard Time"),
            &fallback,
        )
        .unwrap();
        assert_eq!(to_graph_utc(ms), "2026-08-24T09:00:00");
    }

    #[test]
    fn next_monday_from_wednesday() {
        let zone = tz("Europe/Amsterdam").unwrap();
        // Wed 2026-08-19 12:00 CEST
        let now = zoned_local_to_utc("2026-08-19T12:00:00", &zone)
            .unwrap()
            .as_millisecond();
        assert_eq!(next_week_monday(&zone, now), "2026-08-24");
    }

    #[test]
    fn next_monday_from_monday_is_a_week_out() {
        let zone = tz("Europe/Amsterdam").unwrap();
        let now = zoned_local_to_utc("2026-08-24T09:00:00", &zone)
            .unwrap()
            .as_millisecond();
        assert_eq!(next_week_monday(&zone, now), "2026-08-31");
    }
}
