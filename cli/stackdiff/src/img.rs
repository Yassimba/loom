//! Inline terminal images: pipe the mermaid source through `mmdr`
//! (mermaid-rs-renderer) for layout + rasterization, then paint the PNG
//! into the terminal as truecolor half-blocks — works in any truecolor
//! terminal, no graphics protocol required.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use base64::Engine;
use image::GenericImageView;

/// How this terminal can show pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// kitty graphics protocol: kitty, Ghostty, WezTerm
    Kitty,
    /// iTerm2 inline images (OSC 1337)
    Iterm,
    /// Truecolor half-blocks — any terminal
    Blocks,
}

/// Detect the best protocol from the environment.
pub fn detect_protocol() -> Protocol {
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if std::env::var("KITTY_WINDOW_ID").is_ok()
        || term.contains("kitty")
        || term.contains("ghostty")
        || program == "ghostty"
        || program == "WezTerm"
    {
        Protocol::Kitty
    } else if program == "iTerm.app" {
        Protocol::Iterm
    } else {
        Protocol::Blocks
    }
}

/// Encode PNG bytes for the detected protocol.
pub fn png_to_terminal(png: &[u8], protocol: Protocol) -> Result<String> {
    match protocol {
        Protocol::Kitty => Ok(png_to_kitty(png)),
        Protocol::Iterm => Ok(png_to_iterm(png)),
        Protocol::Blocks => png_to_half_blocks(png),
    }
}

/// kitty graphics protocol: transmit-and-display, chunked base64.
fn png_to_kitty(png: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    let mut out = String::new();
    let chunks: Vec<&str> = data
        .as_bytes()
        .chunks(4096)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect();
    for (index, chunk) in chunks.iter().enumerate() {
        let more = if index + 1 < chunks.len() { 1 } else { 0 };
        if index == 0 {
            out.push_str(&format!("\x1b_Gf=100,a=T,m={more};{chunk}\x1b\\"));
        } else {
            out.push_str(&format!("\x1b_Gm={more};{chunk}\x1b\\"));
        }
    }
    out.push('\n');
    out
}

/// iTerm2 OSC 1337 inline image.
fn png_to_iterm(png: &[u8]) -> String {
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    format!(
        "\x1b]1337;File=inline=1;size={};preserveAspectRatio=1:{}\x07\n",
        png.len(),
        data
    )
}

/// Natural pixel width of the diagram, so PNGs can supersample.
fn natural_width(source: &str) -> Option<f64> {
    let mut child = Command::new("mmdr")
        .args(["-i", "-", "--size"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(source.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let width_line = text.lines().find(|line| line.contains("\"width\""))?;
    width_line
        .split(':')
        .nth(1)?
        .trim()
        .trim_end_matches(',')
        .parse()
        .ok()
}

/// Render mermaid source to PNG bytes via the `mmdr` binary, supersampled
/// at 3x natural size so graphics-protocol terminals get retina-crisp text.
pub fn mermaid_to_png(source: &str, theme: &str) -> Result<Vec<u8>> {
    let out = std::env::temp_dir().join(format!("stackdiff-{}.png", std::process::id()));
    let width = natural_width(source)
        .map(|w| ((w * 3.0) as u32).clamp(600, 6000).to_string())
        .unwrap_or_else(|| "2400".to_string());
    let mut child = Command::new("mmdr")
        .args(["-i", "-", "-e", "png", "-t", theme, "-w", &width, "-o"])
        .arg(&out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("mmdr not found — install it with `cargo install mermaid-rs-renderer`")?;
    child
        .stdin
        .take()
        .context("no stdin")?
        .write_all(source.as_bytes())?;
    let result = child.wait_with_output()?;
    if !result.status.success() {
        bail!(
            "mmdr failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    let png = std::fs::read(&out).context("mmdr produced no output")?;
    let _ = std::fs::remove_file(&out);
    Ok(png)
}

fn terminal_cols() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(100)
}

/// Paint PNG bytes as truecolor half-blocks (▀ with fg = upper pixel,
/// bg = lower pixel), two pixel rows per text row.
pub fn png_to_half_blocks(png: &[u8]) -> Result<String> {
    let img = image::load_from_memory(png).context("failed to decode png")?;
    let cols = terminal_cols().saturating_sub(2).clamp(20, 160) as u32;
    let (w, h) = img.dimensions();
    let target_w = cols.min(w.max(1));
    let target_h = ((h as f64 * target_w as f64 / w as f64).round() as u32).max(2);
    let img = img.resize_exact(target_w, target_h, image::imageops::FilterType::CatmullRom);

    let mut out = String::new();
    let mut y = 0;
    while y < target_h {
        for x in 0..target_w {
            let top = img.get_pixel(x, y);
            let bottom = if y + 1 < target_h {
                img.get_pixel(x, y + 1)
            } else {
                top
            };
            let (tr, tg, tb, ta) = (top[0], top[1], top[2], top[3]);
            let (br, bg, bb, ba) = (bottom[0], bottom[1], bottom[2], bottom[3]);
            match (ta > 8, ba > 8) {
                (false, false) => out.push_str("\x1b[0m "),
                (true, false) => {
                    out.push_str(&format!("\x1b[0m\x1b[38;2;{tr};{tg};{tb}m▀"));
                }
                (false, true) => {
                    out.push_str(&format!("\x1b[0m\x1b[38;2;{br};{bg};{bb}m▄"));
                }
                (true, true) => {
                    out.push_str(&format!(
                        "\x1b[38;2;{tr};{tg};{tb}m\x1b[48;2;{br};{bg};{bb}m▀"
                    ));
                }
            }
        }
        out.push_str("\x1b[0m\n");
        y += 2;
    }
    Ok(out)
}
