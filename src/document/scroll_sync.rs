use std::ops::Range;

use gpui_kit::{Bounds, Pixels, base};

const SCROLL_SYNC_EPSILON: f64 = 0.5;
const RAW_WRAP_COLS: f64 = 76.0;
const RENDERED_WRAP_COLS: f64 = 78.0;
const CODE_WRAP_COLS: f64 = 72.0;
const ANCHOR_SEARCH_RADIUS: usize = 16;

pub fn clamp_scroll_offset(current_y: Pixels, delta_y: Pixels, max_y: Pixels) -> Pixels {
    let limit = max_y.max(Pixels::ZERO);
    (current_y + delta_y).clamp(-limit, Pixels::ZERO)
}

pub fn editor_max_scroll_y(content_height: f64, viewport_height: f64) -> f64 {
    (content_height - viewport_height).max(0.0)
}

pub fn scroll_ratio(offset_y: f64, max_y: f64) -> f64 {
    if max_y <= 0.0 {
        0.0
    } else {
        (-offset_y / max_y).clamp(0.0, 1.0)
    }
}

pub fn target_scroll_y(ratio: f64, max_y: f64) -> f64 {
    if max_y <= 0.0 {
        0.0
    } else {
        -(ratio.clamp(0.0, 1.0) * max_y)
    }
}

pub fn needs_scroll_adjustment(current_y: f64, target_y: f64) -> bool {
    (current_y - target_y).abs() > SCROLL_SYNC_EPSILON
}

/// Maps a scroll ratio in rendered Markdown (`Normal` mode) to the matching
/// scroll ratio in raw Markdown (`Insert` mode), accounting for headings,
/// collapsed blank lines, fenced code blocks, tables, and Markdown link syntax.
pub fn map_preview_ratio_to_editor_ratio(source: &str, preview_ratio: f64) -> f64 {
    map_weighted_ratio(source, preview_ratio, false)
}

/// Inverse of [`map_preview_ratio_to_editor_ratio`]: maps a raw editor scroll
/// ratio (`Insert` mode) to the matching rendered preview ratio (`Normal` mode).
pub fn map_editor_ratio_to_preview_ratio(source: &str, editor_ratio: f64) -> f64 {
    map_weighted_ratio(source, editor_ratio, true)
}

/// Returns the UTF-8 byte range of a 0-based source line (excluding trailing `\r`/`\n`).
pub fn line_byte_range(source: &str, target_line: usize) -> Option<Range<usize>> {
    let mut line_ix = 0usize;
    let mut start = 0usize;
    for (idx, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            if line_ix == target_line {
                let end = if idx > start && source.as_bytes()[idx - 1] == b'\r' {
                    idx - 1
                } else {
                    idx
                };
                return Some(start..end);
            }
            line_ix += 1;
            start = idx + 1;
        }
    }
    if line_ix == target_line {
        let mut end = source.len();
        if end > start && source.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        Some(start..end)
    } else {
        None
    }
}

/// Converts a UTF-8 byte offset in `source` to a 0-based `(line, utf16_col)` cursor `Position`.
pub fn byte_offset_to_position(source: &str, byte_offset: usize) -> base::input::Position {
    let mut clamped = byte_offset.min(source.len());
    while clamped > 0 && !source.is_char_boundary(clamped) {
        clamped -= 1;
    }
    let prefix = &source[..clamped];
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (idx, byte) in prefix.bytes().enumerate() {
        if byte == b'\n' {
            line += 1;
            line_start = idx + 1;
        }
    }
    let col = prefix[line_start..].encode_utf16().count() as u32;
    base::input::Position::new(line, col)
}

/// Finds the rendered text range corresponding to `anchor_line`, scanning nearby
/// lines when `anchor_line` itself renders no text (blank line, code fence, HR).
pub fn find_rendered_anchor_range(
    rendered: &base::RenderedText,
    anchor_line: usize,
) -> Option<Range<usize>> {
    let source = rendered.source();
    if let Some(range) = line_byte_range(source, anchor_line)
        && let Some(hit) = rendered.range_for_source(range)
    {
        return Some(hit);
    }

    for delta in 1..=ANCHOR_SEARCH_RADIUS {
        if let Some(range) = line_byte_range(source, anchor_line.saturating_add(delta))
            && let Some(hit) = rendered.range_for_source(range)
        {
            return Some(hit);
        }
    }
    for delta in 1..=ANCHOR_SEARCH_RADIUS {
        if let Some(prev_line) = anchor_line.checked_sub(delta)
            && let Some(range) = line_byte_range(source, prev_line)
            && let Some(hit) = rendered.range_for_source(range)
        {
            return Some(hit);
        }
    }
    None
}

/// Picks the source line in the editor viewport to anchor on when switching
/// `Insert -> Normal`. Prefers the cursor line when it is comfortably visible
/// on screen; otherwise selects the proportional visible line.
pub fn anchor_line_for_editor_viewport(
    visible: Range<usize>,
    cursor_line: usize,
    ratio: f64,
    scroll_y: f64,
    max_y: f64,
) -> usize {
    if scroll_y >= -1.0 || visible.is_empty() {
        return 0;
    }
    let inner = inner_visible_line_range(visible, scroll_y, max_y);
    if inner.contains(&cursor_line) {
        cursor_line
    } else {
        interpolate_line_in_range(inner, ratio)
    }
}

/// Computes a new cursor line inside `visible` when entering `Insert` mode so
/// pressing arrow keys never jumps back to an offscreen cursor (e.g. line 0).
/// Returns `None` when the existing cursor is already comfortably visible.
pub fn target_cursor_line_for_viewport(
    visible: Range<usize>,
    cursor_line: usize,
    ratio: f64,
    scroll_y: f64,
    max_y: f64,
) -> Option<usize> {
    if visible.is_empty() {
        return None;
    }
    let inner = inner_visible_line_range(visible, scroll_y, max_y);
    if inner.contains(&cursor_line) {
        None
    } else {
        Some(interpolate_line_in_range(inner, ratio))
    }
}

/// Computes a clamped vertical scroll offset that brings `line_bounds` into
/// `viewport` at the proportional anchor position matching `anchor_ratio`.
pub fn reveal_scroll_target_y(
    line_bounds: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    current_offset_y: Pixels,
    max_y: Pixels,
    anchor_ratio: f64,
    edge_margin: Pixels,
) -> Pixels {
    let usable_height =
        (viewport.size.height - edge_margin * 2.0 - line_bounds.size.height).max(Pixels::ZERO);
    let desired_top =
        viewport.origin.y + edge_margin + usable_height * (anchor_ratio.clamp(0.0, 1.0) as f32);
    let delta_y = desired_top - line_bounds.origin.y;
    clamp_scroll_offset(current_offset_y, delta_y, max_y)
}

fn inner_visible_line_range(visible: Range<usize>, scroll_y: f64, max_y: f64) -> Range<usize> {
    if visible.is_empty() {
        return 0..0;
    }
    let mut lo = visible.start;
    let mut hi = visible.end;
    // Skip a potentially clipped top line when scrolled past the top edge.
    if scroll_y < -1.0 && lo + 1 < hi {
        lo += 1;
    }
    // `Textarea::visible_row_range` includes 1 extra trailing row + partial bottom line.
    if (-scroll_y + 1.0) < max_y {
        let trim = (hi.saturating_sub(lo)).saturating_sub(1).min(2);
        hi = hi.saturating_sub(trim);
    }
    if lo >= hi {
        visible.start..visible.end.max(visible.start + 1)
    } else {
        lo..hi
    }
}

fn interpolate_line_in_range(range: Range<usize>, ratio: f64) -> usize {
    if range.is_empty() {
        return 0;
    }
    let span = range.end.saturating_sub(range.start + 1);
    let offset = (ratio.clamp(0.0, 1.0) * span as f64).round() as usize;
    (range.start + offset).min(range.end - 1)
}

fn map_weighted_ratio(source: &str, ratio: f64, editor_to_preview: bool) -> f64 {
    let clamped = ratio.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return 0.0;
    }
    if clamped >= 1.0 || source.is_empty() {
        return clamped;
    }

    let weights = compute_line_weights(source);
    let total_from: f64 = weights
        .iter()
        .map(|&(raw, rendered)| if editor_to_preview { raw } else { rendered })
        .sum();
    let total_to: f64 = weights
        .iter()
        .map(|&(raw, rendered)| if editor_to_preview { rendered } else { raw })
        .sum();
    if total_from <= 0.0 || total_to <= 0.0 {
        return clamped;
    }

    let target_from = clamped * total_from;
    let mut acc_from = 0.0;
    let mut acc_to = 0.0;
    for &(raw, rendered) in &weights {
        let (w_from, w_to) = if editor_to_preview {
            (raw, rendered)
        } else {
            (rendered, raw)
        };
        if acc_from + w_from >= target_from {
            let frac = if w_from > 0.0 {
                ((target_from - acc_from) / w_from).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return ((acc_to + frac * w_to) / total_to).clamp(0.0, 1.0);
        }
        acc_from += w_from;
        acc_to += w_to;
    }
    1.0
}

fn compute_line_weights(source: &str) -> Vec<(f64, f64)> {
    let mut weights = Vec::new();
    let mut in_fence: Option<char> = None;
    let mut prev_blank = false;

    for raw_line in source.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let raw_chars = line.chars().count();
        let raw_h = ((raw_chars.max(1) as f64) / RAW_WRAP_COLS).ceil().max(1.0);
        let trimmed = line.trim();

        if let Some(fence_ch) = in_fence {
            if is_fence_delimiter(trimmed, Some(fence_ch)) {
                in_fence = None;
                weights.push((raw_h, 0.55));
            } else {
                let code_h = ((raw_chars.max(1) as f64) / CODE_WRAP_COLS).ceil().max(1.0) * 0.92;
                weights.push((raw_h, code_h));
            }
            prev_blank = false;
            continue;
        }

        if let Some(fence_ch) = opening_fence_char(trimmed) {
            in_fence = Some(fence_ch);
            prev_blank = false;
            weights.push((raw_h, 0.55));
            continue;
        }

        if trimmed.is_empty() {
            let rendered_h = if prev_blank { 0.0 } else { 0.55 };
            prev_blank = true;
            weights.push((raw_h, rendered_h));
            continue;
        }
        prev_blank = false;

        let rendered_h = if is_horizontal_rule(trimmed) {
            1.4
        } else if let Some((level, heading_text)) = parse_atx_heading(trimmed) {
            let visible = visible_markdown_chars(heading_text);
            let cols = match level {
                1 => 36.0,
                2 => 48.0,
                3 => 58.0,
                _ => 68.0,
            };
            let lines = ((visible.max(1) as f64) / cols).ceil().max(1.0);
            match level {
                1 => lines * 2.5 + 0.85,
                2 => lines * 1.95 + 0.65,
                3 => lines * 1.55 + 0.45,
                _ => lines * 1.3 + 0.3,
            }
        } else if is_table_separator(trimmed) {
            0.0
        } else if is_table_row(trimmed) {
            1.65
        } else {
            let body = strip_block_prefix(trimmed);
            let visible = visible_markdown_chars(body);
            ((visible.max(1) as f64) / RENDERED_WRAP_COLS)
                .ceil()
                .max(1.0)
        };

        weights.push((raw_h, rendered_h));
    }

    weights
}

fn opening_fence_char(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn is_fence_delimiter(trimmed: &str, fence: Option<char>) -> bool {
    let Some(ch) = fence else {
        return false;
    };
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    count >= 3 && trimmed[count..].trim().is_empty()
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 3 {
        return false;
    }
    let first = compact.chars().next().unwrap_or_default();
    matches!(first, '-' | '*' | '_') && compact.chars().all(|c| c == first)
}

fn parse_atx_heading(trimmed: &str) -> Option<(usize, &str)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
        Some((hashes, rest.trim()))
    } else {
        None
    }
}

fn is_table_separator(trimmed: &str) -> bool {
    trimmed.contains('|')
        && trimmed.contains('-')
        && trimmed
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

fn is_table_row(trimmed: &str) -> bool {
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() >= 2
}

fn strip_block_prefix(mut line: &str) -> &str {
    while let Some(rest) = line.strip_prefix('>') {
        line = rest.trim_start();
    }
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return rest.trim_start();
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && let Some(rest) = line[digits..]
            .strip_prefix(". ")
            .or_else(|| line[digits..].strip_prefix(") "))
    {
        return rest.trim_start();
    }
    line
}

/// Approximates rendered character count by dropping hidden link/image destinations
/// in `[label](destination)` syntax.
fn visible_markdown_chars(text: &str) -> usize {
    let mut chars = text.chars().peekable();
    let mut count = 0usize;
    while let Some(ch) = chars.next() {
        if ch == ']' && chars.peek() == Some(&'(') {
            chars.next();
            let mut depth = 1usize;
            for next in chars.by_ref() {
                match next {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            continue;
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, Pixels, point, px, size};

    use super::{
        anchor_line_for_editor_viewport, byte_offset_to_position, clamp_scroll_offset,
        editor_max_scroll_y, line_byte_range, map_editor_ratio_to_preview_ratio,
        map_preview_ratio_to_editor_ratio, needs_scroll_adjustment, reveal_scroll_target_y,
        scroll_ratio, target_cursor_line_for_viewport, target_scroll_y,
    };

    #[test]
    fn clamp_scroll_offset_bounds_within_top_and_max_scroll() {
        assert_eq!(
            clamp_scroll_offset(Pixels::ZERO, px(-48.), px(200.)),
            px(-48.)
        );
        assert_eq!(
            clamp_scroll_offset(px(-180.), px(-48.), px(200.)),
            px(-200.)
        );
        assert_eq!(
            clamp_scroll_offset(px(-20.), px(48.), px(200.)),
            Pixels::ZERO
        );
        assert_eq!(
            clamp_scroll_offset(Pixels::ZERO, px(-48.), Pixels::ZERO),
            Pixels::ZERO
        );
    }

    #[test]
    fn scroll_ratio_and_target_scroll_y_round_trip_and_clamp() {
        let max_editor = editor_max_scroll_y(1000.0, 400.0);
        assert_eq!(max_editor, 600.0);
        assert_eq!(editor_max_scroll_y(300.0, 400.0), 0.0);

        let ratio = scroll_ratio(-300.0, max_editor);
        assert!((ratio - 0.5).abs() < f64::EPSILON);
        assert!((target_scroll_y(ratio, 900.0) - (-450.0)).abs() < f64::EPSILON);

        assert_eq!(scroll_ratio(-50.0, 0.0), 0.0);
        assert_eq!(scroll_ratio(-999.0, 200.0), 1.0);
        assert_eq!(scroll_ratio(20.0, 200.0), 0.0);
        assert_eq!(target_scroll_y(1.5, 200.0), -200.0);
        assert_eq!(target_scroll_y(0.5, 0.0), 0.0);

        assert!(!needs_scroll_adjustment(-120.0, -120.3));
        assert!(needs_scroll_adjustment(0.0, -120.0));
        assert!(needs_scroll_adjustment(-120.0, 0.0));
    }

    #[test]
    fn weighted_markdown_ratio_accounts_for_top_heavy_headings_and_round_trips() {
        let mut doc =
            String::from("# Huge Title\n## Section One\n## Section Two\n# Another Title\n");
        for i in 0..40 {
            doc.push_str(&format!("Plain short line {i}\n"));
        }

        // At 25% of rendered height we are still closer to the top in raw lines
        // because the headings take much more vertical space when rendered.
        let editor_ratio = map_preview_ratio_to_editor_ratio(&doc, 0.25);
        assert!(
            editor_ratio < 0.22,
            "expected editor ratio < 0.22, got {editor_ratio}"
        );

        let round_tripped = map_editor_ratio_to_preview_ratio(&doc, editor_ratio);
        assert!(
            (round_tripped - 0.25).abs() < 0.02,
            "expected round trip near 0.25, got {round_tripped}"
        );

        assert_eq!(map_preview_ratio_to_editor_ratio(&doc, 0.0), 0.0);
        assert_eq!(map_preview_ratio_to_editor_ratio(&doc, 1.0), 1.0);
        assert_eq!(map_editor_ratio_to_preview_ratio(&doc, 0.0), 0.0);
        assert_eq!(map_editor_ratio_to_preview_ratio(&doc, 1.0), 1.0);
    }

    #[test]
    fn target_cursor_line_moves_offscreen_cursor_into_viewport_and_preserves_visible_cursor() {
        // Cursor at line 0 while scrolled to lines 100..132 moves inside viewport.
        let moved = target_cursor_line_for_viewport(100..132, 0, 0.5, -2400.0, 4800.0);
        assert!(matches!(moved, Some(line) if (101..130).contains(&line)));

        // Cursor already comfortably inside viewport is preserved untouched.
        assert_eq!(
            target_cursor_line_for_viewport(100..132, 115, 0.5, -2400.0, 4800.0),
            None
        );

        // At top of document with cursor offscreen below, move cursor to line 0.
        assert_eq!(
            target_cursor_line_for_viewport(0..32, 80, 0.0, 0.0, 4800.0),
            Some(0)
        );

        // Anchor line prefers visible cursor when leaving Insert mode.
        assert_eq!(
            anchor_line_for_editor_viewport(100..132, 118, 0.5, -2400.0, 4800.0),
            118
        );
        assert_eq!(
            anchor_line_for_editor_viewport(0..32, 10, 0.0, 0.0, 4800.0),
            0
        );
    }

    #[test]
    fn line_byte_range_and_byte_offset_to_position_handle_crlf_and_utf8() {
        let text = "first\r\nhalo 🌍\nlast";
        assert_eq!(line_byte_range(text, 0), Some(0..5));
        assert_eq!(line_byte_range(text, 1), Some(7..16));
        assert_eq!(line_byte_range(text, 2), Some(17..21));
        assert_eq!(line_byte_range(text, 3), None);

        let pos = byte_offset_to_position(text, 16);
        assert_eq!(pos.line, 1);
        // "halo " (5 UTF-16) + 🌍 (2 UTF-16 surrogate pair) = 7
        assert_eq!(pos.character, 7);
    }

    #[test]
    fn reveal_scroll_target_aligns_line_inside_viewport_and_clamps() {
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
        let line = Bounds::new(point(px(0.), px(900.)), size(px(800.), px(24.)));

        // At ratio 0.0 (top-aligned with 32px margin), delta = 32 - 900 = -868.
        let target = reveal_scroll_target_y(line, viewport, px(-100.), px(2000.), 0.0, px(32.));
        assert_eq!(target, px(-968.));

        // Clamps to -max_y when target would exceed bottom.
        let clamped = reveal_scroll_target_y(line, viewport, px(-1500.), px(2000.), 0.0, px(32.));
        assert_eq!(clamped, px(-2000.));
    }
}
