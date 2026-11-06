//! AndroidFiles — a GPUI file manager for Android devices over adb.

mod actions;
mod app;
mod appearance;
mod backend;
mod dialogs;
mod file_view;
mod format;
mod icons;
mod model;
mod settings;
mod sidebar;

#[cfg(test)]
mod tests;

use gpui_kit::component::TitleBar;
use gpui_kit::*;

fn main() {
    env_logger::init();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);

            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.0), px(720.0)),
                cx,
            )));
            options.window_min_size = Some(size(px(900.0), px(600.0)));
            options.app_id = Some("com.androidfiles.app".to_string());

            // Blur or backdrop where available; the setting can force `Opaque`.
            options.window_background = if cfg!(target_os = "macos") {
                WindowBackgroundAppearance::Blurred
            } else if cfg!(target_os = "windows") {
                WindowBackgroundAppearance::MicaBackdrop
            } else {
                WindowBackgroundAppearance::Transparent
            };

            // macOS keeps its native chrome; other platforms draw their own.
            options.window_decorations = if cfg!(target_os = "macos") {
                None
            } else {
                Some(WindowDecorations::Client)
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::AppView::new(window, cx))
            })
            .expect("failed to open window");
        });
}
