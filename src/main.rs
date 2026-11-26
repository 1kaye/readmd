mod document;
mod keymap;
mod theme;

use gpui_kit::{AppContext as _, Bounds, Pixels, WindowBounds, WindowOptions, px, size};

// Physical window sizing exceptions (platform window geometry, not UI layout).
const INITIAL_WINDOW_WIDTH: Pixels = px(960.);
const INITIAL_WINDOW_HEIGHT: Pixels = px(720.);
const MIN_WINDOW_WIDTH: Pixels = px(360.);
const MIN_WINDOW_HEIGHT: Pixels = px(240.);

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        theme::load_app_fonts(cx);
        keymap::init(cx);

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT),
                cx,
            ))),
            window_min_size: Some(size(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)),
            app_id: Some(document::APP_TITLE.to_string()),
            ..WindowOptions::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            window.set_window_title(document::APP_TITLE);
            theme::sync_with_appearance(window.appearance(), cx);
            cx.new(|cx| document::DocumentWorkspace::new(window, cx))
        })
        .expect("Failed to open readmd window");
    });
}
