//! Named terminal themes: five palette tones that `Sheet` (and, where it
//! reads the shared palette, the TUI) paint with. A theme resolves once at
//! dispatch from the user config plus the terminal's reported appearance,
//! and single tokens stay overridable.
//!
//! Palette sources (each from its canonical upstream, not from another tool):
//! - Catppuccin <https://github.com/catppuccin/catppuccin> (MIT)
//! - Tokyo Night <https://github.com/enkia/tokyo-night-vscode-theme> (MIT)
//! - Nord <https://www.nordtheme.com> (MIT)

use std::collections::HashMap;
use std::sync::OnceLock;

/// One resolved palette: an RGB value and a basic ANSI fallback per tone,
/// so themes stay meaningful on terminals without truecolor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone {
    pub rgb: (u8, u8, u8),
    pub ansi: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTones {
    pub accent: Tone,
    pub good: Tone,
    pub alert: Tone,
    pub bad: Tone,
    pub violet: Tone,
}

pub const DEFAULT_DARK: &str = "rune-dark";
pub const DEFAULT_LIGHT: &str = "rune-light";

const THEMES: &[(&str, ThemeTones)] = &[
    (
        "rune-dark",
        ThemeTones {
            accent: Tone {
                rgb: (125, 207, 255),
                ansi: 36,
            },
            good: Tone {
                rgb: (158, 206, 106),
                ansi: 32,
            },
            alert: Tone {
                rgb: (224, 175, 104),
                ansi: 33,
            },
            bad: Tone {
                rgb: (247, 118, 142),
                ansi: 31,
            },
            violet: Tone {
                rgb: (187, 154, 247),
                ansi: 35,
            },
        },
    ),
    (
        "rune-light",
        ThemeTones {
            accent: Tone {
                rgb: (0, 95, 135),
                ansi: 34,
            },
            good: Tone {
                rgb: (58, 112, 16),
                ansi: 32,
            },
            alert: Tone {
                rgb: (154, 93, 0),
                ansi: 33,
            },
            bad: Tone {
                rgb: (179, 38, 66),
                ansi: 31,
            },
            violet: Tone {
                rgb: (110, 66, 180),
                ansi: 35,
            },
        },
    ),
    (
        "catppuccin-mocha",
        ThemeTones {
            accent: Tone {
                rgb: (137, 180, 250),
                ansi: 36,
            },
            good: Tone {
                rgb: (166, 227, 161),
                ansi: 32,
            },
            alert: Tone {
                rgb: (250, 179, 135),
                ansi: 33,
            },
            bad: Tone {
                rgb: (243, 139, 168),
                ansi: 31,
            },
            violet: Tone {
                rgb: (203, 166, 247),
                ansi: 35,
            },
        },
    ),
    (
        "catppuccin-latte",
        ThemeTones {
            accent: Tone {
                rgb: (30, 102, 245),
                ansi: 34,
            },
            good: Tone {
                rgb: (64, 160, 43),
                ansi: 32,
            },
            alert: Tone {
                rgb: (254, 100, 11),
                ansi: 33,
            },
            bad: Tone {
                rgb: (210, 15, 57),
                ansi: 31,
            },
            violet: Tone {
                rgb: (136, 57, 239),
                ansi: 35,
            },
        },
    ),
    (
        "tokyo-night",
        ThemeTones {
            accent: Tone {
                rgb: (122, 162, 247),
                ansi: 36,
            },
            good: Tone {
                rgb: (158, 206, 106),
                ansi: 32,
            },
            alert: Tone {
                rgb: (255, 158, 100),
                ansi: 33,
            },
            bad: Tone {
                rgb: (247, 118, 142),
                ansi: 31,
            },
            violet: Tone {
                rgb: (187, 154, 247),
                ansi: 35,
            },
        },
    ),
    (
        "nord",
        ThemeTones {
            accent: Tone {
                rgb: (136, 192, 208),
                ansi: 36,
            },
            good: Tone {
                rgb: (163, 190, 140),
                ansi: 32,
            },
            alert: Tone {
                rgb: (235, 203, 139),
                ansi: 33,
            },
            bad: Tone {
                rgb: (191, 97, 106),
                ansi: 31,
            },
            violet: Tone {
                rgb: (180, 142, 173),
                ansi: 35,
            },
        },
    ),
];

/// The theme half of the user configuration.
#[derive(Debug, Clone, Default)]
pub struct ThemeSelection {
    pub name: Option<String>,
    pub auto_switch: bool,
    pub dark_name: Option<String>,
    pub light_name: Option<String>,
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
    Unknown,
}

/// Classify the host appearance from the `COLORFGBG` convention
/// (`<fg>;<bg>`, background 7 or 15 means a light terminal).
#[must_use]
pub fn appearance_from_colorfgbg(value: Option<&str>) -> Appearance {
    let Some(value) = value else {
        return Appearance::Unknown;
    };
    let Some(background) = value.rsplit(';').next() else {
        return Appearance::Unknown;
    };
    match background.trim().parse::<u8>() {
        Ok(7 | 15) => Appearance::Light,
        Ok(_) => Appearance::Dark,
        Err(_) => Appearance::Unknown,
    }
}

#[must_use]
pub fn named(name: &str) -> Option<ThemeTones> {
    THEMES
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, tones)| *tones)
}

/// Resolve the selection to one palette. Returns the palette plus a warning
/// when a configured name is unknown; the default always applies.
#[must_use]
pub fn resolve(selection: &ThemeSelection, appearance: Appearance) -> (ThemeTones, Vec<String>) {
    let mut warnings = Vec::new();
    let base_name = if selection.auto_switch {
        match appearance {
            Appearance::Light => selection
                .light_name
                .clone()
                .unwrap_or_else(|| DEFAULT_LIGHT.to_string()),
            Appearance::Dark => selection
                .dark_name
                .clone()
                .unwrap_or_else(|| DEFAULT_DARK.to_string()),
            Appearance::Unknown => selection
                .name
                .clone()
                .unwrap_or_else(|| DEFAULT_DARK.to_string()),
        }
    } else {
        selection
            .name
            .clone()
            .unwrap_or_else(|| DEFAULT_DARK.to_string())
    };
    let mut tones = named(&base_name).unwrap_or_else(|| {
        warnings.push(format!(
            "unknown theme '{base_name}'; using {DEFAULT_DARK}. Known themes: {}",
            THEMES
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        named(DEFAULT_DARK).expect("the default theme exists")
    });
    for (token, value) in &selection.custom {
        let Some(rgb) = parse_hex(value) else {
            warnings.push(format!("theme.custom.{token}: invalid color '{value}'"));
            continue;
        };
        match token.as_str() {
            "accent" => tones.accent.rgb = rgb,
            "good" => tones.good.rgb = rgb,
            "alert" => tones.alert.rgb = rgb,
            "bad" => tones.bad.rgb = rgb,
            "violet" => tones.violet.rgb = rgb,
            other => warnings.push(format!(
                "theme.custom.{other}: unknown token; tokens: accent, good, alert, bad, violet"
            )),
        }
    }
    (tones, warnings)
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((red, green, blue))
}

static CURRENT: OnceLock<ThemeTones> = OnceLock::new();

/// Install the resolved palette once at dispatch. Later calls keep the
/// first palette, which matches the one-resolution contract.
pub fn install(tones: ThemeTones) {
    let _ = CURRENT.set(tones);
}

/// The active palette; the built-in dark default before any install.
#[must_use]
pub fn current() -> ThemeTones {
    CURRENT
        .get()
        .copied()
        .unwrap_or_else(|| named(DEFAULT_DARK).expect("the default theme exists"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorfgbg_classifies_light_and_dark() {
        assert_eq!(appearance_from_colorfgbg(Some("15;0")), Appearance::Dark);
        assert_eq!(appearance_from_colorfgbg(Some("0;15")), Appearance::Light);
        assert_eq!(appearance_from_colorfgbg(Some("0;7")), Appearance::Light);
        assert_eq!(appearance_from_colorfgbg(None), Appearance::Unknown);
        assert_eq!(
            appearance_from_colorfgbg(Some("default;default")),
            Appearance::Unknown
        );
    }

    #[test]
    fn auto_switch_prefers_the_appearance_pair() {
        let selection = ThemeSelection {
            auto_switch: true,
            ..ThemeSelection::default()
        };
        let (light, _) = resolve(&selection, Appearance::Light);
        assert_eq!(light, named(DEFAULT_LIGHT).unwrap());
        let (dark, _) = resolve(&selection, Appearance::Dark);
        assert_eq!(dark, named(DEFAULT_DARK).unwrap());
    }

    #[test]
    fn unknown_name_warns_and_keeps_the_default() {
        let selection = ThemeSelection {
            name: Some("no-such-theme".to_string()),
            ..ThemeSelection::default()
        };
        let (tones, warnings) = resolve(&selection, Appearance::Unknown);
        assert_eq!(tones, named(DEFAULT_DARK).unwrap());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no-such-theme"));
        assert!(warnings[0].contains("Known themes:"));
    }

    #[test]
    fn custom_token_overrides_one_tone() {
        let mut custom = HashMap::new();
        custom.insert("accent".to_string(), "#ff0000".to_string());
        let selection = ThemeSelection {
            custom,
            ..ThemeSelection::default()
        };
        let (tones, warnings) = resolve(&selection, Appearance::Unknown);
        assert_eq!(tones.accent.rgb, (255, 0, 0));
        assert_eq!(tones.good, named(DEFAULT_DARK).unwrap().good);
        assert!(warnings.is_empty());
    }

    #[test]
    fn invalid_custom_color_warns_and_keeps_the_base() {
        let mut custom = HashMap::new();
        custom.insert("accent".to_string(), "red".to_string());
        let selection = ThemeSelection {
            custom,
            ..ThemeSelection::default()
        };
        let (tones, warnings) = resolve(&selection, Appearance::Unknown);
        assert_eq!(tones.accent, named(DEFAULT_DARK).unwrap().accent);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn parse_hex_rejects_non_ascii_without_panicking() {
        assert_eq!(parse_hex("#a\u{2713}ab"), None);
        assert_eq!(parse_hex("#\u{e9}\u{e9}\u{e9}"), None);
        assert_eq!(parse_hex("#7dcfff"), Some((125, 207, 255)));
    }
}
