use std::borrow::Cow;

use gpui_kit::{App, Hsla, Overflow, StyleRefinement, WindowAppearance, base, hsla};

use crate::document::EditorMode;

/// Dedicated bundled UI font for application chrome (`StatusBar`, `CommandBar`).
/// Registered directly into GPUI's text system without touching `base::Theme` typography.
pub const APP_UI_FONT: &str = "JetBrainsMono Nerd Font";

const BUNDLED_APP_FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFont-Bold.ttf"),
];

/// Semantic foreground/background pair for the bottom-left mode indicator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeBadgeColors {
    pub background: Hsla,
    pub foreground: Hsla,
}

pub fn is_dark_appearance(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
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

pub fn sync_with_appearance(appearance: WindowAppearance, cx: &mut App) {
    let is_dark = is_dark_appearance(appearance);
    let theme = base::Theme::global_mut(cx);
    theme.appearance = if is_dark {
        base::ThemeAppearance::Dark
    } else {
        base::ThemeAppearance::Light
    };
    theme.tokens.colors = if is_dark {
        base::ColorTokens::dark()
    } else {
        base::ColorTokens::light()
    };
    let shadow_color = if is_dark {
        hsla(0.0, 0.0, 0.0, 0.45)
    } else {
        hsla(0.0, 0.0, 0.0, 0.14)
    };
    theme.tokens.shadow = base::ShadowTokens::elevations(shadow_color);
}

pub fn document_style(theme: &base::Theme) -> base::TextViewStyle {
    let mut table = StyleRefinement::default();
    table.overflow.x = Some(Overflow::Scroll);
    base::TextViewStyle::from_theme(theme)
        .with_table(table)
        .with_table_row_hover(theme.tokens.colors.muted)
}

pub fn editor_style(theme: &base::Theme) -> base::input::InputEditorStyle {
    let colors = theme.tokens.colors;
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

/// with balanced contrast for both light and dark appearances.
pub fn mode_badge_colors(theme: &base::Theme, mode: EditorMode) -> ModeBadgeColors {
    match (theme.appearance, mode) {
        (base::ThemeAppearance::Dark, EditorMode::Normal) => ModeBadgeColors {
            background: hsla(0.60, 0.72, 0.65, 1.0),
            foreground: hsla(0.60, 0.25, 0.08, 1.0),
        },
        (base::ThemeAppearance::Dark, EditorMode::Insert) => ModeBadgeColors {
            background: hsla(0.38, 0.52, 0.56, 1.0),
            foreground: hsla(0.38, 0.25, 0.08, 1.0),
        },
        (base::ThemeAppearance::Dark, EditorMode::Command) => ModeBadgeColors {
            background: hsla(0.10, 0.78, 0.62, 1.0),
            foreground: hsla(0.10, 0.30, 0.08, 1.0),
        },
        (base::ThemeAppearance::Light, EditorMode::Normal) => ModeBadgeColors {
            background: hsla(0.60, 0.72, 0.45, 1.0),
            foreground: hsla(0.0, 0.0, 0.98, 1.0),
        },
        (base::ThemeAppearance::Light, EditorMode::Insert) => ModeBadgeColors {
            background: hsla(0.40, 0.58, 0.36, 1.0),
            foreground: hsla(0.0, 0.0, 0.98, 1.0),
        },
        (base::ThemeAppearance::Light, EditorMode::Command) => ModeBadgeColors {
            background: hsla(0.09, 0.76, 0.44, 1.0),
            foreground: hsla(0.0, 0.0, 0.98, 1.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{BUNDLED_APP_FONTS, is_dark_appearance, mode_badge_colors};
    use crate::document::EditorMode;
    use gpui_kit::{WindowAppearance, base};

    #[test]
    fn dark_appearance_maps_all_variants() {
        assert!(!is_dark_appearance(WindowAppearance::Light));
        assert!(!is_dark_appearance(WindowAppearance::VibrantLight));
        assert!(is_dark_appearance(WindowAppearance::Dark));
        assert!(is_dark_appearance(WindowAppearance::VibrantDark));
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
    fn mode_badge_colors_are_distinct_across_all_modes_and_appearances() {
        for appearance in [base::ThemeAppearance::Light, base::ThemeAppearance::Dark] {
            let mut theme = base::Theme::default();
            theme.appearance = appearance;

            let normal = mode_badge_colors(&theme, EditorMode::Normal);
            let insert = mode_badge_colors(&theme, EditorMode::Insert);
            let command = mode_badge_colors(&theme, EditorMode::Command);

            assert_ne!(normal, insert);
            assert_ne!(normal, command);
            assert_ne!(insert, command);
        }
    }
}
