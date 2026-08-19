#!/usr/bin/env node
// PROTOTYPE — wipe me.
// Setup once with 1Password + Playwright. After that, find is one Graph call.

import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { inspectGrid, nextWeekMonday, rankSlots } from "./rank.mjs";
import { writeGalleryHtml } from "./render-gallery.mjs";
import { addDaysYmd, systemTimeZone, toGraphUtc, zonedLocalToUtc } from "./tz.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const PROFILE = join(HERE, ".profile");
const CACHE = join(HERE, ".cache", "graph.json");
const ENV_FILE = join(HERE, ".env");
const TEAMS_CALENDAR = "https://teams.microsoft.com/v2/#/calendar";
const OUTLOOK_CALENDAR = "https://outlook.office.com/calendar/view/week";
const GRAPH = "https://graph.microsoft.com/v1.0";
const GRAPH_AUDS = new Set([
  "https://graph.microsoft.com",
  "graph.microsoft.com",
  "00000003-0000-0000-c000-000000000000",
]);

loadDotEnv(ENV_FILE);

const FLAGS_WITH_VALUE = new Set([
  "--when",
  "--duration",
  "--minutes",
  "--interval",
  "--tz",
  "--top",
  "--hours",
]);

const argv = process.argv.slice(2);
const command = ["setup", "find", "viz"].includes(argv[0]) ? argv.shift() : inferCommand(argv);
const flags = parseFlags(argv);
const who = positionals(argv);

function inferCommand(args) {
  return args.includes("--setup") || args.includes("--login") ? "setup" : "find";
}

function positionals(args) {
  const out = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith("--")) {
      if (FLAGS_WITH_VALUE.has(args[i])) i += 1;
      continue;
    }
    out.push(args[i]);
  }
  return out;
}

function parseFlags(args) {
  const val = (name, fallback) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : fallback;
  };
  const hours = val("--hours", "9-17").split("-").map(Number);
  return {
    when: val("--when", "next-week"),
    duration: Number(val("--duration", "30")),
    interval: Number(val("--minutes", val("--interval", "30"))),
    tz: val("--tz", systemTimeZone()),
    top: Number(val("--top", "3")),
    startHour: hours[0] ?? 9,
    endHour: hours[1] ?? 17,
    headed: args.includes("--headed") || args.includes("--setup") || args.includes("--login"),
    json: args.includes("--json"),
    html: args.includes("--html"),
    forceLogin: args.includes("--setup") || args.includes("--login") || command === "setup",
  };
}

function loadDotEnv(path) {
  if (!existsSync(path)) return;
  for (const line of readFileSync(path, "utf8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq < 0) continue;
    const key = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim().replace(/^['"]|['"]$/g, "");
    if (!(key in process.env)) process.env[key] = value;
  }
}

function flagValueMissing() {
  return !process.env.OP_SERVICE_ACCOUNT_TOKEN && !process.env.OP_CONNECT_TOKEN;
}

function op(args) {
  const result = spawnSync("op", args, {
    encoding: "utf8",
    env: process.env,
  });
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || "op failed").trim());
  }
  return result.stdout.trim();
}

function opVaultArgs() {
  const vault = process.env.OP_TEAMS_VAULT;
  return vault ? ["--vault", vault] : [];
}

function readTeamsItem() {
  const item = process.env.OP_TEAMS_ITEM;
  if (!item) {
    throw new Error(
      "Set OP_TEAMS_ITEM to the 1Password item name/id (and OP_TEAMS_VAULT if the service account sees more than one vault).",
    );
  }
  const raw = op(["item", "get", item, ...opVaultArgs(), "--format=json", "--reveal"]);
  const parsed = JSON.parse(raw);
  const field = (id) =>
    parsed.fields?.find((f) => f.id === id || f.label?.toLowerCase() === id)?.value;
  const username = field("username") || field("email");
  const password = field("password");
  if (!username || !password) {
    throw new Error("1Password item is missing username/password.");
  }
  return { username, password };
}

function readTotp() {
  const item = process.env.OP_TEAMS_ITEM;
  return op(["item", "get", item, ...opVaultArgs(), "--otp"]);
}

function decodeJwt(token) {
  try {
    return JSON.parse(Buffer.from(token.split(".")[1], "base64url").toString());
  } catch {
    return null;
  }
}

function isGraphAud(aud) {
  const list = Array.isArray(aud) ? aud : [aud];
  return list.some((a) => a && (GRAPH_AUDS.has(a) || String(a).includes("graph.microsoft.com")));
}

function calendarScopes(claims) {
  return String(claims?.scp ?? "")
    .split(" ")
    .filter((s) => /^Calendars\./i.test(s));
}

function readCache() {
  if (!existsSync(CACHE)) return null;
  try {
    const cached = JSON.parse(readFileSync(CACHE, "utf8"));
    const claims = decodeJwt(cached.token);
    if (!claims?.exp || claims.exp * 1000 < Date.now() + 120_000) return null;
    if (!isGraphAud(claims.aud)) return null;
    if (!calendarScopes(claims).length) return null;
    return { token: cached.token, claims };
  } catch {
    return null;
  }
}

function writeCache(token, claims) {
  mkdirSync(dirname(CACHE), { recursive: true });
  writeFileSync(CACHE, JSON.stringify({ token, exp: claims.exp }, null, 2));
  chmodSync(CACHE, 0o600);
}

async function graphFetch(token, path, body, extraHeaders = {}) {
  const res = await fetch(`${GRAPH}${path}`, {
    method: body ? "POST" : "GET",
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${token}`,
      ...(body ? { "Content-Type": "application/json" } : {}),
      ...extraHeaders,
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await res.json().catch(() => ({}));
  return { status: res.status, json };
}

function visible(locator, timeout = 800) {
  return locator.first().isVisible({ timeout }).catch(() => false);
}

async function clickFirst(page, selectors) {
  for (const sel of selectors) {
    const loc = page.locator(sel);
    if (await visible(loc, 400)) {
      await loc.first().click();
      return true;
    }
  }
  return false;
}

async function fillIf(page, selectors, value) {
  for (const sel of selectors) {
    const loc = page.locator(sel);
    if (await visible(loc, 400)) {
      await loc.first().fill(value);
      return true;
    }
  }
  return false;
}

async function signIn(page) {
  console.error("1Password item loaded; filling Microsoft login…");
  const creds = readTeamsItem();
  const deadline = Date.now() + 120_000;
  let emailDone = false;
  let passDone = false;
  let otpDone = false;
  let lastUrl = "";

  while (Date.now() < deadline) {
    const url = page.url();
    if (url !== lastUrl) {
      lastUrl = url;
      console.error(`login page: ${url}`);
    }
    if (/teams\.microsoft\.com/.test(url) && !/login\.microsoftonline/.test(url)) {
      console.error("reached Teams");
      return;
    }

    if (await visible(page.getByText(/approve (the )?sign[- ]?in request|check your.*authenticator/i), 300)) {
      await clickFirst(page, [
        "text=/Use a (different|another) (verification )?(option|method)/i",
        "text=/I can.t use my .*app/i",
        "text=/Other ways to sign in/i",
        "#signInAnotherWay",
      ]);
      await clickFirst(page, [
        '[data-value="PhoneAppOTP"]',
        "text=/verification code/i",
        "text=/Use a verification code/i",
      ]);
    }

    if (!emailDone && (await fillIf(page, ['input[name="loginfmt"]', 'input[type="email"]'], creds.username))) {
      console.error("filled email");
      await clickFirst(page, ["#idSIButton9", 'input[type="submit"]']);
      emailDone = true;
      continue;
    }

    if (!passDone && (await fillIf(page, ['input[name="passwd"]', 'input[type="password"]'], creds.password))) {
      console.error("filled password");
      await clickFirst(page, ["#idSIButton9", 'input[type="submit"]']);
      passDone = true;
      continue;
    }

    if (
      !otpDone &&
      (await visible(page.locator("#idTxtBx_SAOTCC_OTC, input[name='otc']"), 400))
    ) {
      console.error("filling TOTP");
      const code = readTotp();
      await fillIf(page, ["#idTxtBx_SAOTCC_OTC", "input[name='otc']"], code);
      await clickFirst(page, ["#idSubmit_SAOTCC_Continue", "#idSIButton9", 'input[type="submit"]']);
      otpDone = true;
      continue;
    }

    if (await visible(page.locator("#idSIButton9"), 300)) {
      const label = await page.locator("#idSIButton9").getAttribute("value");
      if (label && /yes|next|sign in/i.test(label)) {
        await page.locator("#idSIButton9").click();
        continue;
      }
    }

    await new Promise((r) => setTimeout(r, 400));
  }
  const shot = join(HERE, ".cache", "login-timeout.png");
  mkdirSync(dirname(shot), { recursive: true });
  await page.screenshot({ path: shot, fullPage: true }).catch(() => {});
  throw new Error(`Teams login did not finish (last URL ${page.url()}). Screenshot: ${shot}`);
}

async function waitForCalendarToken(bag, ms) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    if (calendarScopes(bag.graph?.claims ?? {}).length) return;
    await new Promise((r) => setTimeout(r, 400));
  }
}

function rememberToken(bag, token) {
  const claims = decodeJwt(token);
  if (!claims) return;
  const aud = Array.isArray(claims.aud) ? claims.aud.join(",") : String(claims.aud ?? "");
  const cals = calendarScopes(claims);
  bag.seen ??= [];
  if (!bag.seen.some((s) => s.aud === aud && s.cals === cals.join(" "))) {
    bag.seen.push({ aud, cals: cals.join(" ") || "(none)", app: claims.app_displayname });
    console.error(`token aud=${aud} app=${claims.app_displayname ?? "?"} calendars=${cals.join(" ") || "no"}`);
  }
  if (!isGraphAud(claims.aud)) return;
  const prev = bag.graph;
  const prevCals = prev ? calendarScopes(prev.claims).length : 0;
  if (!prev || cals.length > prevCals || (cals.length === prevCals && (claims.exp ?? 0) >= (prev.claims.exp ?? 0))) {
    bag.graph = { token, claims };
  }
}

function captureTokens(page, bag) {
  page.on("request", (req) => {
    const header = req.headers().authorization;
    if (header?.toLowerCase().startsWith("bearer ")) rememberToken(bag, header.slice(7).trim());
  });
  page.on("response", async (res) => {
    if (!/\/oauth2\/v2\.0\/token|\/oauth2\/token/.test(res.url())) return;
    try {
      const json = await res.json();
      if (json.access_token) rememberToken(bag, json.access_token);
    } catch {
      /* not json */
    }
  });
}

async function harvestMsal(page, bag) {
  const blobs = await page.evaluate(() => {
    const out = [];
    for (const store of [localStorage, sessionStorage]) {
      for (let i = 0; i < store.length; i++) {
        const key = store.key(i);
        const val = store.getItem(key);
        if (val && val.includes("eyJ")) out.push(val);
      }
    }
    return out;
  }).catch(() => []);
  for (const val of blobs) {
    const jwt = val.match(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g) ?? [];
    for (const token of jwt) rememberToken(bag, token);
    try {
      const parsed = JSON.parse(val);
      const secret = parsed.secret || parsed.accessToken || parsed.access_token;
      if (typeof secret === "string" && secret.startsWith("eyJ")) rememberToken(bag, secret);
    } catch {
      /* not json */
    }
  }
}

async function launch(headed) {
  const opts = {
    headless: !headed,
    viewport: { width: 1400, height: 900 },
    ignoreHTTPSErrors: true,
  };
  try {
    return await chromium.launchPersistentContext(PROFILE, { ...opts, channel: "chrome" });
  } catch {
    return await chromium.launchPersistentContext(PROFILE, opts);
  }
}

async function ensureGraphToken() {
  if (!flags.forceLogin) {
    const cached = readCache();
    if (cached) {
      const probe = await graphFetch(cached.token, "/me");
      if (probe.status === 200) return { ...cached, me: probe.json };
    }
  }

  if (flagValueMissing() && !existsSync(PROFILE)) {
    throw new Error(
      "No cached Teams session. Export OP_SERVICE_ACCOUNT_TOKEN (and OP_TEAMS_ITEM / OP_TEAMS_VAULT), then run: npm run setup",
    );
  }

  const bag = {};
  const context = await launch(flags.headed || flags.forceLogin);
  try {
    for (const page of context.pages()) captureTokens(page, bag);
    context.on("page", (page) => captureTokens(page, bag));
    const page = context.pages()[0] ?? (await context.newPage());
    await page.goto(TEAMS_CALENDAR, { waitUntil: "domcontentloaded" });
    if (/login\.microsoftonline|login\.microsoft\.com/.test(page.url())) {
      await signIn(page);
    }
    await page.goto(TEAMS_CALENDAR, { waitUntil: "domcontentloaded" });
    await waitForCalendarToken(bag, 12_000);
    await harvestMsal(page, bag);
    if (!calendarScopes(bag.graph?.claims ?? {}).length) {
      console.error("Teams token has no Calendars.* scopes; opening Outlook calendar…");
      await page.goto(OUTLOOK_CALENDAR, { waitUntil: "domcontentloaded" });
      if (/login\.microsoftonline|login\.microsoft\.com/.test(page.url())) {
        await signIn(page);
        await page.goto(OUTLOOK_CALENDAR, { waitUntil: "domcontentloaded" });
      }
      await waitForCalendarToken(bag, 20_000);
      await harvestMsal(page, bag);
    }
    if (!calendarScopes(bag.graph?.claims ?? {}).length) {
      const seen = (bag.seen ?? []).map((s) => `${s.app} ${s.aud} cal=${s.cals}`).join("; ");
      throw new Error(`No Graph bearer with Calendars.* scopes. Seen: ${seen || "nothing"}`);
    }
    const me = await graphFetch(bag.graph.token, "/me");
    if (me.status !== 200) {
      throw new Error(`Graph /me failed: ${me.status} ${me.json.error?.message ?? ""}`);
    }
    writeCache(bag.graph.token, bag.graph.claims);
    return { ...bag.graph, me: me.json };
  } finally {
    await context.close();
  }
}

async function resolvePeople(token, me, names) {
  const self = me.mail || me.userPrincipalName;
  const resolved = [{ query: "me", email: self, name: me.displayName }];
  for (const name of names) {
    if (name.includes("@")) {
      resolved.push({ query: name, email: name, name });
      continue;
    }
    const q = encodeURIComponent(`"${name}"`);
    const res = await graphFetch(
      token,
      `/me/people?$search=${q}&$select=displayName,scoredEmailAddresses,userPrincipalName`,
      undefined,
      { ConsistencyLevel: "eventual" },
    );
    const hit = res.json.value?.[0];
    const email =
      hit?.scoredEmailAddresses?.[0]?.address || hit?.userPrincipalName;
    if (!email) {
      throw new Error(`Could not resolve "${name}" via Graph people search. Pass an email.`);
    }
    resolved.push({ query: name, email, name: hit.displayName || name });
  }
  return resolved;
}

function searchWindow(tz, when) {
  let startYmd;
  let endYmd;
  if (when === "next-week") {
    startYmd = nextWeekMonday(tz);
    endYmd = addDaysYmd(startYmd, 5);
  } else {
    const days = Number(when) || 5;
    const today = new Intl.DateTimeFormat("en-CA", {
      timeZone: tz,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(new Date());
    startYmd = addDaysYmd(today, 1);
    endYmd = addDaysYmd(startYmd, days);
  }
  const start = zonedLocalToUtc(`${startYmd}T00:00:00`, tz);
  const end = zonedLocalToUtc(`${endYmd}T00:00:00`, tz);
  return {
    start,
    end,
    startTime: toGraphUtc(start),
    endTime: toGraphUtc(end),
    tz,
  };
}

function usage() {
  console.error(`Usage:
  npm run setup                          # 1Password + Playwright login, cache Graph token
  npm start -- find Winand --when next-week --duration 30
  npm start -- find a@x.com b@x.com --hours 9-17 --top 3 --json
`);
}

async function main() {
  if ((command === "find" || command === "viz") && who.length === 0) {
    usage();
    process.exit(2);
  }

  const session = await ensureGraphToken();
  if (command === "setup") {
    const out = {
      ok: true,
      me: session.me.userPrincipalName,
      exp: new Date(session.claims.exp * 1000).toISOString(),
    };
    console.log(flags.json ? JSON.stringify(out, null, 2) : `Signed in as ${out.me} (token until ${out.exp})`);
    return;
  }

  const people = await resolvePeople(session.token, session.me, who);
  const window = searchWindow(flags.tz, flags.when);
  const schedule = await graphFetch(
    session.token,
    "/me/calendar/getSchedule",
    {
      schedules: people.map((p) => p.email),
      startTime: window.startTime,
      endTime: window.endTime,
      availabilityViewInterval: flags.interval,
    },
    { Prefer: `outlook.timezone="${flags.tz}"` },
  );
  if (schedule.status !== 200) {
    throw new Error(`getSchedule ${schedule.status}: ${schedule.json.error?.message ?? "unknown"}`);
  }

  const slots = rankSlots({
    schedules: schedule.json.value ?? [],
    windowStartMs: window.start.getTime(),
    tz: flags.tz,
    interval: flags.interval,
    duration: flags.duration,
    preferred: { startHour: flags.startHour, endHour: flags.endHour },
    top: flags.top,
  });

  const result = {
    me: session.me.userPrincipalName,
    people,
    window,
    duration: flags.duration,
    hours: `${flags.startHour}-${flags.endHour}`,
    options: slots,
  };

  const grid = inspectGrid({
    schedules: schedule.json.value ?? [],
    windowStartMs: window.start.getTime(),
    tz: flags.tz,
    interval: flags.interval,
    duration: flags.duration,
    preferred: { startHour: flags.startHour, endHour: flags.endHour },
  });

  if (command === "viz" || flags.html) {
    const htmlPath = join(HERE, ".cache", "agendas.html");
    writeGalleryHtml({
      path: htmlPath,
      people,
      grid,
      picks: slots,
      hours: result.hours,
      when: flags.when,
      duration: flags.duration,
    });
    console.error(`wrote ${htmlPath}`);
  }

  if (flags.json) {
    console.log(JSON.stringify({ ...result, grid }, null, 2));
    return;
  }

  const monday = grid[0]?.date;
  const mondayRows = grid.filter((g) => g.date === monday);
  const mondayOpen = mondayRows.filter((g) => g.bookable);
  console.log(`Why not Monday (${monday})?`);
  if (!mondayRows.length) {
    console.log("  no Monday rows in the window");
  } else if (mondayOpen.length) {
    console.log(`  it is bookable from ${mondayOpen[0].time} (${mondayOpen[0].kind})`);
  } else {
    console.log("  every in-hours slot is a real meeting or OOF for at least one of you:");
    for (const g of mondayRows) {
      const whoBusy = g.perPerson
        .filter((p) => p.kind === "busy" || p.kind === "oof")
        .map((p) => `${p.who.split("@")[0]}: ${p.subjects.join(", ") || p.kind}`)
        .join(" · ");
      console.log(`  ${g.time}  ${whoBusy}`);
    }
  }
  const first = grid.find((g) => g.bookable);
  if (first) {
    console.log(`\nFirst bookable in hours: ${first.date} ${first.time} (${first.kind})`);
  }

  if (!slots.length) {
    console.log("\nNo bookable slots (everyone has a real meeting or OOF in every candidate).");
    return;
  }
  console.log(`\nTop ${slots.length} for ${people.map((p) => p.name || p.email).join(", ")} (${flags.when}, ${flags.duration}m):\n`);
  for (const [i, slot] of slots.entries()) {
    console.log(`${i + 1}. ${slot.label}`);
    console.log(`   ${slot.reason}`);
  }
}

main().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
