# Handoff — calendar slot finder (for Claude)

You are writing a **beautiful, shareable report** of a working prototype. The human (Yassin) wants something they can show: what we built, why the picks are trustworthy, and how it feels to choose among alternatives. Not a changelog. Not an engineering dump.

Audience: Yassin plus colleagues who book meetings. They know Outlook/Teams. They do not need Playwright internals.

Tone: calm, specific, a bit delighted. Show the Monday question and answer it. No AI-slop section titles (“In conclusion”, “The future of scheduling”).

---

## What this is

Throwaway prototype at `prototypes/teams-slots/` on this branch. **Do not promote into `skills/`.** Machine-local secrets stay in `personal/teams-slots-prototype/.env` (gitignored).

Question it answers: *“Find a 30-minute spot for me and Winand next week”* → top 3, with a reason you can audit.

Flow:

1. One-time **setup**: headed Playwright + 1Password service account (item `Microsoftonline` in vault `Enexis`, TOTP included) logs into Teams, caches a Graph bearer that has `Calendars.Read*`.
2. Later **find / viz**: resolve names via Graph `/me/people`, `POST /me/calendar/getSchedule`, rank, optionally write the 10-view gallery.

The LLM does **not** write a new script per query. It calls this CLI.

Cookies alone cannot call Graph. The first captured token was Exchange Online **without** calendar scopes (403). The working token is **Microsoft Teams Web Client** with `Calendars.Read`, `Calendars.Read.Shared`, `Calendars.ReadWrite`, `Calendars.ReadWrite.Shared`. Outlook also mints a useful `https://outlook.office.com` token with `Calendars.ReadWrite*`.

---

## How to refresh data

```bash
cd prototypes/teams-slots
# Copy .env.example → .env on a machine that has the 1Password service account.
# Yassin's working copy (secrets, Chrome profile) is personal/teams-slots-prototype/.
npm install
npx playwright install chromium
npm run setup                         # only if Graph cache expired
npm start -- find Winand --when next-week --duration 30
npm start -- viz Winand --when next-week --duration 30
open .cache/agendas.html
```

`--tz` defaults to the machine IANA zone (`Europe/Amsterdam` / CEST here). `--hours 9-17`, `--duration 30`, `--top 3`.

**Do not** paste `.env`, `.cache/graph.json`, or any JWT into the report.

Gallery: `.cache/agendas.html` (regenerate after `viz`). You may screenshot it or describe the ten views; do not inline secrets from the HTML if any leaked (there should be none).

---

## Ranking (the product)

Worst person wins. Real meetings and OOF are **never** overbooked.

Preference ladder (in-hours first, then the same ladder outside 09:00–17:00 / 07:00–20:00 expanded):

1. Earliest **all free**
2. Other all-free in the window
3. Over a **tentative**
4. Over a **generic** hold (Focus, Lunch, Busy, Hold, untitled busy, …)
5. Same three qualities **outside** preferred hours

Generic subjects are a keyword list (`rank.mjs` → `isGenericSubject`). “Lunch (geen meetings zonder overleg!)” is treated as a **real meeting** (title is more than “lunch”). Untitled busy blocks are generic.

This matches Palen (CHI 1999): scheduling is **satisficing**; organizers must judge the *quality* of apparent free time. Faulring & Myers (CHI 2006) treat availability as a **preference continuum**, not binary free/busy.

---

## Timezones (fixed — mention in the report)

Bug: Graph `availabilityView` is indexed from a **UTC** window. We labeled those stamps as if they were already CEST → everything **two hours early**.

Fix (`tz.mjs`): IANA zone throughout. Local midnight → UTC instant for the request; `Prefer: outlook.timezone=…`; each `scheduleItem` converted via its Windows or IANA `timeZone`. Display is formatted back in the IANA zone. Not Amsterdam-hardcoded.

After the fix, Monday titles line up with Outlook (e.g. Daily Engineering Platforms **11:00**, lunch **12:00**, Catch-up Eoin/Yassin/Winand **14:00**).

---

## Latest run (CEST, next week = Mon 24 – Fri 28 Aug 2026)

People:

- Yassin — `yassin.chibrani-derks@enexis.nl`
- Winand — `Winand.Hulleman@enexis.nl` (resolved from “Winand”)

Duration 30m, hours 09:00–17:00 Europe/Amsterdam.

### Why not Monday 24 Aug?

Every in-hours slot is a real meeting or OOF for at least one of them:

| Time | Blocker |
|---|---|
| 09:00 | Yassin: Daily Stand Up · Winand: Focus (communicatie & dagstart) |
| 09:30–10:30 | Winand: EA - SA Sync ANI & APP (Yassin joins 10:30 Wekelijks momentje Yassin/David) |
| 11:00 | Yassin: Daily Engineering Platforms |
| 11:30 | Winand: Bijpraten Guido/Winand |
| 12:00–12:30 | Winand: Lunch (geen meetings zonder overleg!) |
| 13:00–13:30 | Winand: Architectuurplaten DPO |
| 14:00–14:30 | Both: Catch-up Eoin/Yassin/Winand |
| 15:00–16:30 | Winand: Documentatie & Communicatie Epic KOROs - KIS (Yassin/Liane at 16:00) |

There is **no** in-hours Monday slot that is only generic/tentative. (Before the TZ fix we falsely offered Mon 15:00 as generic — that was 17:00 CEST, outside the window.)

### Top 3

1. **Tue 25 Aug 16:30–17:00** — both free (first all-free in hours)
2. **Wed 26 Aug 16:00–16:30** — both free
3. **Tue 25 Aug 09:30–10:00** — over Winand tentative: *Afstemming Marjelle - Winand - Theo*, *WG GRIP op Gas doelarchitectuur -- Fysiek*

First **bookable** (including tentative) is Tue 09:30. First **all free** is Tue 16:30. That distinction is the whole point of the report.

---

## Ten visualizations (same data, `.cache/agendas.html`)

Literature-backed kit. The sticky bottom bar is view 5 (inspector). Click any cell.

| # | View | Lineage | What you should feel |
|---|---|---|---|
| 1 | Density heatmap | When2meet, Tufte overlay/choropleth | Darker = more free. Thin with 2 people; sings at 10. |
| 2 | Person × time matrix | Outlook Scheduling Assistant; Beard et al. 1990 | Who blocks Monday. |
| 3 | Consensus strip | When2meet group chart | Fast yes/no, no names. |
| 4 | One-hue availability bars | Faulring & Myers CHI 2006 | Saturation = bookable. No traffic-light red stealing the eye. |
| 5 | Slot inspector | Faulring scenario 2 | One instant: who, status, titles. |
| 6 | Ranked cards | groupTime CHI 2006, Doodle | Decide here. Includes times the ranker skipped. |
| 7 | Small-multiple weeks | Tufte; Faulring alternate schedules | Three mini-weeks, one ring each. |
| 8 | Early × quality scatter | Faulring constraint evaluation | Left = earlier, top = better tier. |
| 9 | Who to negotiate | Palen; Faulring “with whom to negotiate” | Empty set = take it; a name = ask them to move. |
| 10 | Focus + context | Mackinlay Time Lattice / Spiral Calendar 1994 | Skinny week + exploded day titles. |

Recommended default for a 10-person tool: **3 + 2 + 5 + 6**. The others are for critique.

---

## What the report should contain

A single artifact (HTML or Markdown-with-figures) that a non-engineer can read in five minutes:

1. **The ask** — “30 minutes with Winand next week.”
2. **The three options** — large, dated, with one-line why. Lead with Tue 16:30.
3. **Why not Monday** — the table above, or a screenshot of view 10 / 2. This is the trust section.
4. **How we chose** — the ladder in plain language (free > tentative > generic; never over a real meeting). One sentence on Palen/Faulring if it stays light.
5. **How to see it yourself** — link/path to `agendas.html`, the ten views as a gallery with captions (not ten essays).
6. **How it scales** — 2 or 10 people is the same `getSchedule` (max 20) + worst-person-wins. Density heatmap + negotiate-with list matter more as N grows.
7. **Honest limits** — unofficial Teams-web token (Entra app + MSAL is the durable path); number-matching MFA cannot be filled; tenant can hide free/busy; generic-vs-meeting is a keyword list, not a model; token lasts ~1 hour then the Chrome profile refreshes it.

Optional appendix (small): commands, file map.

### File map

| Path | Role |
|---|---|
| `slots.mjs` | CLI: setup / find / viz |
| `rank.mjs` | ranking + `inspectGrid` |
| `tz.mjs` | IANA ↔ UTC, Graph datetimes |
| `render-gallery.mjs` | the ten views |
| `render-agendas.mjs` | older single heatmap (superseded by gallery) |
| `.cache/agendas.html` | generated report companion |
| `.cache/graph.json` | **secret** |
| `.env` | **secret** |
| `.profile/` | Playwright Chrome profile |

### Visual design for *your* report

Not the prototype’s dark heatmap by default unless you screenshot it. For the write-up: generous type, one accent, the week as a quiet grid, picks numbered. If you produce HTML, make it self-contained and printable. Screenshot `agendas.html` rather than re-implementing all ten unless you want to.

Write in English. Meeting titles may stay Dutch.

---

## Suggested title

Something like: **“Tue 16:30 — first time both of you are actually free.”**  
Subtitle: next week, 30 minutes, Yassin + Winand, CEST.

The report has succeeded if a reader can answer *why not Monday* and *why not Monday 15:00* without asking you.
