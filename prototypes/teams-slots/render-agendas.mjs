import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const KIND_LABEL = {
  free: "free",
  tentative: "tentative",
  generic: "generic hold",
  busy: "meeting",
  oof: "out of office",
  unknown: "unknown",
};

function shortName(person, email) {
  const raw = person?.name || email || "";
  const parts = raw.split(/[,\s]+/).filter(Boolean);
  if (parts.length >= 2 && raw.includes(",")) return parts.at(-1);
  return parts[0] || email.split("@")[0];
}

function uniqueNames(people) {
  const shorts = people.map((p) => shortName(p, p.email));
  const counts = new Map();
  for (const s of shorts) counts.set(s, (counts.get(s) ?? 0) + 1);
  return people.map((p, i) => {
    if ((counts.get(shorts[i]) ?? 0) === 1) return shorts[i];
    const last = (p.name || p.email).split(",")[0].trim();
    return `${shorts[i]} ${last.slice(0, 1)}.`;
  });
}

function esc(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function dayLabel(date) {
  return new Date(`${date}T12:00:00Z`).toLocaleDateString("en-US", {
    weekday: "short",
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  });
}

function personIndex(emails, who) {
  const needle = String(who).toLowerCase();
  return emails.findIndex((e) => e.toLowerCase() === needle);
}

export function writeAgendaHtml({ path, people, grid, picks, hours, when, duration }) {
  const emails = people.map((p) => p.email);
  const names = uniqueNames(people);
  const dates = [...new Set(grid.map((g) => g.date))];
  const times = [...new Set(grid.filter((g) => g.date === dates[0]).map((g) => g.time))];
  const byKey = new Map(grid.map((g) => [`${g.date}|${g.time}`, g]));
  const pickAt = new Map(picks.map((p, i) => [p.start, i + 1]));
  const firstBookable = grid.find((g) => g.bookable);
  const firstFree = grid.find((g) => g.kind === "free");

  const days = dates
    .map((date) => {
      const slots = times.map((time) => byKey.get(`${date}|${time}`)).filter(Boolean);
      const hoursRow = times
        .map((t, i) =>
          t.endsWith(":00")
            ? `<span style="grid-column:${i + 1}">${Number(t.slice(0, 2))}</span>`
            : "",
        )
        .join("");

      const groupCells = slots
        .map((g) => {
          const pick = pickAt.get(g.start) ?? "";
          const freeN = g.perPerson.filter((p) => p.kind === "free").length;
          const blockers = g.perPerson
            .filter((p) => p.kind === "busy" || p.kind === "oof")
            .map((p) => {
              const n = names[personIndex(emails, p.who)] || p.who;
              return `${n}: ${p.subjects.join(", ") || KIND_LABEL[p.kind]}`;
            });
          const tip = JSON.stringify({
            time: `${dayLabel(date)} ${g.time}`,
            kind: g.kind,
            pick,
            people: g.perPerson.map((p) => ({
              name: names[personIndex(emails, p.who)] || p.who,
              kind: p.kind,
              subjects: p.subjects,
            })),
            blockers,
          });
          const count = g.bookable ? "" : `<em>${freeN}</em>`;
          return `<button type="button" class="c ${g.kind}${pick ? " pick" : ""}" data-tip="${esc(tip)}" data-start="${g.start}">${
            pick ? `<b>${pick}</b>` : count
          }</button>`;
        })
        .join("");

      const personRows = names
        .map((name, i) => {
          const cells = slots
            .map((g) => {
              const p = g.perPerson[i];
              const pick = pickAt.get(g.start) ?? "";
              const tip = JSON.stringify({
                time: `${dayLabel(date)} ${g.time}`,
                kind: p?.kind,
                pick,
                people: [
                  {
                    name,
                    kind: p?.kind,
                    subjects: p?.subjects ?? [],
                  },
                ],
                blockers: [],
              });
              return `<button type="button" class="c ${p?.kind ?? "unknown"}${pick ? " pick" : ""}" data-tip="${esc(tip)}"></button>`;
            })
            .join("");
          return `<div class="row"><span class="who" title="${esc(people[i].email)}">${esc(name)}</span><div class="cells">${cells}</div></div>`;
        })
        .join("");

      return `<section class="day" id="day-${date}">
        <h2>${esc(dayLabel(date))}</h2>
        <div class="row hours"><span class="who"></span><div class="cells hours-cells">${hoursRow}</div></div>
        <div class="row group"><span class="who">all ${names.length}</span><div class="cells">${groupCells}</div></div>
        ${personRows}
      </section>`;
    })
    .join("");

  const pickCards = picks
    .map((p, i) => {
      const why =
        p.kind === "free"
          ? "everyone free"
          : p.kind === "tentative"
            ? "over a tentative"
            : "over a generic hold";
      return `<button type="button" class="card" data-start="${p.start}"><i>${i + 1}</i><div><strong>${esc(p.label)}</strong><small>${esc(why)}</small></div></button>`;
    })
    .join("");

  const html = `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(names.join(" + "))} · ${esc(when)}</title>
<style>
  :root {
    --bg: #10110e;
    --ink: #ece7db;
    --mute: #8a867a;
    --line: #262820;
    --free: #3dd68c;
    --tent: #f5c14a;
    --gen: #f0883e;
    --busy: #e24b4a;
    --oof: #5c5c54;
    --unk: #2a2b26;
    --pick: #f4efe4;
  }
  * { box-sizing: border-box; }
  html, body { margin: 0; background: var(--bg); color: var(--ink); }
  body { font: 15px/1.4 "IBM Plex Sans", "Source Sans 3", ui-sans-serif, system-ui, sans-serif; padding: 28px 28px 80px; }
  header { display: flex; justify-content: space-between; gap: 24px; align-items: end; flex-wrap: wrap; margin-bottom: 22px; }
  h1 { font: 600 22px/1.15 "IBM Plex Sans", ui-sans-serif, sans-serif; letter-spacing: -0.03em; margin: 0 0 6px; }
  .sub { color: var(--mute); margin: 0; font-size: 13px; }
  .legend { display: flex; gap: 12px; flex-wrap: wrap; font-size: 12px; color: var(--mute); }
  .legend i { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 5px; vertical-align: -1px; }
  .picks { display: flex; gap: 8px; flex-wrap: wrap; }
  .card { display: flex; gap: 10px; align-items: center; background: #181a15; border: 1px solid var(--line); color: inherit; border-radius: 10px; padding: 8px 12px 8px 8px; cursor: pointer; text-align: left; }
  .card:hover, .card.on { border-color: #6b6e62; }
  .card i { width: 22px; height: 22px; border-radius: 6px; background: var(--pick); color: #111; display: grid; place-items: center; font: 700 12px/1 ui-sans-serif; }
  .card strong { display: block; font-size: 13px; }
  .card small { color: var(--mute); font-size: 11px; }
  .day { margin: 0 0 26px; }
  .day h2 { font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--mute); margin: 0 0 8px; }
  .row { display: grid; grid-template-columns: 108px 1fr; gap: 8px; align-items: stretch; margin: 0 0 2px; }
  .who { font-size: 12px; color: #c4c0b4; display: flex; align-items: center; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .group .who { color: var(--ink); font-weight: 650; font-size: 12px; }
  .cells { display: grid; grid-template-columns: repeat(${times.length}, minmax(0, 1fr)); gap: 2px; }
  .c { appearance: none; border: 0; padding: 0; height: 18px; border-radius: 2px; cursor: pointer; position: relative; background: var(--unk); }
  .group .c { height: 28px; border-radius: 3px; }
  .hours { margin-bottom: 4px; }
  .hours-cells { position: relative; height: 14px; }
  .hours-cells span { font: 10px ui-monospace, SFMono-Regular, monospace; color: var(--mute); }
  .c.free { background: var(--free); }
  .c.tentative { background: var(--tent); }
  .c.generic { background: var(--gen); }
  .c.busy { background: var(--busy); }
  .c.oof { background: var(--oof); }
  .c.pick { box-shadow: inset 0 0 0 2px var(--pick); }
  .c b, .c em { position: absolute; inset: 0; display: grid; place-items: center; font: 700 11px/1 ui-sans-serif; }
  .c b { color: #111; }
  .c em { color: #fff; font-weight: 650; opacity: 0.9; font-size: 10px; }
  .row:hover .who { color: #fff; }
  #tip { position: fixed; z-index: 5; max-width: 320px; background: #1c1e18; border: 1px solid #3a3d34; border-radius: 10px; padding: 10px 12px; font-size: 12px; pointer-events: none; opacity: 0; transform: translateY(4px); transition: opacity .12s; }
  #tip.on { opacity: 1; transform: none; }
  #tip .t { color: var(--mute); font-family: ui-monospace, monospace; font-size: 11px; margin-bottom: 6px; }
  #tip li { display: flex; gap: 8px; margin: 3px 0; list-style: none; }
  #tip ul { margin: 0; padding: 0; }
  #tip .k { width: 8px; height: 8px; border-radius: 2px; margin-top: 4px; flex: none; }
  .note { color: var(--mute); font-size: 13px; max-width: 40rem; margin: 8px 0 0; }
</style>
<body>
  <header>
    <div>
      <h1>${esc(names.join(" + "))}</h1>
      <p class="sub">${esc(when)} · ${esc(String(duration))}m · ${esc(hours)} · ${names.length} people</p>
      <p class="legend">
        <span><i class="c free"></i>free</span>
        <span><i class="c tentative"></i>tentative</span>
        <span><i class="c generic"></i>generic</span>
        <span><i class="c busy"></i>meeting</span>
        <span><i class="c oof"></i>ooo</span>
        <span>group numbers = how many are free when the slot is blocked</span>
      </p>
    </div>
    <div class="picks">${pickCards}</div>
  </header>
  ${days}
  <p class="note">${
    firstFree && firstBookable && firstFree.start !== firstBookable.start
      ? `First bookable is ${esc(firstBookable.date)} ${esc(firstBookable.time)} (${esc(firstBookable.kind)}). First moment everyone is actually free is ${esc(firstFree.date)} ${esc(firstFree.time)}.`
      : firstFree
        ? `First moment everyone is free: ${esc(firstFree.date)} ${esc(firstFree.time)}.`
        : "No all-free slot in hours."
  }</p>
  <div id="tip"></div>
<script>
const tip = document.getElementById("tip");
const kindColor = {free:"#3dd68c",tentative:"#f5c14a",generic:"#f0883e",busy:"#e24b4a",oof:"#5c5c54",unknown:"#2a2b26"};
function show(e, raw) {
  const d = JSON.parse(raw);
  const people = (d.people || []).map(p =>
    "<li><i class=k style=background:"+kindColor[p.kind]+"></i><span><b>"+p.name+"</b> "+p.kind+(p.subjects?.length ? " · "+p.subjects.join(", ") : "")+"</span></li>"
  ).join("");
  tip.innerHTML = "<div class=t>"+d.time+(d.pick ? " · pick "+d.pick : "")+"</div><ul>"+people+"</ul>";
  tip.classList.add("on");
  move(e);
}
function move(e) {
  const x = Math.min(e.clientX + 14, innerWidth - tip.offsetWidth - 12);
  const y = Math.min(e.clientY + 14, innerHeight - tip.offsetHeight - 12);
  tip.style.left = x + "px";
  tip.style.top = y + "px";
}
function hide() { tip.classList.remove("on"); }
for (const el of document.querySelectorAll("[data-tip]")) {
  el.addEventListener("pointerenter", e => show(e, el.dataset.tip));
  el.addEventListener("pointermove", move);
  el.addEventListener("pointerleave", hide);
}
function flash(start) {
  document.querySelectorAll(".card").forEach(c => c.classList.toggle("on", c.dataset.start === start));
  const cell = document.querySelector('.group [data-start="'+start+'"]');
  cell?.closest(".day")?.scrollIntoView({ behavior: "smooth", block: "center" });
}
document.querySelectorAll(".card").forEach(c => c.addEventListener("click", () => flash(c.dataset.start)));
</script>
</body>
</html>`;

  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, html);
}
