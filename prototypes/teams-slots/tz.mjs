const WINDOWS_TO_IANA = {
  UTC: "UTC",
  "tzone://Microsoft/Utc": "UTC",
  "W. Europe Standard Time": "Europe/Amsterdam",
  "Central Europe Standard Time": "Europe/Warsaw",
  "Central European Standard Time": "Europe/Warsaw",
  "Romance Standard Time": "Europe/Paris",
  "GMT Standard Time": "Europe/London",
  "GTB Standard Time": "Europe/Bucharest",
  "E. Europe Standard Time": "Europe/Chisinau",
  "Pacific Standard Time": "America/Los_Angeles",
  "Eastern Standard Time": "America/New_York",
};

export function systemTimeZone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

export function toIana(zone) {
  if (!zone) return "UTC";
  if (zone.includes("/")) return zone;
  return WINDOWS_TO_IANA[zone] ?? zone;
}

export function partsInZone(instant, timeZone) {
  const dtf = new Intl.DateTimeFormat("en-US", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    weekday: "short",
    hourCycle: "h23",
  });
  const map = Object.fromEntries(dtf.formatToParts(instant).map((p) => [p.type, p.value]));
  const iso = `${map.year}-${map.month}-${map.day}T${map.hour}:${map.minute}`;
  return {
    year: Number(map.year),
    month: Number(map.month),
    day: Number(map.day),
    hour: Number(map.hour),
    minute: Number(map.minute),
    second: Number(map.second),
    weekday: map.weekday,
    date: `${map.year}-${map.month}-${map.day}`,
    time: `${map.hour}:${map.minute}`,
    iso,
  };
}

export function formatInZone(instant, timeZone) {
  return partsInZone(instant, timeZone).iso;
}

/** Instant at which `timeZone` shows `isoLocal` (`YYYY-MM-DDTHH:mm` or with seconds). */
export function zonedLocalToUtc(isoLocal, timeZone) {
  const iana = toIana(timeZone);
  const padded = isoLocal.length === 16 ? `${isoLocal}:00` : isoLocal.slice(0, 19);
  if (iana === "UTC") return new Date(`${padded}Z`);
  const asUtc = new Date(`${padded}Z`);
  const shown = partsInZone(asUtc, iana);
  const shownAsUtc = Date.UTC(shown.year, shown.month - 1, shown.day, shown.hour, shown.minute, shown.second);
  return new Date(asUtc.getTime() - (shownAsUtc - asUtc.getTime()));
}

export function graphDateTimeToUtc(dtz, fallbackTz) {
  if (!dtz?.dateTime) return null;
  const raw = String(dtz.dateTime).replace(/\.\d+/, "");
  if (raw.endsWith("Z") || /[+-]\d{2}:\d{2}$/.test(raw)) return new Date(raw);
  const zone = dtz.timeZone ? toIana(dtz.timeZone) : toIana(fallbackTz);
  return zonedLocalToUtc(raw.slice(0, 19), zone);
}

export function ymdInZone(instant, timeZone) {
  return partsInZone(instant, timeZone).date;
}

export function addDaysYmd(ymd, days) {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d + days)).toISOString().slice(0, 10);
}

export function toGraphUtc(instant) {
  return {
    dateTime: instant.toISOString().slice(0, 19),
    timeZone: "UTC",
  };
}
