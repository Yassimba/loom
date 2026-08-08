//! Status palette shared by every renderer. Dark is the default; light
//! flips the tints for white terminals; auto reads COLORFGBG when the
//! terminal exports it.

use crate::types::DiffStatus;

pub type Rgb = (u8, u8, u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub added: (Rgb, Rgb),
    pub removed: (Rgb, Rgb),
    pub changed: (Rgb, Rgb),
}

/// GitHub-dark diff colors: bright text over a subtle tint.
impl Default for Palette {
    fn default() -> Self {
        DARK
    }
}

pub const DARK: Palette = Palette {
    added: ((63, 185, 80), (14, 40, 22)),
    removed: ((248, 81, 73), (45, 17, 17)),
    changed: ((210, 153, 34), (43, 35, 10)),
};

/// GitHub-light diff colors: deep text over a pale wash.
pub const LIGHT: Palette = Palette {
    added: ((17, 99, 41), (218, 251, 225)),
    removed: ((207, 34, 46), (255, 235, 233)),
    changed: ((122, 92, 0), (255, 248, 197)),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Auto,
    Dark,
    Light,
}

impl Palette {
    pub fn pick(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => DARK,
            Theme::Light => LIGHT,
            Theme::Auto => {
                // COLORFGBG is "fg;bg" — bg 7/15 means a light terminal.
                let light = std::env::var("COLORFGBG")
                    .ok()
                    .and_then(|value| {
                        value
                            .rsplit(';')
                            .next()
                            .and_then(|bg| bg.parse::<u8>().ok())
                    })
                    .map(|bg| bg == 7 || bg == 15)
                    .unwrap_or(false);
                if light {
                    LIGHT
                } else {
                    DARK
                }
            }
        }
    }

    pub fn pair(&self, status: DiffStatus) -> Option<(Rgb, Rgb)> {
        match status {
            DiffStatus::Added => Some(self.added),
            DiffStatus::Removed => Some(self.removed),
            DiffStatus::Changed => Some(self.changed),
            DiffStatus::Same => None,
        }
    }

    /// Foreground escape, optionally with the tinted background.
    pub fn open(&self, status: DiffStatus, tinted: bool) -> Option<String> {
        let ((fr, fg, fb), (br, bg, bb)) = self.pair(status)?;
        Some(if tinted {
            format!("\x1b[38;2;{fr};{fg};{fb}m\x1b[48;2;{br};{bg};{bb}m")
        } else {
            format!("\x1b[38;2;{fr};{fg};{fb}m")
        })
    }
}
