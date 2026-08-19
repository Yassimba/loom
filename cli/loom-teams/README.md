# loom-teams

Automate your Teams calendar from the terminal.

The main trick: finding a meeting slot. Instead of scrolling through the
scheduling assistant and eyeballing everyone's calendars, you type one
command and get the best times, ranked, with the reasons spelled out.

```
$ loom-teams find Winand --when next-week

Why not Monday (2026-08-24)?
  every in-hours slot is a real meeting or OOF for at least one of you:
  09:00  yassin: Daily Stand Up · winand: Focus (communicatie & dagstart)
  ...

1. Tue 25 Aug 09:30–10:00  (score +32, tentative)
   in hours — Winand: tentative (WG GRIP op Gas doelarchitectuur)
   +30  circadian: mid-morning attention peak
   +20  weekday: high-attendance day
   -40  conflicts: Winand: tentative
```

It's a single Rust binary, no server. You log into Teams once yourself,
in a real Chrome window — your own password, your own MFA. The tool
keeps the Microsoft Graph token cached, so after that first login, every
question is just one API call.

## What it does

| Command | What you get |
| --- | --- |
| `setup` | The one-time login. Opens Chrome, waits while you sign in, and caches a Graph token. |
| `status` | Who is signed in, which scopes you have, and how long the token is still valid. |
| `export [people…]` | Calendar data as JSON: every event in the window, plus each person's free/busy times. Useful for scripts, reports, or feeding to an LLM. |
| `find <people…>` | The best meeting slots for you and the people you name. Every suggestion comes with the reasons behind it. |

You don't need anyone's email address. `find Winand` is enough, because
the tool looks names up through Graph's people search. To pick a time
window, use `--when next-week`, `--when 5` (the next 5 days), or exact
dates like `--from 2026-08-17 --to 2026-08-21`.

## How `find` picks a slot

Every weekday slot gets a score, and the ten highest win. Nothing is
vetoed: with eleven people someone is always busy, and a hard rule would
just answer "no slots". Instead, each person's conflict costs points,
priced by how awkward the conversation would be:

| Their calendar says | Cost per person |
| --- | --- |
| free | 0 |
| tentative | −40 |
| a soft hold, like "Focus", "Lunch", or an untitled block | −60 |
| free/busy we can't read | −80 |
| a real meeting | −120 |
| out of office | −150 |

So a slot over someone's vacation can still come out on top in a
desperate week — but only when nothing cheaper exists, and the output
always names who it costs ("kasper: out of office").

How does it know a "real meeting" from a "soft hold"? By keywords in the
title. "Lunch" is a hold you can book over. "Lunch (geen meetings zonder
overleg!)" is treated as a real meeting. If the keywords get one wrong,
you can correct individual events with `--classifications` (more on that
below).

On top of the conflict costs, each slot collects points from research
about meetings (plus −50 when it falls outside your preferred hours).
Every point appears in the output, so you can always see why a slot won:

| Factor | Points | Why |
| --- | --- | --- |
| circadian | +30 mid-morning · −35 lunch · −25 post-lunch dip · −15 end of day | People focus best late in the morning and sag after lunch (Monk 2005). And lunch stays lunch, even when the calendar says free. |
| weekday | +20 Tue/Wed/Thu · −15 Mon · −25 Fri | Tuesday meetings get the best attendance; most Friday meetings never happen (YouCanBookMe, based on 2M invite responses). |
| fatigue | −20 per person | Someone's fourth back-to-back half hour builds stress that a break would have reset (Microsoft EEG study, 2021). |
| defrag | +15 per person | A slot right next to an existing meeting, or at the start or end of the day, leaves focus time in one piece. |
| fragmentation | −25 per person | A slot in the middle of a big free block cuts it into two halves, each too short for deep work (Gloria Mark: it takes ~23 minutes to refocus). |
| soon | −8 per day | Earlier in the week is easier to plan around. |

The JSON grid carries the same score for every slot — ready-made heatmap
data, highest to lowest. One nice effect: on an empty calendar, the tool
suggests Tuesday morning instead of Monday at 09:00, because that's what
the attendance data says works best.

## The JSON pipeline

`export` (schema `loom-teams/export/1`) is the foundation, and `find` is
one tool built on top of it. Every event in the export has a stable id.
That means you can have an LLM classify events once, for example marking
an event as a placeholder rather than a real meeting:

```json
{ "e0fdc5761c4": "generic" }
```

Then `find --classifications verdicts.json` uses those verdicts instead
of the keyword guesses. Other analyses, like meeting-time reports or
category breakdowns, follow the same pattern: work from the JSON, never
call Graph again.

## Install

One line, no toolchain needed. macOS or Linux:

```bash
curl -LsSf https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.sh | sh
```

Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Yassimba/loom/main/cli/loom-teams/install.ps1 | iex"
```

Both download the release binary pinned by the published manifest, verify
its checksum, and install to `~/.local/bin`. Already using the loom
setup? `loom-teams` is in its tool list — pick it in the wizard and mise
manages the pin for you. Or build from source with `cargo build
--release`.

## Setup

```bash
loom-teams setup   # a Chrome window opens; sign in like you always do
```

The Graph token lives in your system's credential store — macOS
Keychain, Windows Credential Manager, or the Linux secret service —
under `loom-teams`. The only thing on disk is the Chrome profile, in
`~/Library/Application Support/loom-teams` on macOS (or the platform
equivalent). It holds
your Teams session cookies, so treat it as secret; none of it is tracked
in git.

**About the Keychain popup.** The first time the tool reads the token,
macOS asks: *"loom-teams wants to use your confidential information
stored in the keychain."* Click **Always Allow**. If you click "Allow"
instead, macOS grants access once and asks again on every run — same
dialog, forever. One more quirk: the permission is tied to the exact
binary, so after you rebuild (`cargo build --release`), the dialog comes
back once. That's macOS being careful, not the tool misbehaving.

## Honest limits

- The Graph token comes from the Teams web client, not from a registered
  Entra app. That works fine: the token lasts about an hour, and the
  Chrome profile refreshes it in the background. But a proper app
  registration with MSAL would be the more durable setup.
- Some tenants hide free/busy details. People affected by that show up
  as "unknown" and count as a heavy conflict, so slots over them sink in
  the ranking.
- Graph's `getSchedule` call handles at most 20 people at a time.
