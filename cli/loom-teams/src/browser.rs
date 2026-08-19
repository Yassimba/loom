//! Chrome-driven token acquisition over CDP.
//!
//! Open the Teams calendar in a persistent Chrome profile. On first run
//! (`setup`) the window is visible and the user signs in themselves — MFA,
//! SSO, whatever their tenant wants; we only wait. After that the profile's
//! cookies refresh the session headlessly. Either way we capture a Graph
//! bearer with Calendars.* scopes from (a) Authorization headers on outgoing
//! requests, (b) oauth token endpoint responses, and (c) the MSAL cache in
//! local/session storage. Falls back to the Outlook calendar when the Teams
//! token lacks calendar scopes.

use crate::token::{app_name, calendar_scopes, decode_jwt, exp_epoch, is_graph_aud};
use anyhow::{anyhow, bail, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, EventRequestWillBeSentExtraInfo, EventResponseReceived, GetResponseBodyParams,
};
use chromiumoxide::Page;
use futures::StreamExt;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TEAMS_CALENDAR: &str = "https://teams.microsoft.com/v2/#/calendar";
const OUTLOOK_CALENDAR: &str = "https://outlook.office.com/calendar/view/week";
const LOGIN_HOSTS: [&str; 2] = ["login.microsoftonline", "login.microsoft.com"];

#[derive(Default)]
struct Bag {
    best: Option<(String, Value)>,
    seen: Vec<String>,
}

impl Bag {
    fn remember(&mut self, token: &str) {
        let Some(claims) = decode_jwt(token) else {
            return;
        };
        let aud = match &claims["aud"] {
            Value::Array(a) => a
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(","),
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        let cals = calendar_scopes(&claims);
        let fingerprint = format!("{aud}|{}", cals.join(" "));
        if !self.seen.contains(&fingerprint) {
            self.seen.push(fingerprint);
            eprintln!(
                "token aud={aud} app={} calendars={}",
                app_name(&claims),
                if cals.is_empty() {
                    "no".into()
                } else {
                    cals.join(" ")
                }
            );
        }
        if !is_graph_aud(&claims) {
            return;
        }
        let prev_cals = self
            .best
            .as_ref()
            .map(|(_, c)| calendar_scopes(c).len())
            .unwrap_or(0);
        let prev_exp = self
            .best
            .as_ref()
            .and_then(|(_, c)| exp_epoch(c))
            .unwrap_or(0);
        let better = cals.len() > prev_cals
            || (cals.len() == prev_cals && exp_epoch(&claims).unwrap_or(0) >= prev_exp);
        if self.best.is_none() || better {
            self.best = Some((token.to_string(), claims));
        }
    }

    fn has_calendar_token(&self) -> bool {
        self.best
            .as_ref()
            .map(|(_, c)| !calendar_scopes(c).is_empty())
            .unwrap_or(false)
    }
}

/// Prefer an installed Chrome; fall back to chromiumoxide's own detection.
fn chrome_executable() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        )]
    } else if cfg!(target_os = "windows") {
        ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|base| PathBuf::from(base).join("Google/Chrome/Application/chrome.exe"))
            .collect()
    } else {
        [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    };
    candidates.into_iter().find(|p| p.exists())
}

async fn launch(profile: &Path, headed: bool) -> Result<Browser> {
    let mut builder = BrowserConfig::builder()
        .user_data_dir(profile)
        .window_size(1400, 900)
        .viewport(None);
    if headed {
        builder = builder.with_head();
    }
    if let Some(exe) = chrome_executable() {
        builder = builder.chrome_executable(exe);
    }
    let config = builder
        .build()
        .map_err(|e| anyhow!("browser config: {e}"))?;
    let (browser, mut handler) = Browser::launch(config).await.context("launching Chrome")?;
    tokio::task::spawn(async move { while handler.next().await.is_some() {} });
    Ok(browser)
}

/// Attach network sniffers that feed every observed bearer into the bag.
async fn capture_tokens(page: &Page, bag: Arc<Mutex<Bag>>) -> Result<()> {
    page.execute(EnableParams::default()).await.ok();

    let mut headers = page
        .event_listener::<EventRequestWillBeSentExtraInfo>()
        .await
        .context("subscribing to request events")?;
    let header_bag = bag.clone();
    tokio::task::spawn(async move {
        while let Some(event) = headers.next().await {
            let inner = event.headers.inner();
            let auth = inner["authorization"]
                .as_str()
                .or(inner["Authorization"].as_str());
            if let Some(value) = auth {
                if let Some(token) = value
                    .strip_prefix("Bearer ")
                    .or(value.strip_prefix("bearer "))
                {
                    header_bag.lock().unwrap().remember(token.trim());
                }
            }
        }
    });

    let mut responses = page
        .event_listener::<EventResponseReceived>()
        .await
        .context("subscribing to response events")?;
    let response_page = page.clone();
    let response_bag = bag;
    tokio::task::spawn(async move {
        while let Some(event) = responses.next().await {
            let url = &event.response.url;
            if !url.contains("/oauth2/v2.0/token") && !url.contains("/oauth2/token") {
                continue;
            }
            let body = response_page
                .execute(GetResponseBodyParams::new(event.request_id.clone()))
                .await;
            if let Ok(body) = body {
                if let Ok(json) = serde_json::from_str::<Value>(&body.result.body) {
                    if let Some(token) = json["access_token"].as_str() {
                        response_bag.lock().unwrap().remember(token);
                    }
                }
            }
        }
    });
    Ok(())
}

/// Pull every JWT-looking secret out of the page's MSAL local/session storage.
async fn harvest_msal(page: &Page, bag: &Arc<Mutex<Bag>>) {
    const SCRIPT: &str = r#"
    (() => {
      const out = new Set();
      const jwtRe = /eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g;
      for (const store of [localStorage, sessionStorage]) {
        for (let i = 0; i < store.length; i++) {
          const val = store.getItem(store.key(i));
          if (!val || !val.includes('eyJ')) continue;
          for (const m of val.match(jwtRe) ?? []) out.add(m);
          try {
            const parsed = JSON.parse(val);
            const secret = parsed.secret || parsed.accessToken || parsed.access_token;
            if (typeof secret === 'string' && secret.startsWith('eyJ')) out.add(secret);
          } catch {}
        }
      }
      return Array.from(out);
    })()
    "#;
    let tokens: Vec<String> = page
        .evaluate(SCRIPT)
        .await
        .ok()
        .and_then(|r| r.into_value().ok())
        .unwrap_or_default();
    let mut bag = bag.lock().unwrap();
    for token in tokens {
        bag.remember(&token);
    }
}

async fn current_url(page: &Page) -> String {
    page.url().await.ok().flatten().unwrap_or_default()
}

fn on_login_page(url: &str) -> bool {
    LOGIN_HOSTS.iter().any(|h| url.contains(h))
}

/// The user signs in themselves in the visible Chrome window; we just wait
/// (up to 5 minutes) until the login pages hand back to the calendar.
async fn wait_for_manual_login(page: &Page, headed: bool) -> Result<()> {
    if !headed {
        bail!(
            "Session expired and this is a headless run. Run: loom-teams setup (and sign in \
             in the Chrome window that opens)."
        );
    }
    eprintln!("Sign in to Microsoft in the Chrome window (MFA and all); waiting up to 5 min…");
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut last_url = String::new();
    while Instant::now() < deadline {
        let url = current_url(page).await;
        if url != last_url {
            last_url = url.clone();
            eprintln!("login page: {url}");
        }
        if !url.is_empty() && !on_login_page(&url) {
            eprintln!("signed in");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    bail!(
        "Login not completed within 5 minutes (last URL {})",
        current_url(page).await
    )
}

async fn wait_for_calendar_token(bag: &Arc<Mutex<Bag>>, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        if bag.lock().unwrap().has_calendar_token() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

async fn goto_and_login(page: &Page, url: &str, headed: bool) -> Result<()> {
    page.goto(url).await.context("navigation failed")?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    if on_login_page(&current_url(page).await) {
        wait_for_manual_login(page, headed).await?;
        page.goto(url).await.context("navigation failed")?;
    }
    Ok(())
}

/// Returns a Graph bearer with Calendars.* scopes.
pub async fn acquire(profile: &Path, headed: bool) -> Result<String> {
    let browser = launch(profile, headed).await?;
    let result = acquire_inner(&browser, headed).await;
    // Best-effort teardown; the CDP connection dying also kills our handler task.
    let mut browser = browser;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    result
}

async fn acquire_inner(browser: &Browser, headed: bool) -> Result<String> {
    let bag = Arc::new(Mutex::new(Bag::default()));
    let pages = browser.pages().await.unwrap_or_default();
    let page = match pages.into_iter().next() {
        Some(p) => p,
        None => browser
            .new_page("about:blank")
            .await
            .context("opening page")?,
    };
    capture_tokens(&page, bag.clone()).await?;

    goto_and_login(&page, TEAMS_CALENDAR, headed).await?;
    wait_for_calendar_token(&bag, 12_000).await;
    harvest_msal(&page, &bag).await;

    if !bag.lock().unwrap().has_calendar_token() {
        eprintln!("Teams token has no Calendars.* scopes; opening Outlook calendar…");
        goto_and_login(&page, OUTLOOK_CALENDAR, headed).await?;
        wait_for_calendar_token(&bag, 20_000).await;
        harvest_msal(&page, &bag).await;
    }

    let bag = bag.lock().unwrap();
    if !bag.has_calendar_token() {
        let seen = bag.seen.join("; ");
        bail!(
            "No Graph bearer with Calendars.* scopes. Seen: {}",
            if seen.is_empty() { "nothing" } else { &seen }
        );
    }
    Ok(bag.best.clone().expect("checked above").0)
}
