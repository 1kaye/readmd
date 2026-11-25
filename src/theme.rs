use gpui_kit::{App, Hsla, Overflow, StyleRefinement, WindowAppearance, base, hsla};

use crate::document::EditorMode;

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

/// LazyVim-inspired semantic mode indicator colors, defined once in the theme layer
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
        (base::ThemeAppearance::Light, EditorMode::Normal) => ModeBadgeColors {
            background: hsla(0.60, 0.72, 0.45, 1.0),
            foreground: hsla(0.0, 0.0, 0.98, 1.0),
        },
        (base::ThemeAppearance::Light, EditorMode::Insert) => ModeBadgeColors {
            background: hsla(0.40, 0.58, 0.36, 1.0),
            foreground: hsla(0.0, 0.0, 0.98, 1.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{is_dark_appearance, mode_badge_colors};
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
    fn mode_badge_colors_are_distinct_across_modes_and_appearances() {
        let mut light = base::Theme::default();
        light.appearance = base::ThemeAppearance::Light;

        let mut dark = base::Theme::default();
        dark.appearance = base::ThemeAppearance::Dark;

        assert_ne!(
            mode_badge_colors(&light, EditorMode::Normal),
            mode_badge_colors(&light, EditorMode::Insert)
        );
        assert_ne!(
            mode_badge_colors(&dark, EditorMode::Normal),
            mode_badge_colors(&dark, EditorMode::Insert)
        );
    }
}
