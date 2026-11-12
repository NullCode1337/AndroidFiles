//! FOR DEBUGGING ONLY; NOT USED IN MAIN CODE
//! Offscreen rendering of the real app view. Frames are binary PPMs under
//! `/tmp` to avoid an image-encoder dependency; see `AGENTS.md`.

use std::io::Write;
use std::sync::Arc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::{
    AppContext, Bounds, Focusable as _, HeadlessAppContext, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollEvent,
    ScrollSource, TouchPhase, WindowBounds, WindowOptions, point, px, size,
};

use crate::app::AppView;
use crate::model::Side;
use crate::settings::WindowControlsPref;

#[test]
fn settings_button_opens_dialog() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.settings".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    // Drain one frame so toolbar hitboxes exist.
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render frame");

    // Headless windows report server-side decorations, so no caption buttons.
    let at = point(px(916.), px(26.));
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            }),
            cx,
        );
        assert!(
            window.has_active_dialog(cx),
            "clicking the settings button opens the dialog"
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("click settings");

    // Headless time only moves when advanced; jump past the entrance.
    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render settled dialog");

    let image = cx.capture_screenshot(handle).expect("capture settings");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_settings.ppm").expect("create settings snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

/// Every value sits in a borderless read-only input.
#[test]
fn wireless_device_dialog_opens() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.wireless".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.load_demo_snapshot_state();
        view.settings.wireless_devices = vec!["192.168.1.42:37123".into()];
    });

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.open_wireless_device_dialog(window, cx));
        assert!(
            window.has_active_dialog(cx),
            "the wireless device dialog becomes active"
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render wireless dialog");

    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render settled wireless dialog");

    let image = cx.capture_screenshot(handle).expect("capture wireless");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_wireless.ppm").expect("create wireless snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn pull_error_opens_a_dialog() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, _view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.pull-error".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");

    cx.update_window(handle, |_, window, cx| {
        crate::dialogs::open_pull_errors_dialog(
            window,
            cx,
            &[("photo.jpg".to_string(), "Permission denied".to_string())],
        );
        assert!(
            window.has_active_dialog(cx),
            "a failed pull opens an error dialog"
        );
    })
    .expect("open pull error dialog");
}

#[test]
fn queued_pull_errors_open_on_the_next_frame() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.pull-error-queued".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.pending_pull_errors
            .push(("photo.jpg".into(), "Permission denied".into()));
        view.pending_pull_errors
            .push(("video.mp4".into(), "No such file".into()));
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("draw queued failure");
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        assert!(
            window.has_active_dialog(cx),
            "queued failures surface as one dialog after the frame"
        );
    })
    .expect("draw deferred dialog");
    view.update(&mut cx, |view, _| {
        assert!(
            view.pending_pull_errors.is_empty(),
            "the queue is drained once"
        );
    });
}

#[test]
fn usb_device_has_a_disconnect_confirmation() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.usb-disconnect".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.confirm_disconnect_device(
                "R5CT1234567".to_string(),
                "Pixel 8".to_string(),
                window,
                cx,
            );
        });
        assert!(
            window.has_active_dialog(cx),
            "USB devices get a confirmation too"
        );
    })
    .expect("open USB disconnect confirmation");
}

#[test]
fn pair_wireless_device_dialog_opens() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.pair-wireless".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.open_pair_wireless_device_dialog(window, cx)
        });
        assert!(
            window.has_active_dialog(cx),
            "the pairing dialog becomes active"
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render pairing dialog");

    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render settled pairing dialog");

    let image = cx.capture_screenshot(handle).expect("capture pairing");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_pair_wireless.ppm")
        .expect("create pairing snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn properties_dialog_text_is_copyable() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.properties".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_properties_demo_state());

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.open_properties_dialog(window, cx));
        assert!(
            window.has_active_dialog(cx),
            "the properties dialog becomes active"
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render properties dialog");

    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render settled dialog");

    let image = cx.capture_screenshot(handle).expect("capture properties");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_properties.ppm")
        .expect("create properties snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn ctrl_shift_n_opens_new_folder_dialog() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.new-folder-key".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        let focus = view.read(cx).search_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_keystroke(Keystroke::parse("ctrl-shift-n").unwrap(), cx);
        assert!(
            window.has_active_dialog(cx),
            "ctrl-shift-n should open the new folder dialog"
        );
    })
    .expect("dispatch ctrl-shift-n");
}

#[test]
fn enter_creates_the_folder_from_the_dialog() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.new-folder-enter".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");

    let dir =
        std::env::temp_dir().join(format!("androidfiles-enter-folder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    view.update(&mut cx, |view, _| {
        view.load_host_only_demo_state();
        view.host_cwd = dir.clone();
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        let focus = view.read(cx).search_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_keystroke(Keystroke::parse("ctrl-shift-n").unwrap(), cx);
        assert!(window.has_active_dialog(cx), "new folder dialog opens");
        // Let the dialog render so its input takes initial focus.
        window.refresh();
        window.draw(cx).clear(cx);
        for ch in ["c", "r", "e", "a", "t", "e", "d"] {
            window.dispatch_keystroke(
                Keystroke {
                    modifiers: Modifiers::default(),
                    key: ch.to_string(),
                    key_char: Some(ch.to_string()),
                },
                cx,
            );
        }
        window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
        assert!(
            !window.has_active_dialog(cx),
            "Enter submits the dialog instead of only closing it"
        );
    })
    .expect("dispatch Enter in new folder dialog");

    assert!(
        dir.join("created").is_dir(),
        "Enter creates the folder named in the input"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn enter_runs_wireless_validation_instead_of_closing() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.wireless-enter".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.open_wireless_device_dialog(window, cx));
        window.refresh();
        window.draw(cx).clear(cx);
        window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
        assert!(
            window.has_active_dialog(cx),
            "an empty address reports an error and keeps the dialog open"
        );
    })
    .expect("dispatch Enter in wireless dialog");
}

/// The delete key belongs to the search field while it is focused.
#[test]
fn delete_key_does_not_reach_file_actions_from_search() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.delete-key".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        let focus = view.read(cx).search_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_keystroke(Keystroke::parse("delete").unwrap(), cx);
        assert!(
            !window.has_active_dialog(cx),
            "delete in the search field must not delete the selected file"
        );
    })
    .expect("dispatch delete");
}

#[test]
fn sidebar_handles_usb_and_wireless_devices() {
    use crate::model::{DeviceInfo, DeviceStateInfo};

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.sidebar-devices".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.load_demo_snapshot_state();
        let serial = "adb-RFAX41Q89WZ-GDeqAX._adb-tls-connect._tcp";
        view.devices = vec![
            DeviceInfo {
                serial: serial.into(),
                model: "SM-R940".into(),
                state: DeviceStateInfo::Connected,
                wireless_serial: Some(serial.into()),
            },
            DeviceInfo {
                serial: "emulator-5554".into(),
                model: "Pixel 8".into(),
                state: DeviceStateInfo::Connected,
                wireless_serial: None,
            },
        ];
        view.selected_device = Some(serial.into());
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render sidebar");

    let image = cx.capture_screenshot(handle).expect("capture sidebar");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_sidebar_devices.ppm")
        .expect("create sidebar snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn panes_show_scrollbars_for_overflowing_lists() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.scrollbar".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.load_large_demo_state(200);
        view.show_host_pane = true;
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        view.update(cx, |view, _| {
            view.device_scroll.set_offset(point(px(0.), px(-80.)));
        });
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("scroll the device pane");

    cx.advance_clock(std::time::Duration::from_millis(300));
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render scrollbar");

    let image = cx.capture_screenshot(handle).expect("capture scrollbar");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_scrollbar.ppm")
        .expect("create scrollbar snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn scrcpy_refuses_without_a_connected_device() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.scrcpy".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_host_only_demo_state());
    view.update(&mut cx, |view, cx| view.launch_scrcpy(cx));

    let status = cx.update(|cx| view.read(cx).status_line.clone());
    assert_eq!(
        status.as_deref(),
        Some("Connect a device before starting screen mirroring.")
    );
}

#[test]
fn titlebar_buttons_do_not_start_a_drag() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.titlebar".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render frame");

    // Frame listeners live on the drawn frame, so draw before dispatching.
    let press = |cx: &mut HeadlessAppContext, at, expect_drag: bool| {
        cx.update_window(handle, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    position: at,
                    modifiers: Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            assert_eq!(
                view.read(cx).is_titlebar_dragging(),
                expect_drag,
                "press at {at:?}"
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    position: at,
                    modifiers: Modifiers::none(),
                    button: MouseButton::Left,
                    click_count: 1,
                }),
                cx,
            );
        })
        .expect("dispatch press");
    };

    // Empty title bar space starts a window drag.
    press(&mut cx, point(px(400.), px(26.)), true);
    // Checked last because its release opens a dialog over the bar.
    press(&mut cx, point(px(916.), px(26.)), false);
}

/// The empty device pane must not blank the host listing.
#[test]
fn view_mode_round_trips_in_settings() {
    use crate::settings::{Settings, ViewMode};

    let defaults = Settings::default();
    assert_eq!(defaults.view_mode, ViewMode::Grid);
    assert!(!defaults.show_host_pane);
    assert!(!defaults.host_only);

    let settings = Settings {
        view_mode: ViewMode::List,
        show_host_pane: true,
        ..Default::default()
    };
    let text = serde_json::to_string(&settings).expect("serialize settings");
    let loaded: Settings = serde_json::from_str(&text).expect("load settings");
    assert_eq!(loaded.view_mode, ViewMode::List);
    assert!(loaded.show_host_pane);

    // Older files without the fields keep the defaults.
    let legacy: Settings = serde_json::from_str("{\"version\":2}").expect("legacy settings");
    assert_eq!(legacy.view_mode, ViewMode::Grid);
    assert!(!legacy.show_host_pane);
    assert!(!legacy.host_only);
}

#[test]
fn favorites_can_be_reordered() {
    use crate::model::{Favorite, reorder_favorites};

    let mut favorites = vec![
        Favorite {
            device_serial: "DEMO123".into(),
            path: "/sdcard/A".into(),
            name: "A".into(),
        },
        Favorite {
            device_serial: "DEMO123".into(),
            path: "/sdcard/B".into(),
            name: "B".into(),
        },
        Favorite {
            device_serial: "DEMO123".into(),
            path: "/sdcard/C".into(),
            name: "C".into(),
        },
    ];

    assert!(reorder_favorites(&mut favorites, 0, 2));
    let names: Vec<_> = favorites
        .iter()
        .map(|favorite| favorite.name.as_str())
        .collect();
    assert_eq!(names, ["B", "C", "A"]);
    assert!(!reorder_favorites(&mut favorites, 2, 2));
}

#[test]
fn empty_paste_shows_a_toast() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.empty-paste".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| view.paste(window, cx));
        assert!(
            !window.notifications(cx).is_empty(),
            "an empty paste shows a toast"
        );
    })
    .expect("paste with an empty clipboard");
}

#[test]
fn blank_space_click_clears_selection() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.blank-click".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();
        view.selected.clear();
        view.selected.insert("/sdcard/Pictures".into());
        view.focused_side = Side::Device;
        view.clear_selection(Side::Host, cx);
        assert!(view.selected.is_empty());
        assert_eq!(view.focused_side, Side::Host);
    });
}

#[cfg(unix)]
#[test]
fn host_mount_points_include_root() {
    let mounts = crate::backend::host::mount_points();
    assert!(mounts.iter().any(|path| path == std::path::Path::new("/")));
}

#[test]
fn root_shortcut_follows_the_focused_pane() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.path-root".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();

        view.focused_side = Side::Host;
        view.host_cwd = std::path::PathBuf::from("/home/demo");
        view.open_root(cx);
        assert_eq!(view.host_cwd, std::path::PathBuf::from("/"));
        assert_eq!(view.focused_side, Side::Host);

        view.device_cwd = "/".into();
        view.focused_side = Side::Device;
        view.open_root(cx);
        assert_eq!(view.device_cwd, "/");
        assert_eq!(view.focused_side, Side::Device);

        // A host mount focuses the PC pane and opens it when hidden.
        view.host_only = false;
        view.show_host_pane = false;
        view.focused_side = Side::Device;
        view.open_host_path(std::path::PathBuf::from("/android"), cx);
        assert!(view.show_host_pane);
        assert_eq!(view.focused_side, Side::Host);
        assert_eq!(view.host_cwd, std::path::PathBuf::from("/android"));
    });
}

#[test]
fn this_computer_expands_the_pc_pane_over_a_device() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.host-only".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();
        assert!(!view.host_only);
        assert!(!view.show_host_pane);

        view.activate_host_pane(cx);
        assert!(view.host_only, "PC pane takes the full main area");
        assert!(view.show_host_pane);
        assert_eq!(view.focused_side, Side::Host);

        view.activate_host_pane(cx);
        assert!(!view.host_only, "second activation returns to the device");
        assert!(!view.show_host_pane);
        assert_eq!(view.focused_side, Side::Device);

        // Navigating the device leaves PC-only mode.
        view.activate_host_pane(cx);
        assert!(view.host_only);
        view.navigate_device("/sdcard".into(), cx);
        assert!(!view.host_only);
        assert!(!view.show_host_pane);
        assert_eq!(view.focused_side, Side::Device);
    });
}

#[test]
fn clipboard_keeps_all_selected_items() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.clipboard-items".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();
        view.selected.clear();
        view.selected.insert("/sdcard/Documents".into());
        view.selected.insert("/sdcard/Music".into());
        view.set_clipboard(crate::model::ClipboardOp::Copy, Side::Device, cx);
        let clipboard = view.clipboard.as_ref().expect("clipboard populated");
        let mut names: Vec<_> = clipboard
            .items
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        names.sort();
        assert_eq!(names, ["Documents", "Music"]);
    });
}

#[test]
fn dragging_across_rows_extends_selection() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.drag-select".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();
        view.selected.clear();
        view.begin_drag_select("/sdcard/Documents", Side::Device, false, cx);
        view.extend_drag_select("/sdcard/Music", Side::Device, cx);
        view.extend_drag_select("/sdcard/Pictures", Side::Host, cx);
        assert_eq!(view.selected.len(), 2);
        assert!(view.selected.contains("/sdcard/Documents"));
        assert!(view.selected.contains("/sdcard/Music"));
        assert_eq!(view.select_drag, Some(Side::Device));
        view.end_drag_select(cx);
        assert_eq!(view.select_drag, None);

        // Ctrl-click adds without being undone by the release click.
        view.selected.clear();
        view.begin_drag_select("/sdcard/Documents", Side::Device, true, cx);
        assert!(view.selected.is_empty());
        view.end_drag_select(cx);
        view.select_entry("/sdcard/Documents", Side::Device, true, cx);
        view.select_entry("/sdcard/Music", Side::Device, true, cx);
        assert_eq!(view.selected.len(), 2);

        // Ctrl-sweep commits the anchor on the first move.
        view.selected.clear();
        view.begin_drag_select("/sdcard/Documents", Side::Device, true, cx);
        view.extend_drag_select("/sdcard/Music", Side::Device, cx);
        assert!(view.selected.contains("/sdcard/Documents"));
        assert!(view.selected.contains("/sdcard/Music"));
        view.end_drag_select(cx);

        // A selected row starts a transfer drag, not another sweep.
        view.begin_drag_select("/sdcard/Documents", Side::Device, false, cx);
        assert_eq!(view.select_drag, None);
        assert_eq!(view.selected.len(), 2);
    });
}

#[test]
fn right_click_selects_unselected_rows_and_keeps_multiselection() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.context-select".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, cx| {
        view.load_demo_snapshot_state();
        view.selected.clear();
        view.selected.insert("/sdcard/Documents".into());
        view.selected.insert("/sdcard/Music".into());
        view.select_for_context_menu("/sdcard/Pictures", Side::Device, cx);
        assert_eq!(view.selected.len(), 1);
        assert!(view.selected.contains("/sdcard/Pictures"));

        view.selected.insert("/sdcard/Music".into());
        view.select_for_context_menu("/sdcard/Pictures", Side::Device, cx);
        assert_eq!(view.selected.len(), 2);
        assert!(view.selected.contains("/sdcard/Music"));
    });
}

#[test]
fn finished_transfers_expire() {
    use crate::model::{TransferDirection, TransferProgress};
    use std::time::{Duration, Instant};

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (_handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.transfer-expiry".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.transfers = vec![
            TransferProgress {
                id: 1,
                name: "old".into(),
                direction: TransferDirection::Push,
                bytes_done: 1,
                bytes_total: 1,
                done: true,
                failed: false,
                error: None,
                started: chrono::Utc::now(),
                finished_at: Some(Instant::now() - Duration::from_secs(10)),
            },
            TransferProgress {
                id: 2,
                name: "recent".into(),
                direction: TransferDirection::Push,
                bytes_done: 1,
                bytes_total: 1,
                done: true,
                failed: false,
                error: None,
                started: chrono::Utc::now(),
                finished_at: Some(Instant::now()),
            },
            TransferProgress {
                id: 3,
                name: "running".into(),
                direction: TransferDirection::Push,
                bytes_done: 0,
                bytes_total: 1,
                done: false,
                failed: false,
                error: None,
                started: chrono::Utc::now(),
                finished_at: None,
            },
        ];
        assert!(view.prune_finished_transfers());
        let names: Vec<_> = view.transfers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["recent", "running"]);
        assert!(!view.prune_finished_transfers());
    });
}

#[test]
fn sidebar_scrolls_with_many_transfers() {
    use crate::model::{TransferDirection, TransferProgress};

    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.sidebar-scroll".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| {
        view.load_demo_snapshot_state();
        view.transfers = (0..40)
            .map(|id| TransferProgress {
                id,
                name: format!("item-{id}.jpg"),
                direction: TransferDirection::Push,
                bytes_done: id,
                bytes_total: 100,
                done: false,
                failed: false,
                error: None,
                started: chrono::Utc::now(),
                finished_at: None,
            })
            .collect();
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        window.dispatch_event(
            PlatformInput::Scroll(ScrollEvent {
                position: point(px(100.), px(600.)),
                delta: ScrollDelta::Lines(point(0., -5.)),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
                source: ScrollSource::Pointer,
            }),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("scroll the sidebar");

    let image = cx.capture_screenshot(handle).expect("capture sidebar");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_sidebar_transfers.ppm")
        .expect("create sidebar transfers snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn disconnect_clears_loaded_device_folder() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.disconnected".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());
    view.update(&mut cx, |view, _| {
        assert!(view.visible_len(Side::Device) > 0);
    });

    view.update(&mut cx, |view, cx| {
        view.apply_devices_for_test(Vec::new(), cx)
    });
    view.update(&mut cx, |view, _| {
        assert!(view.selected_device.is_none());
        assert_eq!(view.visible_len(Side::Device), 0);
        assert_eq!(view.current_folder_name(Side::Device), "No device");
    });

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render disconnected state");

    let image = cx.capture_screenshot(handle).expect("capture disconnected");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_disconnected.ppm")
        .expect("create disconnected snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
}

#[test]
fn host_pane_lists_entries_when_device_is_empty() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.host-only".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_host_only_demo_state());

    view.update(&mut cx, |view, _| {
        // It asks first; its empty order must not blank the host pane.
        assert_eq!(view.visible_len(Side::Device), 0);
        assert_eq!(view.visible_len(Side::Host), 1);
        assert_eq!(view.visible_entries(Side::Host)[0].name, "Downloads");
    });
    let _ = handle;
}

/// No global binding may shadow backspace while the search field has focus.
#[test]
fn backspace_edits_the_toolbar_search() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.search".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_host_only_demo_state());

    let key = |key: &str, key_char: Option<&str>| Keystroke {
        modifiers: Modifiers::default(),
        key: key.to_string(),
        key_char: key_char.map(str::to_string),
    };

    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        let focus = view.read(cx).search_input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        for ch in ["a", "b"] {
            window.dispatch_keystroke(key(ch, Some(ch)), cx);
        }
    })
    .expect("type into search");

    let typed = cx.update(|cx| view.read(cx).search_input.read(cx).value().to_string());
    assert_eq!(typed, "ab", "typing reaches the search field");

    cx.update_window(handle, |_, window, cx| {
        window.dispatch_keystroke(key("backspace", None), cx);
    })
    .expect("press backspace");

    let edited = cx.update(|cx| view.read(cx).search_input.read(cx).value().to_string());
    assert_eq!(edited, "a", "backspace deletes in the search field");
}

/// Shortcuts move whichever pane is highlighted.
#[test]
fn sidebar_shortcuts_follow_the_selected_device() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: true,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.locations".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_host_only_demo_state());
    let temp = std::env::temp_dir();

    view.update(&mut cx, |view, cx| {
        view.show_host_pane = true;
        view.focused_side = Side::Host;
        view.open_location("Temporary", "/sdcard/Download", temp.clone(), cx);
        assert_eq!(view.host_cwd, temp, "highlighted host pane navigates");
        assert_eq!(view.focused_side, Side::Host);

        view.focused_side = Side::Device;
        view.open_location("Downloads", "/sdcard/Download", temp.clone(), cx);
        assert_eq!(view.host_cwd, temp, "no-device shortcut targets the host");
        assert!(view.show_host_pane);

        view.load_demo_snapshot_state();
        view.show_host_pane = true;
        view.focused_side = Side::Host;
        view.open_location("Downloads", "/sdcard/Download", temp.clone(), cx);
        assert_eq!(
            view.device_cwd, "/sdcard/Download",
            "a selected device wins over the highlighted host pane"
        );
        assert_eq!(view.focused_side, Side::Device);

        let host = view.host_cwd.clone();
        view.open_location("Pictures", "/sdcard/Pictures", temp, cx);
        assert_eq!(view.device_cwd, "/sdcard/Pictures");
        assert_eq!(
            view.host_cwd, host,
            "device navigation leaves the host pane"
        );
    });
    let _ = handle;
}

/// Run with `--ignored --nocapture`.
#[test]
#[ignore = "render benchmark"]
fn bench_large_folder_render() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: false,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.bench".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_large_demo_state(5_000));

    fn measure(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, label: &str) {
        // Warm up layout caches.
        cx.update_window(handle, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .expect("frame");
        let start = std::time::Instant::now();
        const FRAMES: u32 = 10;
        for _ in 0..FRAMES {
            cx.update_window(handle, |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })
            .expect("frame");
        }
        eprintln!("{label}: {:.2?}/frame", start.elapsed() / FRAMES);
    }

    measure(&mut cx, handle, "grid 5000");
    view.update(&mut cx, |view, _| {
        view.view_mode = crate::app::ViewMode::List
    });
    measure(&mut cx, handle, "list 5000");
    view.update(&mut cx, |view, _| {
        view.selected = view
            .visible_entries(Side::Device)
            .iter()
            .take(2_500)
            .map(|entry| entry.path.clone())
            .collect();
    });
    measure(&mut cx, handle, "list 5000 selected");
}

#[test]
fn render_app_snapshot() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);

    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1120.), px(720.)),
                    })),
                    focus: false,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.app".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open headless window");
    view.update(&mut cx, |view, _| view.load_demo_snapshot_state());

    cx.update_window(handle, |_, window, cx| {
        eprintln!(
            "bounds={:?} viewport={:?} scale={}",
            window.bounds(),
            window.viewport_size(),
            window.scale_factor()
        );
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render frame");

    let image = cx.capture_screenshot(handle).expect("capture frame");
    let (width, height) = image.dimensions();

    let mut out = std::fs::File::create("/tmp/androidfiles.ppm").expect("create snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles.ppm ({width}x{height})");

    // Title bar with the control capsule on the left, as macOS lays it out.
    view.update(&mut cx, |view, _| {
        view.settings.window_controls = WindowControlsPref::Left;
    });
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render left-controls frame");
    let image = cx
        .capture_screenshot(handle)
        .expect("capture left controls");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_left_controls.ppm")
        .expect("create left-controls snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_left_controls.ppm ({width}x{height})");

    // List mode, so both layouts are exercised.
    view.update(&mut cx, |view, _| {
        view.settings.window_controls = WindowControlsPref::Auto;
        view.view_mode = crate::app::ViewMode::List;
    });
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render list frame");
    let image = cx.capture_screenshot(handle).expect("capture list");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_list.ppm").expect("create list snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_list.ppm ({width}x{height})");

    // Third frame: split view with the host pane alongside the device.
    view.update(&mut cx, |view, _| {
        view.view_mode = crate::app::ViewMode::Grid;
        view.show_host_pane = true;
    });
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render split frame");
    let image = cx.capture_screenshot(handle).expect("capture split");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_split.ppm").expect("create split snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_split.ppm ({width}x{height})");

    // The 340px host pane must switch to compact columns.
    view.update(&mut cx, |view, _| {
        view.view_mode = crate::app::ViewMode::List;
        view.show_host_pane = true;
    });
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render split list frame");
    let image = cx.capture_screenshot(handle).expect("capture split list");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_split_list.ppm")
        .expect("create split list snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_split_list.ppm ({width}x{height})");

    // Drive the divider with real pointer events; the panes start even.
    let file_area = 1120.
        - f32::from(crate::appearance::SIDEBAR_WIDTH)
        - f32::from(crate::appearance::PANE_DIVIDER_WIDTH);
    let default_host_width = file_area / 2.;
    let seam = f32::from(crate::appearance::SIDEBAR_WIDTH)
        + default_host_width
        + f32::from(crate::appearance::PANE_DIVIDER_WIDTH) / 2.;
    let (start, end) = (point(px(seam), px(300.)), point(px(seam + 60.), px(300.)));
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: end,
                modifiers: Modifiers::none(),
                pressed_button: Some(MouseButton::Left),
            }),
            cx,
        );
    })
    .expect("dispatch divider drag");
    view.update(&mut cx, |view, _| {
        assert_eq!(
            view.host_pane_width,
            px(default_host_width + 60.),
            "divider drag resizes pane"
        );
        assert!(view.host_pane_custom);
    });
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: end,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            }),
            cx,
        );
    })
    .expect("dispatch divider release");

    view.update(&mut cx, |view, cx| {
        // Divider drag: back to the equal split, then clamp at both ends.
        view.host_pane_custom = false;
        view.begin_pane_resize(500., px(1120.), cx);
        view.update_pane_resize(560., px(1120.), cx);
        assert_eq!(view.host_pane_width, px(default_host_width + 60.));
        view.update_pane_resize(100., px(1120.), cx);
        assert_eq!(view.host_pane_width, crate::appearance::MIN_HOST_PANE_WIDTH);
        view.update_pane_resize(2000., px(1120.), cx);
        assert_eq!(view.host_pane_width, px(1120. - 212. - 5. - 300.));
        view.end_pane_resize(cx);

        // The sidebar row expands the PC pane, then returns to the device.
        view.show_host_pane = false;
        view.host_only = false;
        view.focused_side = crate::model::Side::Device;
        view.activate_host_pane(cx);
        assert!(view.show_host_pane);
        assert!(view.host_only);
        assert_eq!(view.focused_side, crate::model::Side::Host);
        view.activate_host_pane(cx);
        assert!(!view.show_host_pane);
        assert!(!view.host_only);
        assert_eq!(view.focused_side, crate::model::Side::Device);
    });

    // PC pane expanded over an attached device.
    view.update(&mut cx, |view, cx| {
        view.view_mode = crate::app::ViewMode::Grid;
        view.show_host_pane = false;
        view.host_only = false;
        view.focused_side = crate::model::Side::Device;
        view.activate_host_pane(cx);
    });
    cx.update_window(handle, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render host-only frame");
    let image = cx.capture_screenshot(handle).expect("capture host-only");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_host_only.ppm").expect("create snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_host_only.ppm ({width}x{height})");

    // A fresh window: resizing headless does not re-run layout.
    let (narrow, narrow_view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(900.), px(600.)),
                    })),
                    focus: false,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.narrow".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open narrow headless window");
    narrow_view.update(&mut cx, |view, _| {
        view.load_demo_snapshot_state();
        view.view_mode = crate::app::ViewMode::List;
        view.show_host_pane = true;
    });
    cx.update_window(narrow, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render narrow split list frame");
    let image = cx
        .capture_screenshot(narrow)
        .expect("capture narrow split list");
    let (width, height) = image.dimensions();
    let mut out = std::fs::File::create("/tmp/androidfiles_narrow_split_list.ppm")
        .expect("create narrow split list snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_narrow_split_list.ppm ({width}x{height})");

    // The host pane must list its folder, and the no-device card centred.
    let (pairless, pairless_view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1100.), px(700.)),
                    })),
                    focus: false,
                    show: false,
                    window_decorations: Some(gpui_kit::WindowDecorations::Client),
                    app_id: Some("com.androidfiles.no-device".to_string()),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| AppView::new(window, cx)),
            )
        })
        .expect("open no-device headless window");
    pairless_view.update(&mut cx, |view, _| {
        view.load_host_only_demo_state();
        view.show_host_pane = true;
    });
    cx.update_window(pairless, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render no-device frame");
    let image = cx
        .capture_screenshot(pairless)
        .expect("capture no-device frame");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_no_device.ppm").expect("create snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_no_device.ppm ({width}x{height})");

    // Deep host path: crumbs must wrap inside the pane.
    pairless_view.update(&mut cx, |view, _| {
        view.host_cwd = std::path::PathBuf::from(
            "/home/demo/Documents/Projects/AndroidFiles/builds/releases/2026-11-06/nightly-x86_64",
        );
    });
    cx.update_window(pairless, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .expect("render long-path frame");
    let image = cx
        .capture_screenshot(pairless)
        .expect("capture long-path frame");
    let (width, height) = image.dimensions();
    let mut out =
        std::fs::File::create("/tmp/androidfiles_long_path.ppm").expect("create snapshot");
    write!(out, "P6\n{width} {height}\n255\n").expect("header");
    for pixel in image.pixels() {
        out.write_all(&[pixel[0], pixel[1], pixel[2]])
            .expect("pixel");
    }
    out.flush().expect("flush");
    eprintln!("wrote /tmp/androidfiles_long_path.ppm ({width}x{height})");

    // Clicking empty space in the host pane makes it the highlighted pane.
    let at = point(px(500.), px(500.));
    cx.update_window(pairless, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: at,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            }),
            cx,
        );
    })
    .expect("click host pane");
    pairless_view.update(&mut cx, |view, _| {
        assert_eq!(view.focused_side, Side::Host, "clicking a pane focuses it");
    });
}
