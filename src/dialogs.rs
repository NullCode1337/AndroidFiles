//! Modal dialogs: settings, create/rename, delete confirmation, pull, properties.

use std::cell::Cell;
use std::path::PathBuf;

use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::*;

use crate::app::AppView;
use crate::appearance;
use crate::format::{format_date, format_mode, format_size};
use crate::model::{EntryKind, Side};
use crate::settings::{ThemePref, WindowControlsPref};

/// Kept as an entity so its controlled switches re-render on toggle.
pub struct SettingsView {
    app: WeakEntity<AppView>,
    adb_input: Entity<InputState>,
}

impl SettingsView {
    pub fn new(app: WeakEntity<AppView>, adb_input: Entity<InputState>) -> Self {
        Self { app, adb_input }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.weak_entity();
        let app = self.app.clone();
        let settings = self
            .app
            .read_with(cx, |a, _| a.settings.clone())
            .unwrap_or_default();
        let resolved_adb = self
            .app
            .read_with(cx, |a, _| a.adb_path.clone())
            .ok()
            .flatten();
        let adb_input = self.adb_input.clone();
        let muted = cx.theme().muted_foreground;

        v_flex()
            .gap_3()
            .child(settings_row(
                "Theme",
                h_flex()
                    .gap_1()
                    .child(theme_button(
                        settings.theme == ThemePref::System,
                        "System",
                        ThemePref::System,
                        app.clone(),
                    ))
                    .child(theme_button(
                        settings.theme == ThemePref::Light,
                        "Light",
                        ThemePref::Light,
                        app.clone(),
                    ))
                    .child(theme_button(
                        settings.theme == ThemePref::Dark,
                        "Dark",
                        ThemePref::Dark,
                        app.clone(),
                    )),
            ))
            .child(settings_row(
                "Window controls",
                h_flex()
                    .gap_1()
                    .child(window_controls_button(
                        "Auto",
                        WindowControlsPref::Auto,
                        settings.window_controls,
                        app.clone(),
                        this.clone(),
                    ))
                    .child(window_controls_button(
                        "Left",
                        WindowControlsPref::Left,
                        settings.window_controls,
                        app.clone(),
                        this.clone(),
                    ))
                    .child(window_controls_button(
                        "Right",
                        WindowControlsPref::Right,
                        settings.window_controls,
                        app.clone(),
                        this.clone(),
                    )),
            ))
            .child(settings_row(
                "Translucent window",
                Switch::new("settings-translucent")
                    .checked(settings.translucent_window)
                    .on_change({
                        let app = app.clone();
                        let this = this.clone();
                        move |checked, window, cx| {
                            app.update(cx, |app, cx| {
                                app.settings.translucent_window = *checked;
                                let _ = app.settings.save();
                                app.apply_theme(window, cx);
                            })
                            .ok();
                            this.update(cx, |_, cx| cx.notify()).ok();
                        }
                    }),
            ))
            .child(settings_row(
                "Show hidden files",
                Switch::new("settings-hidden")
                    .checked(settings.show_hidden)
                    .on_change({
                        let app = app.clone();
                        let this = this.clone();
                        move |checked, _window, cx| {
                            app.update(cx, |app, cx| {
                                app.settings.show_hidden = *checked;
                                let _ = app.settings.save();
                                cx.notify();
                            })
                            .ok();
                            this.update(cx, |_, cx| cx.notify()).ok();
                        }
                    }),
            ))
            .child(settings_row(
                "Single-click to open",
                Switch::new("settings-single-click")
                    .checked(settings.open_on_single_click)
                    .on_change({
                        let app = app.clone();
                        let this = this.clone();
                        move |checked, _window, cx| {
                            app.update(cx, |app, cx| {
                                app.settings.open_on_single_click = *checked;
                                let _ = app.settings.save();
                                cx.notify();
                            })
                            .ok();
                            this.update(cx, |_, cx| cx.notify()).ok();
                        }
                    }),
            ))
            .child(settings_row(
                "Confirm before delete",
                Switch::new("settings-confirm-delete")
                    .checked(settings.confirm_before_delete)
                    .on_change({
                        let app = app.clone();
                        let this = this.clone();
                        move |checked, _window, cx| {
                            app.update(cx, |app, cx| {
                                app.settings.confirm_before_delete = *checked;
                                let _ = app.settings.save();
                                cx.notify();
                            })
                            .ok();
                            this.update(cx, |_, cx| cx.notify()).ok();
                        }
                    }),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_sm().child("adb path"))
                    .child(Input::new(&adb_input))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(match &resolved_adb {
                                Some(p) => format!("Resolved: {p}"),
                                None => "adb not found — install platform-tools or set a path."
                                    .to_string(),
                            }),
                    ),
            )
            .child(
                h_flex().justify_end().gap_2().pt_2().child(
                    Button::new("settings-done")
                        .primary()
                        .label("Done")
                        .on_click({
                            let app = app.clone();
                            let adb_input = adb_input.clone();
                            move |_, window, cx| {
                                let raw = adb_input.read(cx).value().to_string();
                                let path = if raw.trim().is_empty() {
                                    None
                                } else {
                                    Some(raw.trim().to_string())
                                };
                                app.update(cx, |app, cx| app.set_adb_path(path, cx)).ok();
                                window.close_dialog(cx);
                            }
                        }),
                ),
            )
    }
}

fn settings_row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().text_sm().child(label))
        .child(control)
}

fn theme_button(active: bool, label: &str, pref: ThemePref, app: WeakEntity<AppView>) -> Button {
    let button = Button::new(format!("settings-theme-{}", label.to_lowercase())).label(label);
    let button = if active {
        button.primary()
    } else {
        button.ghost()
    };
    button.on_click(move |_, window, cx| {
        app.update(cx, |app, cx| {
            app.settings.theme = pref;
            let _ = app.settings.save();
            app.apply_theme(window, cx);
        })
        .ok();
    })
}

fn window_controls_button(
    label: &'static str,
    pref: WindowControlsPref,
    current: WindowControlsPref,
    app: WeakEntity<AppView>,
    view: WeakEntity<SettingsView>,
) -> Button {
    let button =
        Button::new(format!("settings-window-controls-{}", label.to_lowercase())).label(label);
    let button = if current == pref {
        button.primary()
    } else {
        button.ghost()
    };
    button.on_click(move |_, _window, cx| {
        app.update(cx, |app, cx| {
            app.settings.window_controls = pref;
            let _ = app.settings.save();
            cx.notify();
        })
        .ok();
        view.update(cx, |_, cx| cx.notify()).ok();
    })
}

impl AppView {
    pub fn open_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.weak_entity();
        let default = self.settings.adb_path.clone().unwrap_or_default();
        let adb_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Auto-detect")
                .default_value(default)
        });
        let view = cx.new(|_cx| SettingsView::new(app, adb_input));
        window.open_dialog(cx, move |dialog, window, _cx| {
            dialog
                .title("Settings")
                .width(appearance::dialog_width(window.viewport_size().width))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(view.clone())
        });
    }

    pub fn open_new_folder_dialog(
        &mut self,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Folder name"));
        let initial_focus = Cell::new(true);
        let this = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                let handle = input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            }
            dialog
                .title(match side {
                    Side::Host => "New folder on this computer",
                    Side::Device => "New folder on device",
                })
                .width(appearance::compact_dialog_width(
                    window.viewport_size().width,
                ))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(Input::new(&input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("new-folder-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click({
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("new-folder-create")
                                .primary()
                                .label("Create")
                                .on_click({
                                    let this = this.clone();
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        this.update(cx, |this, cx| {
                                            this.create_entry(side, name, true, cx)
                                        })
                                        .ok();
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_new_file_dialog(
        &mut self,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("File name"));
        let initial_focus = Cell::new(true);
        let this = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                let handle = input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            }
            dialog
                .title(match side {
                    Side::Host => "New file on this computer",
                    Side::Device => "New file on device",
                })
                .width(appearance::compact_dialog_width(
                    window.viewport_size().width,
                ))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(Input::new(&input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("new-file-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click({
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("new-file-create")
                                .primary()
                                .label("Create")
                                .on_click({
                                    let this = this.clone();
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        this.update(cx, |this, cx| {
                                            this.create_entry(side, name, false, cx)
                                        })
                                        .ok();
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_rename_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.focused_side;
        let current = self
            .single_selected_entry(side)
            .map(|e| e.name)
            .unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("New name")
                .default_value(current)
        });
        let initial_focus = Cell::new(true);
        let this = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                let handle = input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            }
            dialog
                .title("Rename")
                .width(appearance::compact_dialog_width(
                    window.viewport_size().width,
                ))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(Input::new(&input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("rename-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click({
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("rename-ok")
                                .primary()
                                .label("Rename")
                                .on_click({
                                    let this = this.clone();
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        let name = input.read(cx).value().to_string();
                                        this.update(cx, |this, cx| {
                                            this.rename_entry(side, name, cx)
                                        })
                                        .ok();
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_delete_confirm(
        &mut self,
        side: Side,
        paths: Vec<String>,
        names: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = paths.len();
        let preview = if count <= 5 {
            names.join(", ")
        } else {
            format!(
                "{} items including {}",
                count,
                names.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            )
        };
        let this = cx.weak_entity();
        window.open_alert_dialog(cx, move |alert, window, _cx| {
            alert
                .title(format!(
                    "Delete {count} item{}?",
                    if count == 1 { "" } else { "s" }
                ))
                .width(appearance::compact_dialog_width(
                    window.viewport_size().width,
                ))
                .description(format!("This will permanently delete: {preview}"))
                .confirm()
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .cancel_text("Cancel")
                .on_ok({
                    let this = this.clone();
                    let paths = paths.clone();
                    move |_, _window, cx| {
                        this.update(cx, |this, cx| this.perform_delete(side, paths.clone(), cx))
                            .ok();
                        true
                    }
                })
        });
    }

    pub fn open_properties_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.focused_side;
        let Some(entry) = self.single_selected_entry(side) else {
            return;
        };
        let kind = match &entry.kind {
            EntryKind::File => "File".to_string(),
            EntryKind::Dir => "Folder".to_string(),
            EntryKind::Symlink {
                target,
                resolves_to_dir,
            } => {
                format!(
                    "Symlink → {target}{}",
                    if *resolves_to_dir { " (to folder)" } else { "" }
                )
            }
            EntryKind::Other => "Other".to_string(),
        };
        let rows = [
            ("Name", entry.name.clone()),
            ("Path", entry.path.clone()),
            ("Type", kind),
            ("Size", format_size(entry.size)),
            ("Modified", format_date(entry.modified)),
            ("Mode", format_mode(entry.mode)),
            (
                "Owner",
                entry.owner.clone().unwrap_or_else(|| "—".to_string()),
            ),
            (
                "Hidden",
                if entry.hidden { "yes" } else { "no" }.to_string(),
            ),
        ];
        // Read-only inputs keep values selectable without an editable form.
        let fields = rows
            .iter()
            .map(|(label, value)| {
                let value = value.clone();
                if *label == "Path" {
                    PropField::Wrapped(cx.new(|cx| {
                        TextareaState::new(window, cx)
                            .default_value(value)
                            .auto_grow(1, 5)
                    }))
                } else {
                    PropField::Text(cx.new(|cx| InputState::new(window, cx).default_value(value)))
                }
            })
            .collect::<Vec<_>>();
        window.open_dialog(cx, move |dialog, window, _cx| {
            dialog
                .title("Properties")
                .width(appearance::dialog_width(window.viewport_size().width))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(
                    v_flex().gap_1().children(
                        rows.iter()
                            .zip(fields.iter())
                            .map(|((label, _), field)| prop_row(label, field.clone())),
                    ),
                )
                .footer(
                    h_flex().justify_end().child(
                        Button::new("props-close")
                            .primary()
                            .label("Close")
                            .on_click({
                                move |_, window, cx| {
                                    window.close_dialog(cx);
                                }
                            }),
                    ),
                )
        });
    }

    pub fn open_pull_target_dialog(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let default = self.settings.default_download_folder.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Destination folder")
                .default_value(default)
        });
        let initial_focus = Cell::new(true);
        let this = cx.weak_entity();
        let path = path.to_string();
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                let handle = input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            }
            dialog
                .title("Pull to…")
                .width(appearance::dialog_width(window.viewport_size().width))
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .child(Input::new(&input))
                .footer(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("pull-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click({
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(Button::new("pull-ok").primary().label("Pull").on_click({
                            let this = this.clone();
                            let input = input.clone();
                            let path = path.clone();
                            move |_, window, cx| {
                                let dest = input.read(cx).value().to_string();
                                this.update(cx, |this, cx| {
                                    this.pull_paths_to(vec![path.clone()], PathBuf::from(dest), cx);
                                })
                                .ok();
                                window.close_dialog(cx);
                            }
                        })),
                )
        });
    }
}

#[derive(Clone)]
enum PropField {
    Text(Entity<InputState>),
    Wrapped(Entity<TextareaState>),
}

fn prop_row(label: &'static str, field: PropField) -> impl IntoElement {
    let row = h_flex().w_full().gap_2();
    let label = div().w_24().flex_none().text_sm().child(label);
    match field {
        PropField::Text(input) => row.items_center().child(label).child(
            div().flex_1().min_w_0().child(
                Input::new(&input)
                    .readonly(true)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .small(),
            ),
        ),
        PropField::Wrapped(input) => row.items_start().child(label.pt(px(3.))).child(
            div().flex_1().min_w_0().child(
                Textarea::new(&input)
                    .readonly(true)
                    .appearance(false)
                    .bordered(false)
                    .small(),
            ),
        ),
    }
}
