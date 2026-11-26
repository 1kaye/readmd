mod document;
mod keymap;
mod theme;

use std::path::PathBuf;

use gpui_kit::{
    AppContext as _, Bounds, Pixels, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};

// Physical window sizing exceptions (platform window geometry, not UI layout).
const INITIAL_WINDOW_WIDTH: Pixels = px(960.);
const INITIAL_WINDOW_HEIGHT: Pixels = px(720.);
const MIN_WINDOW_WIDTH: Pixels = px(360.);
const MIN_WINDOW_HEIGHT: Pixels = px(240.);

fn main() {
    let initial_path = std::env::args_os().nth(1).map(PathBuf::from);

    gpui_kit::application().run(move |cx| {
        gpui_kit::base::init(cx);
        theme::load_app_fonts(cx);
        theme::init(cx);
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
            titlebar: Some(TitlebarOptions {
                title: Some(document::APP_TITLE.into()),
                ..Default::default()
            }),
            // Matches StartupWMClass=readmd in readmd.desktop on Wayland/Hyprland.
            app_id: Some(document::APP_TITLE.into()),
            ..WindowOptions::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            window.set_window_title(document::APP_TITLE);
            cx.new(|cx| document::DocumentWorkspace::new(initial_path.clone(), window, cx))
        })
        .expect("Failed to open readmd window");
        cx.activate(true);
    });
}
