use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use gpui_kit::{App, Global, Hsla, Overflow, StyleRefinement, Styled as _, base, hsla, rems, rgb};

use crate::document::EditorMode;

/// Dedicated bundled UI font for application chrome (`StatusBar`, `CommandBar`, `HelpOverlay`).
/// Registered directly into GPUI's text system without touching `base::Theme` typography.
pub const APP_UI_FONT: &str = "JetBrainsMono Nerd Font";

const BUNDLED_APP_FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Bold.ttf"),
];

/// RGB triplet (0..=255 per channel) used for deterministic, crate-free palette math.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn to_hsla(self) -> Hsla {
        let hex = ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | (self.2 as u32);
        Hsla::from(rgb(hex))
    }

    /// WCAG 2.x relative luminance in `[0.0, 1.0]`.
    pub fn luminance(self) -> f32 {
        fn channel(v: u8) -> f32 {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }

    /// Linear blend from `self` (`t = 0.0`) toward `other` (`t = 1.0`).
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| -> u8 {
            ((a as f32) * (1.0 - t) + (b as f32) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Self(
            lerp(self.0, other.0),
            lerp(self.1, other.1),
            lerp(self.2, other.2),
        )
    }
}

/// Resolved semantic palette used to project into `gpui_kit::base::Theme` and chrome badges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPalette {
    pub appearance: base::ThemeAppearance,
    pub background: Rgb,
    pub foreground: Rgb,
    pub surface: Rgb,
    pub muted: Rgb,
    pub border: Rgb,
    pub accent: Rgb,
    pub destructive: Rgb,
    pub insert_badge: Rgb,
    pub command_badge: Rgb,
    pub selection_background: Option<Rgb>,
}

/// Snapshot of the resolved system theme (Omarchy `colors.toml` or soft dark fallback).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemThemeSnapshot {
    pub palette: AppPalette,
    pub is_omarchy: bool,
}

/// Built-in theme presets selectable via `:Theme <name>` (especially useful on non-Omarchy systems).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    Dark,
    Light,
    TokyoNight,
    Catppuccin,
    Gruvbox,
    Nord,
    FlexokiLight,
}

impl ThemePreset {
    pub const ALL: &'static [Self] = &[
        Self::Dark,
        Self::Light,
        Self::TokyoNight,
        Self::Catppuccin,
        Self::Gruvbox,
        Self::Nord,
        Self::FlexokiLight,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::TokyoNight => "tokyo-night",
            Self::Catppuccin => "catppuccin",
            Self::Gruvbox => "gruvbox",
            Self::Nord => "nord",
            Self::FlexokiLight => "flexoki-light",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Dark => "Soft matte charcoal dark",
            Self::Light => "Clean neutral light",
            Self::TokyoNight => "Tokyo Night dark",
            Self::Catppuccin => "Catppuccin Mocha dark",
            Self::Gruvbox => "Gruvbox dark",
            Self::Nord => "Nord polar dark",
            Self::FlexokiLight => "Flexoki warm paper light",
        }
    }

    pub fn palette(self) -> AppPalette {
        match self {
            Self::Dark => default_soft_dark_palette(),
            Self::Light => AppPalette {
                appearance: base::ThemeAppearance::Light,
                background: Rgb(0xfa, 0xfa, 0xf9),
                foreground: Rgb(0x18, 0x18, 0x1b),
                surface: Rgb(0xf2, 0xf2, 0xf0),
                muted: Rgb(0xe8, 0xe8, 0xe5),
                border: Rgb(0xd8, 0xd8, 0xd4),
                accent: Rgb(0x25, 0x63, 0xeb),
                destructive: Rgb(0xc9, 0x2a, 0x2a),
                insert_badge: Rgb(0x2b, 0x8a, 0x3e),
                command_badge: Rgb(0xc2, 0x6d, 0x0a),
                selection_background: None,
            },
            Self::TokyoNight => AppPalette {
                appearance: base::ThemeAppearance::Dark,
                background: Rgb(0x1a, 0x1b, 0x26),
                foreground: Rgb(0xc0, 0xca, 0xf5),
                surface: Rgb(0x22, 0x24, 0x33),
                muted: Rgb(0x2a, 0x2d, 0x3f),
                border: Rgb(0x36, 0x3a, 0x52),
                accent: Rgb(0x7a, 0xa2, 0xf7),
                destructive: Rgb(0xf7, 0x76, 0x8e),
                insert_badge: Rgb(0x9e, 0xce, 0x6a),
                command_badge: Rgb(0xe0, 0xaf, 0x68),
                selection_background: None,
            },
            Self::Catppuccin => AppPalette {
                appearance: base::ThemeAppearance::Dark,
                background: Rgb(0x1e, 0x1e, 0x2e),
                foreground: Rgb(0xcd, 0xd6, 0xf4),
                surface: Rgb(0x25, 0x25, 0x38),
                muted: Rgb(0x31, 0x32, 0x44),
                border: Rgb(0x45, 0x47, 0x5a),
                accent: Rgb(0x89, 0xb4, 0xfa),
                destructive: Rgb(0xf3, 0x8b, 0xa8),
                insert_badge: Rgb(0xa6, 0xe3, 0xa1),
                command_badge: Rgb(0xf9, 0xe2, 0xaf),
                selection_background: None,
            },
            Self::Gruvbox => AppPalette {
                appearance: base::ThemeAppearance::Dark,
                background: Rgb(0x28, 0x28, 0x28),
                foreground: Rgb(0xeb, 0xdb, 0xb2),
                surface: Rgb(0x32, 0x30, 0x2f),
                muted: Rgb(0x3c, 0x38, 0x36),
                border: Rgb(0x50, 0x49, 0x45),
                accent: Rgb(0x83, 0xa5, 0x98),
                destructive: Rgb(0xfb, 0x49, 0x34),
                insert_badge: Rgb(0xb8, 0xbb, 0x26),
                command_badge: Rgb(0xfa, 0xbd, 0x2f),
                selection_background: None,
            },
            Self::Nord => AppPalette {
                appearance: base::ThemeAppearance::Dark,
                background: Rgb(0x2e, 0x34, 0x40),
                foreground: Rgb(0xec, 0xef, 0xf4),
                surface: Rgb(0x36, 0x3d, 0x4b),
                muted: Rgb(0x3b, 0x42, 0x52),
                border: Rgb(0x4c, 0x56, 0x6a),
                accent: Rgb(0x88, 0xc0, 0xd0),
                destructive: Rgb(0xbf, 0x61, 0x6a),
                insert_badge: Rgb(0xa3, 0xbe, 0x8c),
                command_badge: Rgb(0xeb, 0xcb, 0x8b),
                selection_background: None,
            },
            Self::FlexokiLight => AppPalette {
                appearance: base::ThemeAppearance::Light,
                background: Rgb(0xff, 0xfc, 0xf0),
                foreground: Rgb(0x10, 0x0f, 0x0f),
                surface: Rgb(0xf2, 0xf0, 0xe5),
                muted: Rgb(0xe6, 0xe4, 0xd9),
                border: Rgb(0xda, 0xd8, 0xce),
                accent: Rgb(0x20, 0x5e, 0xa6),
                destructive: Rgb(0xaf, 0x30, 0x29),
                insert_badge: Rgb(0x66, 0x80, 0x0b),
                command_badge: Rgb(0xad, 0x83, 0x01),
                selection_background: None,
            },
        }
    }
}

/// Active theme selection mode: follow system (Omarchy / soft dark fallback) or a manual preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePreference {
    #[default]
    Auto,
    Preset(ThemePreset),
}

/// App-global retained palette so live theme polling only refreshes windows on real changes
/// and custom chrome (mode badges) stays synchronized with the active Omarchy/fallback theme.
#[derive(Clone, Debug)]
struct ActivePalette {
    preference: ThemePreference,
    palette: AppPalette,
}

impl Global for ActivePalette {}

/// Semantic foreground/background pair for the bottom-left mode indicator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeBadgeColors {
    pub background: Hsla,
    pub foreground: Hsla,
}

/// Default palette used when not running on Omarchy (or when `colors.toml` is missing/invalid).
/// Soft matte charcoal dark — clearly dark without being pitch black (`#000000`).
pub fn default_soft_dark_palette() -> AppPalette {
    AppPalette {
        appearance: base::ThemeAppearance::Dark,
        background: Rgb(0x16, 0x16, 0x18),
        foreground: Rgb(0xe4, 0xe4, 0xe7),
        surface: Rgb(0x1d, 0x1d, 0x21),
        muted: Rgb(0x25, 0x25, 0x2b),
        border: Rgb(0x2e, 0x2e, 0x36),
        accent: Rgb(0x7a, 0xa2, 0xf7),
        destructive: Rgb(0xe0, 0x6c, 0x75),
        insert_badge: Rgb(0x73, 0xc9, 0x91),
        command_badge: Rgb(0xe5, 0xa5, 0x5d),
        selection_background: None,
    }
}

/// Registers the bundled app UI font before opening any window.
/// Leaves `base::Theme::tokens.typography` untouched so Markdown defaults stay unaffected.
pub fn load_app_fonts(cx: &mut App) {
    let fonts = BUNDLED_APP_FONTS
        .iter()
        .map(|bytes| Cow::Borrowed(*bytes))
        .collect();
    cx.text_system()
        .add_fonts(fonts)
        .expect("Failed to register bundled JetBrainsMono Nerd Font");
}

/// Initializes the active application theme before opening windows.
/// Follows Omarchy's `colors.toml` when present; otherwise installs the soft charcoal dark theme.
pub fn init(cx: &mut App) {
    let snapshot = load_system_theme();
    apply_palette_with_preference(ThemePreference::Auto, snapshot.palette, cx);
}

pub fn active_preference(cx: &App) -> ThemePreference {
    cx.try_global::<ActivePalette>()
        .map(|active| active.preference)
        .unwrap_or(ThemePreference::Auto)
}

/// Switches the active theme preference (`auto` resumes Omarchy live sync; presets apply immediately).
pub fn set_preference(preference: ThemePreference, cx: &mut App) {
    let palette = match preference {
        ThemePreference::Auto => load_system_theme().palette,
        ThemePreference::Preset(preset) => preset.palette(),
    };
    apply_palette_with_preference(preference, palette, cx);
}

/// Re-reads the system theme synchronously and applies it if in `Auto` mode and changed.
/// Returns `true` when the active palette changed.
pub fn sync_with_system(cx: &mut App) -> bool {
    if active_preference(cx) != ThemePreference::Auto {
        return false;
    }
    let snapshot = load_system_theme();
    apply_system_palette_if_changed(snapshot.palette, cx)
}

/// Applies a background-polled system palette only when in `Auto` mode and the palette changed.
/// Returns `true` when `base::Theme` was updated.
pub fn apply_system_palette_if_changed(palette: AppPalette, cx: &mut App) -> bool {
    if active_preference(cx) != ThemePreference::Auto {
        return false;
    }
    if cx
        .try_global::<ActivePalette>()
        .is_some_and(|current| current.palette == palette)
    {
        return false;
    }
    apply_palette_with_preference(ThemePreference::Auto, palette, cx);
    true
}

/// Projects `palette` into `gpui_kit::base::Theme` and stores it as the active app palette.
fn apply_palette_with_preference(preference: ThemePreference, palette: AppPalette, cx: &mut App) {
    let is_dark = palette.appearance == base::ThemeAppearance::Dark;
    let colors = color_tokens_from_palette(&palette);

    let theme = base::Theme::global_mut(cx);
    theme.appearance = palette.appearance;
    theme.tokens.colors = colors;
    let shadow_color = if is_dark {
        hsla(0.0, 0.0, 0.0, 0.45)
    } else {
        hsla(0.0, 0.0, 0.0, 0.14)
    };
    theme.tokens.shadow = base::ShadowTokens::elevations(shadow_color);

    cx.set_global(ActivePalette {
        preference,
        palette,
    });
}

/// Reads the current Omarchy theme if present and valid; otherwise falls back to the soft dark theme.
/// Safe to call from a background thread.
pub fn load_system_theme() -> SystemThemeSnapshot {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    load_system_theme_from_home(home.as_deref())
}

pub fn load_system_theme_from_home(home: Option<&Path>) -> SystemThemeSnapshot {
    if let Some(home_dir) = home
        && let Some(current_dir) = omarchy_current_dir_from_home(home_dir)
    {
        let colors_path = current_dir.join("theme").join("colors.toml");
        if let Ok(contents) = fs::read_to_string(&colors_path)
            && let Ok(palette) = parse_omarchy_colors_toml(&contents)
        {
            return SystemThemeSnapshot {
                palette,
                is_omarchy: true,
            };
        }
    }

    SystemThemeSnapshot {
        palette: default_soft_dark_palette(),
        is_omarchy: false,
    }
}

/// Resolves Omarchy's active theme state directory:
/// primary `$HOME/.local/state/omarchy/current`, falling back to legacy
/// `$HOME/.config/omarchy/current` only when the primary directory is absent.
pub fn omarchy_current_dir_from_home(home: &Path) -> Option<PathBuf> {
    let primary = home
        .join(".local")
        .join("state")
        .join("omarchy")
        .join("current");
    if primary.exists() {
        return Some(primary);
    }
    let legacy = home.join(".config").join("omarchy").join("current");
    if legacy.exists() {
        return Some(legacy);
    }
    None
}

/// Parses an Omarchy `colors.toml` file into an `AppPalette`.
/// Requires `background` and `foreground`; derives optional surfaces and semantic accents cleanly.
pub fn parse_omarchy_colors_toml(contents: &str) -> Result<AppPalette, String> {
    let mut entries: HashMap<String, String> = HashMap::new();

    for raw_line in contents.lines() {
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let Some((raw_key, raw_val)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let val = unquote_toml_value(raw_val.trim())?;
        entries.insert(key, val);
    }

    let background = parse_required_color(&entries, "background")?;
    let foreground = parse_required_color(&entries, "foreground")?;

    let appearance = match entries.get("mode").map(|s| s.trim().to_ascii_lowercase()) {
        Some(ref m) if m == "dark" => base::ThemeAppearance::Dark,
        Some(ref m) if m == "light" => base::ThemeAppearance::Light,
        Some(other) => return Err(format!("Unsupported mode '{other}' in colors.toml")),
        None => {
            if background.luminance() < foreground.luminance() {
                base::ThemeAppearance::Dark
            } else {
                base::ThemeAppearance::Light
            }
        }
    };

    let is_dark = appearance == base::ThemeAppearance::Dark;
    let accent = parse_first_color(&entries, &["accent", "color4", "blue"]).unwrap_or(if is_dark {
        Rgb(0x7a, 0xa2, 0xf7)
    } else {
        Rgb(0x25, 0x63, 0xeb)
    });
    let destructive =
        parse_first_color(&entries, &["red", "color1", "danger"]).unwrap_or(if is_dark {
            Rgb(0xe0, 0x6c, 0x75)
        } else {
            Rgb(0xc9, 0x2a, 0x2a)
        });
    let insert_badge =
        parse_first_color(&entries, &["green", "color2", "success"]).unwrap_or(if is_dark {
            Rgb(0x73, 0xc9, 0x91)
        } else {
            Rgb(0x2b, 0x8a, 0x3e)
        });
    let command_badge =
        parse_first_color(&entries, &["yellow", "color3", "warning"]).unwrap_or(if is_dark {
            Rgb(0xe5, 0xa5, 0x5d)
        } else {
            Rgb(0xc2, 0x6d, 0x0a)
        });

    let surface = parse_first_color(&entries, &["surface"])
        .unwrap_or_else(|| background.mix(foreground, if is_dark { 0.045 } else { 0.035 }));
    let muted = parse_first_color(&entries, &["inset"])
        .unwrap_or_else(|| background.mix(foreground, if is_dark { 0.075 } else { 0.06 }));
    let border = parse_first_color(&entries, &["border"])
        .unwrap_or_else(|| background.mix(foreground, if is_dark { 0.14 } else { 0.15 }));
    let selection_background = parse_first_color(&entries, &["selection_background", "selection"]);

    Ok(AppPalette {
        appearance,
        background,
        foreground,
        surface,
        muted,
        border,
        accent,
        destructive,
        insert_badge,
        command_badge,
        selection_background,
    })
}

pub fn color_tokens_from_palette(palette: &AppPalette) -> base::ColorTokens {
    let is_dark = palette.appearance == base::ThemeAppearance::Dark;
    let mut colors = if is_dark {
        base::ColorTokens::dark()
    } else {
        base::ColorTokens::light()
    };

    let primary_fg = contrast_ink(palette.accent);
    let destructive_fg = contrast_ink(palette.destructive);

    // Follow Omarchy (or soft charcoal fallback) for app backgrounds & command-bar accents,
    // while keeping neutral text, muted surfaces, borders, and Markdown highlights standard.
    colors.background = palette.background.to_hsla();
    colors.surface = palette.surface.to_hsla();
    colors.accent = palette.accent.to_hsla();
    colors.accent_foreground = primary_fg.to_hsla();
    colors.destructive = palette.destructive.to_hsla();
    colors.destructive_foreground = destructive_fg.to_hsla();
    colors.ring = palette.accent.to_hsla();

    colors
}

fn neutral_document_theme(theme: &base::Theme) -> base::Theme {
    let mut doc_theme = theme.clone();
    let is_dark = theme.appearance == base::ThemeAppearance::Dark;
    let mut colors = if is_dark {
        base::ColorTokens::dark()
    } else {
        base::ColorTokens::light()
    };
    colors.background = theme.tokens.colors.background;
    // Translucent neutral grays adapt automatically to any dark or light canvas:
    // slightly lighter neutral gray on dark themes, soft darker neutral gray on light themes.
    colors.muted = if is_dark {
        hsla(0.0, 0.0, 1.0, 0.085)
    } else {
        hsla(0.0, 0.0, 0.0, 0.055)
    };
    colors.border = if is_dark {
        hsla(0.0, 0.0, 1.0, 0.16)
    } else {
        hsla(0.0, 0.0, 0.0, 0.14)
    };
    doc_theme.tokens.colors = colors;
    doc_theme
}

pub fn document_style(theme: &base::Theme) -> base::TextViewStyle {
    let doc_theme = neutral_document_theme(theme);
    let is_dark = doc_theme.appearance == base::ThemeAppearance::Dark;

    // Inline code pills (e.g. `gpui-base`) are lifted slightly brighter in dark mode
    // and softly shaded in light mode, without any outline/border.
    let inline_bg = if is_dark {
        hsla(0.0, 0.0, 1.0, 0.14)
    } else {
        hsla(0.0, 0.0, 0.0, 0.08)
    };

    let inline_code = StyleRefinement::default()
        .bg(inline_bg)
        .px(rems(0.3))
        .rounded(rems(0.25));

    let mut table = StyleRefinement::default();
    table.overflow.x = Some(Overflow::Scroll);

    base::TextViewStyle::from_theme(&doc_theme)
        .with_inline_code_style(inline_code)
        .with_table(table)
        .with_table_row_hover(doc_theme.tokens.colors.muted)
}

pub fn editor_style(theme: &base::Theme) -> base::input::InputEditorStyle {
    let doc_theme = neutral_document_theme(theme);
    let colors = doc_theme.tokens.colors;
    base::input::InputEditorStyle {
        foreground: colors.foreground,
        muted_foreground: colors.muted_foreground,
        background: colors.background,
        border: colors.border,
        selection: colors.selection,
        caret: colors.foreground,
        ..Default::default()
    }
}

/// Input style for the top-center command bar, matching its raised `surface` background.
pub fn command_input_style(theme: &base::Theme) -> base::input::InputEditorStyle {
    let mut style = editor_style(theme);
    style.background = theme.tokens.colors.surface;
    style
}

/// Semantic mode badge colors derived from the active Omarchy/fallback palette
/// with guaranteed readable foreground contrast.
pub fn mode_badge_colors(cx: &App, mode: EditorMode) -> ModeBadgeColors {
    let palette = cx
        .try_global::<ActivePalette>()
        .map(|active| active.palette.clone())
        .unwrap_or_else(default_soft_dark_palette);
    mode_badge_colors_for_palette(&palette, mode)
}

pub fn mode_badge_colors_for_palette(palette: &AppPalette, mode: EditorMode) -> ModeBadgeColors {
    let bg = match mode {
        EditorMode::Normal => palette.accent,
        EditorMode::Insert => palette.insert_badge,
        EditorMode::Command => palette.command_badge,
    };
    ModeBadgeColors {
        background: bg.to_hsla(),
        foreground: contrast_ink(bg).to_hsla(),
    }
}

fn contrast_ink(bg: Rgb) -> Rgb {
    let dark = Rgb(0x11, 0x11, 0x14);
    let light = Rgb(0xfa, 0xfa, 0xfc);
    let bg_l = bg.luminance();
    let dark_ratio = (bg_l.max(dark.luminance()) + 0.05) / (bg_l.min(dark.luminance()) + 0.05);
    let light_ratio = (bg_l.max(light.luminance()) + 0.05) / (bg_l.min(light.luminance()) + 0.05);
    if dark_ratio >= light_ratio {
        dark
    } else {
        light
    }
}

fn parse_required_color(entries: &HashMap<String, String>, key: &str) -> Result<Rgb, String> {
    let raw = entries
        .get(key)
        .ok_or_else(|| format!("Missing required key '{key}' in colors.toml"))?;
    parse_hex_rgb(raw).ok_or_else(|| format!("Invalid hex color for '{key}': '{raw}'"))
}

fn parse_first_color(entries: &HashMap<String, String>, keys: &[&str]) -> Option<Rgb> {
    keys.iter()
        .find_map(|key| entries.get(*key).and_then(|raw| parse_hex_rgb(raw)))
}

pub fn parse_hex_rgb(raw: &str) -> Option<Rgb> {
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Rgb(r, g, b))
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_double = false;
    let mut in_single = false;
    let mut escaped = false;
    for (ix, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_double => escaped = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '#' if !in_double && !in_single => return &line[..ix],
            _ => {}
        }
    }
    line
}

fn unquote_toml_value(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Ok(inner.to_string());
    }
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
    {
        return Ok(inner.to_string());
    }
    if trimmed.is_empty() {
        return Err("Empty value in colors.toml".to_string());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use gpui_kit::base;

    use super::{
        BUNDLED_APP_FONTS, Rgb, ThemePreset, color_tokens_from_palette, default_soft_dark_palette,
        document_style, load_system_theme_from_home, mode_badge_colors_for_palette, parse_hex_rgb,
        parse_omarchy_colors_toml,
    };
    use crate::document::EditorMode;

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "readmd-theme-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn bundled_app_fonts_have_valid_truetype_headers() {
        assert_eq!(BUNDLED_APP_FONTS.len(), 2);
        for bytes in BUNDLED_APP_FONTS {
            assert!(bytes.len() > 1024);
            assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
        }
    }

    #[test]
    fn soft_dark_fallback_is_dark_charcoal_not_pitch_black() {
        let fallback = default_soft_dark_palette();
        assert_eq!(fallback.appearance, base::ThemeAppearance::Dark);
        // Not pitch black (#000000), yet comfortably dark charcoal.
        assert_ne!(fallback.background, Rgb(0, 0, 0));
        assert!(fallback.background.0 >= 0x12 && fallback.background.0 <= 0x20);
        assert!(fallback.background.luminance() < 0.03);
        assert!(fallback.surface.luminance() > fallback.background.luminance());
    }

    #[test]
    fn parses_omarchy_dark_and_light_colors_toml_with_comments() {
        let dark_toml = r##"
            # Tokyo Night sample
            accent = "#7aa2f7"
            cursor = "#c0caf5"
            foreground = "#c0caf5" # inline comment
            background = "#1a1b26"
            selection_foreground = "#c0caf5"
            selection_background = "#33467c"
            color1 = "#f7768e"
            color2 = "#9ece6a"
            color3 = "#e0af68"
            color4 = "#7aa2f7"
        "##;
        let dark = parse_omarchy_colors_toml(dark_toml).expect("valid dark colors.toml");
        assert_eq!(dark.appearance, base::ThemeAppearance::Dark);
        assert_eq!(dark.background, Rgb(0x1a, 0x1b, 0x26));
        assert_eq!(dark.foreground, Rgb(0xc0, 0xca, 0xf5));
        assert_eq!(dark.accent, Rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(dark.destructive, Rgb(0xf7, 0x76, 0x8e));
        assert_eq!(dark.insert_badge, Rgb(0x9e, 0xce, 0x6a));
        assert_eq!(dark.command_badge, Rgb(0xe0, 0xaf, 0x68));
        assert_eq!(dark.selection_background, Some(Rgb(0x33, 0x46, 0x7c)));

        let light_toml = r##"
            background = "#fffcf0"
            foreground = "#100f0f"
            accent = "#205ea6"
            color1 = "#af3029"
            color2 = "#66800b"
            color3 = "#ad8301"
        "##;
        let light = parse_omarchy_colors_toml(light_toml).expect("valid light colors.toml");
        assert_eq!(light.appearance, base::ThemeAppearance::Light);
        assert_eq!(light.background, Rgb(0xff, 0xfc, 0xf0));
        let tokens = color_tokens_from_palette(&light);
        assert_eq!(tokens.background, Rgb(0xff, 0xfc, 0xf0).to_hsla());
    }

    #[test]
    fn rejects_incomplete_or_invalid_colors_toml() {
        assert!(parse_omarchy_colors_toml("foreground = \"#ffffff\"").is_err());
        assert!(
            parse_omarchy_colors_toml("background = \"not-hex\"\nforeground = \"#ffffff\"")
                .is_err()
        );
        assert_eq!(parse_hex_rgb("#1a1b26"), Some(Rgb(0x1a, 0x1b, 0x26)));
        assert_eq!(parse_hex_rgb("#xyz123"), None);
    }

    #[test]
    fn loads_omarchy_primary_over_legacy_and_falls_back_on_non_omarchy() {
        let dir = unique_temp_dir("lookup");
        fs::create_dir_all(&dir).unwrap();

        // Non-Omarchy home -> soft dark fallback
        let non_omarchy = load_system_theme_from_home(Some(&dir));
        assert!(!non_omarchy.is_omarchy);
        assert_eq!(non_omarchy.palette, default_soft_dark_palette());

        // Legacy Omarchy dir used when primary is absent
        let legacy_theme = dir.join(".config/omarchy/current/theme");
        fs::create_dir_all(&legacy_theme).unwrap();
        fs::write(
            legacy_theme.join("colors.toml"),
            "background = \"#282828\"\nforeground = \"#ebdbb2\"\naccent = \"#83a598\"\n",
        )
        .unwrap();
        let legacy_loaded = load_system_theme_from_home(Some(&dir));
        assert!(legacy_loaded.is_omarchy);
        assert_eq!(legacy_loaded.palette.background, Rgb(0x28, 0x28, 0x28));

        // Primary Omarchy dir takes precedence over legacy
        let primary_theme = dir.join(".local/state/omarchy/current/theme");
        fs::create_dir_all(&primary_theme).unwrap();
        fs::write(
            primary_theme.join("colors.toml"),
            "background = \"#1e1e2e\"\nforeground = \"#cdd6f4\"\naccent = \"#89b4fa\"\n",
        )
        .unwrap();
        let primary_loaded = load_system_theme_from_home(Some(&dir));
        assert!(primary_loaded.is_omarchy);
        assert_eq!(primary_loaded.palette.background, Rgb(0x1e, 0x1e, 0x2e));

        // Existing but invalid primary falls back to soft dark (does not revive stale legacy)
        fs::write(primary_theme.join("colors.toml"), "invalid").unwrap();
        let invalid_primary = load_system_theme_from_home(Some(&dir));
        assert!(!invalid_primary.is_omarchy);
        assert_eq!(invalid_primary.palette, default_soft_dark_palette());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn color_tokens_follow_omarchy_background_and_command_accent_but_keep_neutral_markdown_muted() {
        let dark_toml = r##"
            background = "#1a1b26"
            foreground = "#c0caf5"
            accent = "#ff55aa"
            selection_background = "#552288"
        "##;
        let palette = parse_omarchy_colors_toml(dark_toml).expect("valid toml");
        let tokens = color_tokens_from_palette(&palette);
        let neutral_dark = base::ColorTokens::dark();

        // Background & command-bar accent follow Omarchy.
        assert_eq!(tokens.background, Rgb(0x1a, 0x1b, 0x26).to_hsla());
        assert_eq!(tokens.accent, Rgb(0xff, 0x55, 0xaa).to_hsla());
        assert_eq!(tokens.ring, Rgb(0xff, 0x55, 0xaa).to_hsla());

        // Markdown highlights (muted code pills, selection, links, foreground) stay neutral.
        assert_eq!(tokens.muted, neutral_dark.muted);
        assert_eq!(tokens.selection, neutral_dark.selection);
        assert_eq!(tokens.primary, neutral_dark.primary);
        assert_eq!(tokens.foreground, neutral_dark.foreground);
    }

    #[test]
    fn document_style_uses_brighter_inline_code_in_dark_mode_and_shaded_in_light_mode() {
        let mut dark_theme = base::Theme::default();
        dark_theme.appearance = base::ThemeAppearance::Dark;
        dark_theme.tokens.colors = color_tokens_from_palette(&ThemePreset::Dark.palette());

        let mut light_theme = base::Theme::default();
        light_theme.appearance = base::ThemeAppearance::Light;
        light_theme.tokens.colors = color_tokens_from_palette(&ThemePreset::Light.palette());

        let dark_doc = document_style(&dark_theme);
        let light_doc = document_style(&light_theme);
        let dark_inline = dark_doc.inline_code_style();
        let light_inline = light_doc.inline_code_style();

        assert!(dark_inline.background.is_some());
        assert!(light_inline.background.is_some());
        assert_ne!(dark_inline.background, light_inline.background);
    }

    #[test]
    fn mode_badge_colors_are_distinct_across_all_modes() {
        let palette = default_soft_dark_palette();
        let normal = mode_badge_colors_for_palette(&palette, EditorMode::Normal);
        let insert = mode_badge_colors_for_palette(&palette, EditorMode::Insert);
        let command = mode_badge_colors_for_palette(&palette, EditorMode::Command);

        assert_ne!(normal, insert);
        assert_ne!(normal, command);
        assert_ne!(insert, command);
    }
}
