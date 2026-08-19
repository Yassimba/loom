import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

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

export function writeGalleryHtml({ path, people, grid, picks, hours, when, duration }) {
  const names = uniqueNames(people);
  const dates = [...new Set(grid.map((g) => g.date))];
  const times = [...new Set(grid.filter((g) => g.date === dates[0]).map((g) => g.time))];
  const data = {
    names,
    emails: people.map((p) => p.email),
    dates,
    times,
    hours,
    when,
    duration,
    picks,
    grid,
  };

  const html = `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>10 scheduling views · ${names.join(" + ")}</title>
<style>
  :root {
    --bg: #f3efe6; --ink: #1a1914; --mute: #6b675c; --card: #fffcf6; --line: #ddd6c6;
    --free: #2f9e44; --tent: #f59f00; --gen: #e8590c; --busy: #c92a2a; --oof: #868e96;
  }
  * { box-sizing: border-box; }
  html, body { margin: 0; background: var(--bg); color: var(--ink); }
  body { font: 15px/1.45 "Iowan Old Style", Palatino, Georgia, serif; padding: 28px 28px 96px; }
  h1 { font-size: 1.7rem; letter-spacing: -0.03em; margin: 0 0 6px; }
  .lede { color: var(--mute); max-width: 40rem; margin: 0 0 28px; }
  nav { display: flex; flex-wrap: wrap; gap: 6px; margin: 0 0 32px; }
  nav a { color: var(--ink); text-decoration: none; font: 12px/1 ui-sans-serif, system-ui; border: 1px solid var(--line); background: var(--card); padding: 6px 8px; border-radius: 999px; }
  nav a:hover { border-color: #111; }
  section { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 20px 22px 24px; margin: 0 0 18px; }
  section h2 { font-size: 1.05rem; margin: 0 0 4px; }
  section p.why { color: var(--mute); font-size: 13px; margin: 0 0 14px; max-width: 44rem; }
  .cite { font-size: 11px; color: #8a8578; }
  .row { display: grid; grid-template-columns: 92px 1fr; gap: 8px; align-items: center; margin: 0 0 2px; }
  .who { font: 11px/1.2 ui-sans-serif, system-ui; color: #444; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cells { display: grid; gap: 2px; }
  .c { height: 16px; border-radius: 2px; background: #e9e4d8; position: relative; border: 0; padding: 0; cursor: pointer; }
  .c.pick { box-shadow: inset 0 0 0 2px #111; }
  .c b { position: absolute; inset: 0; display: grid; place-items: center; font: 700 9px/1 ui-sans-serif; }
  .hours { font: 10px ui-monospace, monospace; color: var(--mute); }
  .hours span { opacity: .85; }
  .day-label { font: 700 11px/1 ui-sans-serif; letter-spacing: .06em; text-transform: uppercase; color: var(--mute); margin: 12px 0 6px; }
  .day-label:first-child { margin-top: 0; }
  .cards { display: flex; flex-wrap: wrap; gap: 8px; }
  .card { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; min-width: 200px; cursor: pointer; background: #fff; text-align: left; font: inherit; color: inherit; }
  .card.on, .card:hover { border-color: #111; }
  .card i { font: 700 12px ui-sans-serif; }
  .card small { display: block; color: var(--mute); font-size: 12px; }
  .scatter { position: relative; height: 220px; border: 1px solid var(--line); border-radius: 10px; background: #fff; }
  .scatter .dot { position: absolute; width: 12px; height: 12px; border-radius: 50%; transform: translate(-50%,50%); border: 0; cursor: pointer; }
  .scatter .dot.pick { width: 18px; height: 18px; box-shadow: 0 0 0 2px #111; }
  .scatter .axis { position: absolute; font: 10px ui-sans-serif; color: var(--mute); }
  .blockers { display: grid; gap: 8px; }
  .blockers article { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; background: #fff; cursor: pointer; }
  .chip { display: inline-block; font: 11px ui-sans-serif; background: #f4efe4; border-radius: 999px; padding: 2px 8px; margin: 2px 4px 0 0; }
  .inspector { position: sticky; bottom: 12px; background: #1b1a16; color: #f4efe4; border-radius: 14px; padding: 14px 16px; margin-top: 8px; font: 13px/1.4 ui-sans-serif, system-ui; }
  .inspector h3 { margin: 0 0 6px; font-size: 13px; letter-spacing: .04em; text-transform: uppercase; color: #b5b09f; }
  .inspector li { margin: 3px 0; }
  .zoom-nav { display: flex; gap: 6px; margin: 0 0 10px; }
  .zoom-nav button { font: 12px ui-sans-serif; border: 1px solid var(--line); background: #fff; border-radius: 8px; padding: 6px 8px; cursor: pointer; }
  .zoom-nav button.on { background: #1b1a16; color: #fff; border-color: #1b1a16; }
  .titles { font: 12px/1.35 ui-sans-serif; color: #444; margin-top: 8px; }
  .multi { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 12px; }
  .multi h3 { font-size: 13px; margin: 0 0 8px; }
  .bars .bar { height: 18px; border-radius: 3px; background: #e9e4d8; overflow: hidden; }
  .bars .fill { height: 100%; }
  .note { font-size: 12px; color: var(--mute); margin-top: 8px; }
</style>
<body>
  <h1>Ten ways to see the same week</h1>
  <p class="lede">${names.join(" + ")} · ${when} · ${duration}m · ${hours}. Click any cell or card — the inspector at the bottom is view 5, always on. Same data, different encodings.</p>
  <nav></nav>
  <div id="views"></div>
  <div class="inspector" id="inspector"><h3>5 · Slot inspector</h3><div id="insp"></div></div>
<script>
const D = ${JSON.stringify(data)};
const KIND = {free:"#2f9e44",tentative:"#f59f00",generic:"#e8590c",busy:"#c92a2a",oof:"#868e96",unknown:"#e9e4d8"};
const KIND_ORDER = {free:0,tentative:1,generic:2,busy:3,oof:3,unknown:4};
const HUE = {free:1,tentative:0.62,generic:0.38,busy:0.08,oof:0.08,unknown:0};
function dayName(date) {
  return new Date(date+"T12:00:00Z").toLocaleDateString("en-US",{weekday:"short",day:"numeric",month:"short",timeZone:"UTC"});
}
function byKey(date,time){ return D.grid.find(g => g.date===date && g.time===time); }
function pickNum(start){ const i = D.picks.findIndex(p => p.start===start); return i<0 ? 0 : i+1; }
const cols = \`repeat(\${D.times.length}, minmax(0,1fr))\`;

let selected = D.picks[0]?.start || D.grid.find(g=>g.bookable)?.start;

function hoursRow(){
  return \`<div class="row"><span class="who"></span><div class="cells hours" style="grid-template-columns:\${cols}">\${
    D.times.map((t,i)=> t.endsWith(":00") ? \`<span style="grid-column:\${i+1}">\${+t.slice(0,2)}</span>\` : "").join("")
  }</div></div>\`;
}
function cell(g, {mode="kind", pickMarks=true}={}) {
  if (!g) return \`<i class="c"></i>\`;
  const n = pickMarks ? pickNum(g.start) : 0;
  const freeN = g.perPerson.filter(p=>p.kind==="free").length;
  let bg, label="";
  if (mode==="density") {
    const t = freeN / Math.max(1, D.names.length);
    bg = \`rgba(47,158,68,\${0.12 + t*0.88})\`;
    label = freeN;
  } else if (mode==="hue") {
    bg = \`rgba(47,158,68,\${HUE[g.kind] ?? 0.1})\`;
  } else {
    bg = KIND[g.kind] || KIND.unknown;
  }
  return \`<button type="button" class="c\${n?" pick":""}" data-start="\${g.start}" style="background:\${bg}">\${n? \`<b>\${n}</b>\`: (mode==="density"? \`<b>\${label}</b>\`: "")}</button>\`;
}
function matrix(mode, people=true, days=D.dates, pickMarks=true) {
  return days.map(date => {
    const group = \`<div class="row"><span class="who">all \${D.names.length}</span><div class="cells" style="grid-template-columns:\${cols}">\${
      D.times.map(t => cell(byKey(date,t),{mode,pickMarks})).join("")
    }</div></div>\`;
    const rows = people ? D.names.map((name,i) => \`<div class="row"><span class="who">\${name}</span><div class="cells" style="grid-template-columns:\${cols}">\${
      D.times.map(t => {
        const g = byKey(date,t);
        const p = g?.perPerson[i];
        const n = pickMarks ? pickNum(g?.start) : 0;
        const bg = mode==="hue" ? \`rgba(47,158,68,\${HUE[p?.kind]??0.1})\` : (KIND[p?.kind]||KIND.unknown);
        return \`<button type="button" class="c\${n?" pick":""}" data-start="\${g?.start||""}" style="background:\${bg}"></button>\`;
      }).join("")
    }</div></div>\`) : "";
    return \`<div class="day-label">\${dayName(date)}</div>\${hoursRow()}\${group}\${rows}\`;
  }).join("");
}

function inspect(start){
  selected = start;
  const g = D.grid.find(x => x.start===start);
  const el = document.getElementById("insp");
  document.querySelectorAll(".card").forEach(c => c.classList.toggle("on", c.dataset.start===start));
  if (!g) { el.textContent = "Click a cell."; return; }
  const n = pickNum(g.start);
  const blockers = g.perPerson.filter(p => p.kind==="busy"||p.kind==="oof");
  el.innerHTML = \`<strong>\${dayName(g.date)} \${g.time}–\${g.end.slice(11,16)}</strong>
    · group: \${g.kind}\${n? " · pick "+n : (g.bookable? " · bookable": " · blocked")}
    <ul>\${g.perPerson.map((p,i)=>\`<li><span style="color:\${KIND[p.kind]}">●</span> <b>\${D.names[i]}</b> \${p.kind}\${p.subjects.length? " — "+p.subjects.join(", "):""}</li>\`).join("")}</ul>
    \${blockers.length? "<div>Negotiate with: "+blockers.map((p,i)=>D.names[g.perPerson.indexOf(p)]).join(", ")+"</div>":"<div>No hard conflicts.</div>"}\`;
}

function view(id, title, cite, why, body) {
  return \`<section id="\${id}"><h2>\${title}</h2><p class="why">\${why} <span class="cite">\${cite}</span></p>\${body}</section>\`;
}

const bookable = D.grid.filter(g=>g.bookable).sort((a,b)=> (KIND_ORDER[a.kind]-KIND_ORDER[b.kind]) || a.start.localeCompare(b.start));
const firstFree = D.grid.find(g=>g.kind==="free");
const firstBook = D.grid.find(g=>g.bookable);

const sections = [];

sections.push(view("v1","1 · Density heatmap","When2meet · Tufte overlay / choropleth",
  "Darker green = more people free. Names are gone on purpose. Best at 8–50 people; with two people you only get three shades.",
  D.dates.map(date => \`<div class="day-label">\${dayName(date)}</div>\${hoursRow()}<div class="row"><span class="who">free / \${D.names.length}</span><div class="cells" style="grid-template-columns:\${cols}">\${D.times.map(t=>cell(byKey(date,t),{mode:"density"})).join("")}</div></div>\`).join("")));

sections.push(view("v2","2 · Person × time matrix","Outlook Scheduling Assistant · Beard et al. 1990",
  "Row per person. This is how you see that Monday morning is Winand, not you.",
  matrix("kind")));

sections.push(view("v3","3 · Consensus strip","When2meet group chart",
  "Only the AND of everyone. Fastest yes/no. Pair with the matrix or you cannot explain a red cell.",
  matrix("kind", false)));

sections.push(view("v4","4 · Availability bars (one hue)","Faulring & Myers, CHI 2006",
  "Same matrix, but saturation of one green instead of traffic-light colors. Darker = more bookable. Red is not allowed to steal the eye.",
  matrix("hue")));

sections.push(view("v5","5 · Slot inspector","Faulring scenario 2",
  "Click anywhere. This panel is that view — constraint check for one instant. Seeded with pick 1.",
  \`<p class="note">The sticky bar at the bottom of the page is the inspector. Try Monday 15:00 vs Tuesday 16:30.</p>\`));

sections.push(view("v6","6 · Ranked cards","groupTime CHI 2006 · Doodle",
  "Decide here. Grids explore; cards commit. Top 3 from the ranker, then the next bookable times it skipped.",
  \`<div class="cards">\${bookable.slice(0,8).map((g,i)=> {
    const n = pickNum(g.start);
    return \`<button class="card" data-start="\${g.start}"><i>\${n||"·"}</i> <b>\${dayName(g.date)} \${g.time}</b><small>\${g.kind}\${n? " · official pick "+n : " · not in top 3"}</small></button>\`;
  }).join("")}</div>\`));

sections.push(view("v7","7 · Small-multiple weeks","Tufte small multiples · Faulring alternate schedules",
  "One mini week per official pick. Same axes, so alternatives sit still while your eye jumps.",
  \`<div class="multi">\${D.picks.map((p,i)=>\`<div><h3>Pick \${i+1} · \${p.label}</h3>\${
    D.dates.map(date => \`<div class="day-label">\${dayName(date)}</div><div class="row"><span class="who"></span><div class="cells" style="grid-template-columns:\${cols}">\${
      D.times.map(t => {
        const g = byKey(date,t);
        const mine = g?.start===p.start;
        return cell(g, {mode:"kind", pickMarks: mine});
      }).join("")
    }</div></div>\`).join("")
  }</div>\`).join("")}</div>\`));

const minT = new Date(D.grid[0].start).getTime();
const maxT = new Date(D.grid.at(-1).start).getTime();
const scatterDots = D.grid.map(g => {
  const x = ((new Date(g.start).getTime()-minT)/(maxT-minT))*100;
  const y = (1 - (KIND_ORDER[g.kind]??4)/3.2)*100;
  const n = pickNum(g.start);
  return \`<button class="dot\${n?" pick":""}" data-start="\${g.start}" title="\${g.date} \${g.time} \${g.kind}" style="left:\${x}%;bottom:\${Math.max(6,y)}%;background:\${KIND[g.kind]}"></button>\`;
}).join("");
sections.push(view("v8","8 · Early × quality","Faulring: evaluate options against constraints",
  "Left = earlier in the week. Top = better tier. Monday 15:00 sits left and lower (generic). Tuesday 16:30 sits righter and on the free row. That is why pick 1 is not Monday.",
  \`<div class="scatter"><span class="axis" style="left:8px;top:8px">better →</span><span class="axis" style="right:8px;bottom:8px">later →</span><span class="axis" style="left:8px;bottom:8px">free / tentative / generic / blocked</span>\${scatterDots}</div>
   <p class="note">Filled rings are official picks. Click any dot.</p>\`));

sections.push(view("v9","9 · Who to negotiate with","Palen 1999 · Faulring “with whom to negotiate”",
  "Each bookable option as a political object: empty set means take it; a name means you would be asking them to move.",
  \`<div class="blockers">\${bookable.slice(0,8).map(g=>{
    const hard = g.perPerson.filter(p=>p.kind==="busy"||p.kind==="oof");
    const soft = g.perPerson.filter(p=>p.kind==="tentative"||p.kind==="generic");
    const chips = hard.length? hard.map(p=>\`<span class="chip">\${D.names[g.perPerson.indexOf(p)]} · \${p.subjects[0]||p.kind}</span>\`).join("")
      : (soft.length? soft.map(p=>\`<span class="chip">\${D.names[g.perPerson.indexOf(p)]} · \${p.kind}\${p.subjects[0]? " · "+p.subjects[0]:""}</span>\`).join("") : \`<span class="chip">nobody</span>\`);
    return \`<article data-start="\${g.start}"><b>\${dayName(g.date)} \${g.time}</b> · \${g.kind}<div>\${chips}</div></article>\`;
  }).join("")}</div>\`));

sections.push(view("v10","10 · Focus + context","Mackinlay Time Lattice / Spiral Calendar 1994 · Tufte two-scale",
  "Skinny week on top. Click a day to explode titles. Monday autopsy without losing the week.",
  \`<div class="zoom-nav" id="zoomnav">\${D.dates.map(d=>\`<button data-date="\${d}">\${dayName(d)}</button>\`).join("")}</div>
   <div id="zoomctx"></div><div class="titles" id="zoomtitles"></div>\`));

document.getElementById("views").innerHTML = sections.join("");
document.querySelector("nav").innerHTML = [...document.querySelectorAll("section")].map(s => \`<a href="#\${s.id}">\${s.querySelector("h2").textContent}</a>\`).join("");

function renderZoom(date){
  document.querySelectorAll("#zoomnav button").forEach(b=>b.classList.toggle("on", b.dataset.date===date));
  document.getElementById("zoomctx").innerHTML = matrix("kind", true, [date]);
  const lines = D.times.map(t => {
    const g = byKey(date,t);
    const bits = (g?.perPerson||[]).filter(p=>p.subjects.length).map((p,i)=> D.names[g.perPerson.indexOf(p)]+": "+p.subjects.join(", "));
    return bits.length ? \`<div><b>\${t}</b> \${bits.join(" · ")}</div>\` : "";
  }).join("");
  document.getElementById("zoomtitles").innerHTML = lines || "<div>No titled blocks this day.</div>";
  bindClicks();
}
document.getElementById("zoomnav").addEventListener("click", e => { if (e.target.dataset.date) renderZoom(e.target.dataset.date); });
renderZoom(D.dates[0]);

function bindClicks(){
  document.querySelectorAll("[data-start]").forEach(el => {
    el.addEventListener("click", () => { if (el.dataset.start) inspect(el.dataset.start); });
  });
}
bindClicks();
inspect(selected);
</script>
</body>
</html>`;

  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, html);
}
