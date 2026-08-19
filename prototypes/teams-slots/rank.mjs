// PROTOTYPE — wipe me.
// Pure ranking: given Graph getSchedule payloads, pick meeting slots.
// Worst person wins. Real meetings and OOF are never overbooked.

import { graphDateTimeToUtc, partsInZone } from "./tz.mjs";

export const TIER = {
  FREE_IN: 0,
  TENTATIVE_IN: 1,
  GENERIC_IN: 2,
  FREE_OUT: 3,
  TENTATIVE_OUT: 4,
  GENERIC_OUT: 5,
};

const TIER_LABEL = {
  [TIER.FREE_IN]: "all free, in hours",
  [TIER.TENTATIVE_IN]: "over a tentative, in hours",
  [TIER.GENERIC_IN]: "over a generic block, in hours",
  [TIER.FREE_OUT]: "all free, outside hours",
  [TIER.TENTATIVE_OUT]: "over a tentative, outside hours",
  [TIER.GENERIC_OUT]: "over a generic block, outside hours",
};

const GENERIC_EXACT = new Set([
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
]);

export function isGenericSubject(subject) {
  const s = (subject ?? "").trim().toLowerCase();
  if (GENERIC_EXACT.has(s)) return true;
  if (/^(focus|blocked?|hold|buffer|personal)\b/.test(s) && s.split(/\s+/).length <= 3) {
    return true;
  }
  return false;
}

export function addMinutesToWall(iso, minutes) {
  const d = new Date(`${iso}Z`);
  d.setUTCMinutes(d.getUTCMinutes() + minutes);
  return d.toISOString().replace("Z", "").slice(0, 16);
}

export function labelWall(iso, tz = "UTC") {
  const instant = iso.endsWith("Z") || iso.includes("+") ? new Date(iso) : new Date(`${iso}:00Z`);
  const p = partsInZone(instant, tz);
  const day = new Date(`${p.date}T12:00:00Z`).toLocaleDateString("en-US", {
    weekday: "short",
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  });
  return `${day} ${p.time}`;
}

function slotRange(windowStartMs, index, steps, interval) {
  const start = windowStartMs + index * interval * 60_000;
  const end = windowStartMs + (index + steps) * interval * 60_000;
  return { start, end };
}

function itemOverlaps(item, slotStartMs, slotEndMs, tz) {
  const a = graphDateTimeToUtc(item.start, tz)?.getTime();
  const b = graphDateTimeToUtc(item.end, tz)?.getTime();
  if (a == null || b == null) return false;
  return a < slotEndMs && b > slotStartMs;
}

function personOccupancy(schedule, startIndex, steps, interval, windowStartMs, tz) {
  const digits = (schedule.availabilityView ?? "").slice(startIndex, startIndex + steps);
  if (digits.length < steps) return { kind: "unknown", items: [] };

  const { start: slotStart, end: slotEnd } = slotRange(windowStartMs, startIndex, steps, interval);
  const items = (schedule.scheduleItems ?? []).filter((item) =>
    itemOverlaps(item, slotStart, slotEnd, tz),
  );

  if ([...digits].includes("3") || items.some((i) => i.status === "oof")) {
    return { kind: "oof", items };
  }

  const busyItems = items.filter((i) => i.status === "busy" || i.status === "workingElsewhere");
  if ([...digits].includes("2") || busyItems.length) {
    const real = busyItems.filter((i) => i.isPrivate || !isGenericSubject(i.subject));
    if (real.length) return { kind: "busy", items: real };
    return { kind: "generic", items: busyItems };
  }

  if ([...digits].includes("1") || items.some((i) => i.status === "tentative")) {
    return { kind: "tentative", items };
  }
  return { kind: "free", items: [] };
}

const WORST = ["oof", "busy", "unknown", "generic", "tentative", "free"];

function worstKind(kinds) {
  return WORST.find((k) => kinds.includes(k)) ?? "free";
}

function tierFor(kind, inHours) {
  if (kind === "free") return inHours ? TIER.FREE_IN : TIER.FREE_OUT;
  if (kind === "tentative") return inHours ? TIER.TENTATIVE_IN : TIER.TENTATIVE_OUT;
  if (kind === "generic") return inHours ? TIER.GENERIC_IN : TIER.GENERIC_OUT;
  return null;
}

function reason(kind, inHours, perPerson) {
  const base = TIER_LABEL[tierFor(kind, inHours)];
  const notes = perPerson
    .filter((p) => p.kind !== "free" && p.items?.length)
    .map((p) => {
      const sub = p.items.map((i) => i.subject || "(no title)").join(", ");
      return `${p.who}: ${p.kind} (${sub})`;
    });
  return notes.length ? `${base} — ${notes.join("; ")}` : base;
}

export function rankSlots({
  schedules,
  windowStartMs,
  tz,
  interval,
  duration,
  preferred = { startHour: 9, endHour: 17 },
  expanded = { startHour: 7, endHour: 20 },
  top = 3,
}) {
  const steps = Math.max(1, Math.round(duration / interval));
  const views = schedules.map((s) => s.availabilityView ?? "");
  const n = views.length ? Math.min(...views.map((v) => v.length)) : 0;
  const found = [];

  for (let i = 0; i + steps <= n; i++) {
    const { start, end } = slotRange(windowStartMs, i, steps, interval);
    const startP = partsInZone(new Date(start), tz);
    const endP = partsInZone(new Date(end), tz);
    const dow = new Date(`${startP.date}T12:00:00Z`).getUTCDay();
    if (dow === 0 || dow === 6) continue;
    if (startP.hour < expanded.startHour || startP.hour >= expanded.endHour) continue;
    if (endP.hour > expanded.endHour || (endP.hour === expanded.endHour && endP.minute > 0)) {
      continue;
    }

    const perPerson = schedules.map((s) => ({
      who: s.scheduleId,
      ...personOccupancy(s, i, steps, interval, windowStartMs, tz),
    }));
    const kind = worstKind(perPerson.map((p) => p.kind));
    if (kind === "oof" || kind === "busy" || kind === "unknown") continue;

    const inHours =
      startP.hour >= preferred.startHour &&
      startP.hour < preferred.endHour &&
      (endP.hour < preferred.endHour || (endP.hour === preferred.endHour && endP.minute === 0));

    const tier = tierFor(kind, inHours);
    found.push({
      start: startP.iso,
      end: endP.iso,
      label: `${labelWall(new Date(start).toISOString(), tz)}–${endP.time}`,
      tier,
      kind,
      inHours,
      reason: reason(kind, inHours, perPerson),
      perPerson: perPerson.map((p) => ({
        who: p.who,
        kind: p.kind,
        subjects: (p.items ?? []).map((i) => i.subject || "(no title)"),
      })),
    });
  }

  found.sort((a, b) => a.tier - b.tier || a.start.localeCompare(b.start));
  return found.slice(0, top);
}

export function inspectGrid({
  schedules,
  windowStartMs,
  tz,
  interval,
  duration,
  preferred = { startHour: 9, endHour: 17 },
}) {
  const steps = Math.max(1, Math.round(duration / interval));
  const views = schedules.map((s) => s.availabilityView ?? "");
  const n = views.length ? Math.min(...views.map((v) => v.length)) : 0;
  const rows = [];

  for (let i = 0; i + steps <= n; i++) {
    const { start, end } = slotRange(windowStartMs, i, steps, interval);
    const startP = partsInZone(new Date(start), tz);
    const endP = partsInZone(new Date(end), tz);
    const dow = new Date(`${startP.date}T12:00:00Z`).getUTCDay();
    if (dow === 0 || dow === 6) continue;
    const inHours =
      startP.hour >= preferred.startHour &&
      startP.hour < preferred.endHour &&
      (endP.hour < preferred.endHour || (endP.hour === preferred.endHour && endP.minute === 0));
    if (!inHours) continue;

    const perPerson = schedules.map((s) => ({
      who: s.scheduleId,
      ...personOccupancy(s, i, steps, interval, windowStartMs, tz),
    }));
    const kind = worstKind(perPerson.map((p) => p.kind));
    rows.push({
      start: startP.iso,
      end: endP.iso,
      date: startP.date,
      time: startP.time,
      kind,
      bookable: kind !== "oof" && kind !== "busy" && kind !== "unknown",
      perPerson: perPerson.map((p) => ({
        who: p.who,
        kind: p.kind,
        subjects: (p.items ?? []).map((item) => item.subject || "(no title)"),
      })),
    });
  }
  return rows;
}

export function nextWeekMonday(tz, from = new Date()) {
  const ymd = new Intl.DateTimeFormat("en-CA", {
    timeZone: tz,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(from);
  const [y, m, d] = ymd.split("-").map(Number);
  const dow = new Date(Date.UTC(y, m - 1, d, 12)).getUTCDay();
  const delta = dow === 1 ? 7 : (8 - dow) % 7 || 7;
  const monday = new Date(Date.UTC(y, m - 1, d + delta));
  return monday.toISOString().slice(0, 10);
}
