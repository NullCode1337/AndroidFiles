//! The sidebar: shortcuts, favorites, devices, and active transfers.

use std::path::PathBuf;

use chrono::Utc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{AppView, DragGhost, DragPayload};
use crate::appearance;
use crate::format::{device_label, format_size, format_speed};
use crate::model::Side;

impl AppView {
    pub fn render_sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = cx.theme().sidebar;
        let sidebar_fg = cx.theme().sidebar_foreground;
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let primary = cx.theme().primary;
        let drop_bg = cx.theme().drop_target;
        let icon_color: Hsla = rgba(0x5a6570ff).into();
        let active = cx.theme().sidebar_accent;
        let hover = cx.theme().list_hover;
        let this = cx.weak_entity();

        let host_active = self.show_host_pane && self.focused_side == Side::Host;
        let add_wireless = cx.weak_entity();
        let add_wireless_item = div().w_full().flex_none().px(px(8.)).pb(px(8.)).child(
            h_flex()
                .id("add-wireless-device")
                .w_full()
                .h(px(26.))
                .items_center()
                .justify_center()
                .gap(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .on_click(move |_, window, cx| {
                    add_wireless
                        .update(cx, |this, cx| this.open_wireless_device_dialog(window, cx))
                        .ok();
                })
                .child(
                    div()
                        .flex_none()
                        .text_color(primary)
                        .child(Icon::new(IconName::Wifi).size(px(15.))),
                )
                .child(div().text_sm().child("Add wireless device")),
        );
        let devices = (!self.devices.is_empty()).then(|| {
            v_flex()
                .w_full()
                .flex_none()
                .child(section_header("Devices", muted))
                .child(self.render_devices(cx, muted, active, hover))
                .into_any_element()
        });
        let paths = self.selected_device.is_some().then(|| {
            v_flex()
                .w_full()
                .flex_none()
                .child(section_header("Paths", muted))
                .child(self.render_paths(cx, muted, active, hover))
                .into_any_element()
        });

        v_flex()
            .id("sidebar")
            .h_full()
            .w(appearance::SIDEBAR_WIDTH)
            .flex_none()
            .flex_col()
            .border_r_1()
            .border_color(border)
            .bg(sidebar_bg)
            .text_color(sidebar_fg)
            .child(
                v_flex()
                    .w_full()
                    .flex_none()
                    .pt(px(10.))
                    .px(px(8.))
                    .gap(px(1.))
                    .when(self.selected_device.is_some(), {
                        let this = this.clone();
                        move |el| {
                            let drop_payload = this.clone();
                            let drop_external = this.clone();
                            el.child(
                                nav_row(
                                    "nav-internal-storage",
                                    "Internal storage",
                                    IconName::HardDrive,
                                    NavRowColors {
                                        icon: primary,
                                        active,
                                        hover,
                                    },
                                    self.focused_side == Side::Device
                                        && (self.device_cwd == "/sdcard"
                                            || self.device_cwd == "/storage/emulated/0"),
                                    {
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            this.update(cx, |this, cx| {
                                                this.navigate_device("/sdcard".into(), cx)
                                            })
                                            .ok();
                                        }
                                    },
                                )
                                .drag_over::<DragPayload>(move |style, _, _, _| style.bg(drop_bg))
                                .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_bg))
                                .on_drop::<DragPayload>(move |payload, window, cx| {
                                    drop_payload
                                        .update(cx, |this, cx| {
                                            this.handle_internal_drop(
                                                payload,
                                                Side::Device,
                                                "/sdcard".into(),
                                                window,
                                                cx,
                                            )
                                        })
                                        .ok();
                                })
                                .on_drop::<ExternalPaths>(
                                    move |paths, _, cx| {
                                        drop_external
                                            .update(cx, |this, cx| {
                                                this.handle_external_drop(
                                                    paths.paths(),
                                                    Side::Device,
                                                    "/sdcard".into(),
                                                    cx,
                                                )
                                            })
                                            .ok();
                                    },
                                ),
                            )
                        }
                    })
                    .child(nav_row(
                        "nav-this-computer",
                        "This computer",
                        IconName::Laptop,
                        NavRowColors {
                            icon: primary,
                            active,
                            hover,
                        },
                        host_active,
                        {
                            let this = this.clone();
                            move |_, _, cx| {
                                this.update(cx, |this, cx| this.activate_host_pane(cx)).ok();
                            }
                        },
                    )),
            )
            .child(section_header("Favorites", muted))
            .child(self.render_quick_locations(cx, icon_color, active, hover))
            .child(self.render_favorites(cx, icon_color, active, hover))
            .when_some(devices, |el, devices| el.child(devices))
            .when_some(paths, |el, paths| el.child(paths))
            .when(self.devices.is_empty(), move |el| {
                let refresh = this.clone();
                el.child(
                    div()
                        .w_full()
                        .flex_none()
                        .px(px(8.))
                        .pt(px(6.))
                        .child(nav_row(
                            "refresh-devices",
                            "Refresh devices",
                            IconName::RefreshCw,
                            NavRowColors {
                                icon: muted,
                                active,
                                hover,
                            },
                            false,
                            move |_, _, cx| {
                                refresh.update(cx, |this, cx| this.refresh_devices(cx)).ok();
                            },
                        )),
                )
            })
            .when(!self.transfers.is_empty(), move |el| {
                el.child(section_header("Transfers", muted))
                    .child(self.render_transfers(cx))
            })
            .child(div().flex_1().min_h_0())
            .child(add_wireless_item)
            .overflow_y_scrollbar()
            .id("sidebar-scroll")
    }

    fn render_quick_locations(
        &self,
        cx: &mut Context<Self>,
        icon_color: Hsla,
        active: Hsla,
        hover: Hsla,
    ) -> impl IntoElement {
        let this = cx.weak_entity();
        let locations: Vec<(&'static str, &'static str, IconName)> = vec![
            ("DCIM", "/sdcard/DCIM", IconName::Camera),
            ("Documents", "/sdcard/Documents", IconName::FileText),
            ("Downloads", "/sdcard/Download", IconName::ArrowDownToLine),
            ("Movies", "/sdcard/Movies", IconName::Film),
            ("Music", "/sdcard/Music", IconName::Music),
            ("Pictures", "/sdcard/Pictures", IconName::Image),
        ];

        v_flex()
            .w_full()
            .flex_none()
            .px(px(8.))
            .gap(px(1.))
            .children(locations.into_iter().map(move |(label, path, icon)| {
                let this = this.clone();
                let device_path = path.to_string();
                let host_path = host_shortcut_path(label);
                // Active for whichever pane is highlighted in this folder.
                let is_active = if self.selected_device.is_some() {
                    self.device_cwd == path
                } else {
                    self.show_host_pane && self.host_cwd == host_path
                };
                let target = host_shortcut_path(label);
                nav_row(
                    format!("loc-{path}"),
                    label,
                    icon,
                    NavRowColors {
                        icon: icon_color,
                        active,
                        hover,
                    },
                    is_active,
                    move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.open_location(label, &device_path, target.clone(), cx)
                        })
                        .ok();
                    },
                )
            }))
    }

    fn render_favorites(
        &self,
        cx: &mut Context<Self>,
        icon_color: Hsla,
        active: Hsla,
        hover: Hsla,
    ) -> impl IntoElement {
        if self.selected_device.is_none() {
            return v_flex();
        }
        let this = cx.weak_entity();
        let add_this = this.clone();
        let muted = cx.theme().muted_foreground;
        let drop_bg = cx.theme().drop_target;
        let device = self.selected_device.clone();
        let favorites: Vec<_> = self
            .settings
            .favorites
            .iter()
            .enumerate()
            .filter(|(_, favorite)| device.as_deref() == Some(favorite.device_serial.as_str()))
            .collect();

        v_flex()
            .w_full()
            .flex_none()
            .px(px(8.))
            .gap(px(1.))
            .when(!favorites.is_empty(), move |el| {
                el.children(favorites.into_iter().map(move |(index, fav)| {
                    let click = this.clone();
                    let drop = this.clone();
                    let drop_internal = this.clone();
                    let drop_external = this.clone();
                    let serial = fav.device_serial.clone();
                    let path = fav.path.clone();
                    let drop_path_internal = path.clone();
                    let drop_path_external = path.clone();
                    let name = fav.name.clone();
                    let drag_name = name.clone();
                    let is_active = self.focused_side == Side::Device && self.device_cwd == path;
                    nav_row(
                        format!("fav-{serial}-{path}"),
                        name,
                        IconName::Folder,
                        NavRowColors {
                            icon: icon_color,
                            active,
                            hover,
                        },
                        is_active,
                        {
                            let path = path.clone();
                            move |_, _, cx| {
                                click
                                    .update(cx, |this, cx| this.navigate_device(path.clone(), cx))
                                    .ok();
                            }
                        },
                    )
                    .on_drag(FavoriteDrag { index }, move |_, offset, _, cx| {
                        cx.new(|_cx| DragGhost::new(drag_name.clone(), offset))
                    })
                    .on_drop::<FavoriteDrag>(move |payload, _, cx| {
                        drop.update(cx, |this, cx| {
                            this.reorder_favorite(payload.index, index, cx)
                        })
                        .ok();
                    })
                    .drag_over::<DragPayload>(move |style, _, _, _| style.bg(drop_bg))
                    .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_bg))
                    .on_drop::<DragPayload>(move |payload, window, cx| {
                        drop_internal
                            .update(cx, |this, cx| {
                                this.handle_internal_drop(
                                    payload,
                                    Side::Device,
                                    drop_path_internal.clone(),
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                    })
                    .on_drop::<ExternalPaths>(move |paths, _, cx| {
                        drop_external
                            .update(cx, |this, cx| {
                                this.handle_external_drop(
                                    paths.paths(),
                                    Side::Device,
                                    drop_path_external.clone(),
                                    cx,
                                )
                            })
                            .ok();
                    })
                }))
            })
            .child(
                div()
                    .id("add-favorite")
                    .w_full()
                    .h(px(26.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .text_xs()
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click({
                        let this = add_this.clone();
                        move |_, _, cx| {
                            this.update(cx, |this, cx| this.add_current_to_favorites(cx))
                                .ok();
                        }
                    })
                    .child("+ Add current folder"),
            )
    }

    fn render_devices(
        &self,
        cx: &mut Context<Self>,
        muted: Hsla,
        active: Hsla,
        hover: Hsla,
    ) -> impl IntoElement {
        let this = cx.weak_entity();
        let mut rows: Vec<AnyElement> = Vec::new();

        for device in &self.devices {
            let serial = device.serial.clone();
            let label = device_label(&device.model, &device.serial);
            let is_selected = self.selected_device.as_deref() == Some(serial.as_str());
            let wireless = device.wireless_serial.clone();
            let disconnect_name = if device.model.is_empty() {
                serial.clone()
            } else {
                device.model.clone()
            };
            let select = this.clone();
            let select_serial = serial.clone();
            let disconnect = this.clone();

            rows.push(
                h_flex()
                    .id(ElementId::from(format!("device-{}", device.serial)))
                    .w_full()
                    .h(px(26.))
                    .flex_none()
                    .overflow_hidden()
                    .px(px(8.))
                    .gap(px(4.))
                    .rounded(px(6.))
                    .when(is_selected, move |el| el.bg(active))
                    .when(!is_selected, move |el| el.hover(move |s| s.bg(hover)))
                    .child(
                        h_flex()
                            .id(format!("device-select-{serial}"))
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap(px(8.))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                select
                                    .update(cx, |this, cx| {
                                        this.select_device(select_serial.clone(), cx);
                                        this.navigate_device("/sdcard".into(), cx);
                                    })
                                    .ok();
                            })
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(muted)
                                    .child(Icon::new(IconName::Smartphone).size(px(15.))),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_sm()
                                    .child(label),
                            ),
                    )
                    .when_some(wireless, move |el, address| {
                        el.child(
                            Button::new(format!("disconnect-{serial}"))
                                .ghost()
                                .small()
                                .rounded(px(6.))
                                .w(px(22.))
                                .h(px(22.))
                                .icon(Icon::new(IconName::Trash).size(px(13.)))
                                .tooltip("Disconnect")
                                .on_click(move |_, window, cx| {
                                    disconnect
                                        .update(cx, |this, cx| {
                                            this.confirm_disconnect_device(
                                                address.clone(),
                                                disconnect_name.clone(),
                                                window,
                                                cx,
                                            )
                                        })
                                        .ok();
                                }),
                        )
                    })
                    .into_any_element(),
            );
        }

        v_flex()
            .w_full()
            .flex_none()
            .px(px(8.))
            .gap(px(1.))
            .children(rows)
    }

    fn render_paths(
        &self,
        cx: &mut Context<Self>,
        muted: Hsla,
        active: Hsla,
        hover: Hsla,
    ) -> impl IntoElement {
        let this = cx.weak_entity();
        let mut rows: Vec<AnyElement> = Vec::new();

        for (id, label, path) in [
            ("root", "/", "/"),
            ("sdcard", "/sdcard", "/storage/emulated/0"),
        ] {
            let is_active = self.focused_side == Side::Device
                && (self.device_cwd == path
                    || (path == "/storage/emulated/0" && self.device_cwd == "/sdcard"));
            let this = this.clone();
            let target = path.to_string();
            rows.push(
                nav_row(
                    format!("root-{id}"),
                    label,
                    IconName::HardDrive,
                    NavRowColors {
                        icon: muted,
                        active,
                        hover,
                    },
                    is_active,
                    move |_, _, cx| {
                        this.update(cx, |this, cx| this.navigate_device(target.clone(), cx))
                            .ok();
                    },
                )
                .into_any_element(),
            );
        }

        v_flex()
            .w_full()
            .flex_none()
            .px(px(8.))
            .gap(px(1.))
            .children(rows)
    }

    fn render_transfers(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let primary = cx.theme().primary;
        let danger = cx.theme().danger;
        let success = cx.theme().success;

        v_flex()
            .w_full()
            .flex_none()
            .children(self.transfers.iter().map(move |t| {
                let arrow = match t.direction {
                    crate::model::TransferDirection::Push => "↑",
                    crate::model::TransferDirection::Pull => "↓",
                };
                let pct = t.fraction() * 100.0;
                let color = if t.failed {
                    danger
                } else if t.done {
                    success
                } else {
                    primary
                };
                let speed = (!t.done && t.bytes_done > 0).then(|| {
                    let secs = Utc::now()
                        .signed_duration_since(t.started)
                        .num_seconds()
                        .max(1);
                    format_speed(t.bytes_done / secs as u64)
                });
                v_flex()
                    .id(ElementId::from(format!("transfer-{}", t.id)))
                    .w_full()
                    .px(px(16.))
                    .py(px(3.))
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(6.))
                            .child(div().text_xs().text_color(muted).child(arrow))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_xs()
                                    .child(t.name.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div().flex_1().min_w_0().child(
                                    Progress::new(ElementId::from(format!(
                                        "transfer-progress-{}",
                                        t.id
                                    )))
                                    .value(pct)
                                    .color(color),
                                ),
                            )
                            .child(div().text_xs().text_color(muted).child(format!(
                                "{}/{}",
                                format_size(t.bytes_done),
                                format_size(t.bytes_total)
                            ))),
                    )
                    .when_some(speed, |el, s| {
                        el.child(div().text_xs().text_color(muted).child(s))
                    })
                    .when_some(t.error.clone(), move |el, e| {
                        el.child(div().text_xs().text_color(danger).child(e))
                    })
            }))
    }

    fn add_current_to_favorites(&mut self, cx: &mut Context<Self>) {
        let Some(serial) = self.selected_device.clone() else {
            return;
        };
        let path = self.device_cwd.clone();
        let name = if path == "/sdcard" {
            "Internal storage".to_string()
        } else {
            path.rsplit('/').next().unwrap_or(&path).to_string()
        };
        if !self
            .settings
            .favorites
            .iter()
            .any(|f| f.device_serial == serial && f.path == path)
        {
            self.settings.favorites.push(crate::model::Favorite {
                device_serial: serial,
                path,
                name,
            });
            let _ = self.settings.save();
        }
        cx.notify();
    }

    pub(crate) fn reorder_favorite(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if crate::model::reorder_favorites(&mut self.settings.favorites, from, to) {
            let _ = self.settings.save();
            cx.notify();
        }
    }
}

#[derive(Clone)]
struct FavoriteDrag {
    index: usize,
}

#[derive(Clone, Copy)]
struct NavRowColors {
    icon: Hsla,
    active: Hsla,
    hover: Hsla,
}

/// One inset navigation row, shared by shortcuts, favorites, and locations.
fn nav_row(
    id: impl Into<ElementId>,
    label: impl Into<String>,
    icon: IconName,
    colors: NavRowColors,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl StatefulInteractiveElement + IntoElement {
    h_flex()
        .id(id)
        .w_full()
        .h(px(26.))
        .flex_none()
        .px(px(8.))
        .gap(px(8.))
        .rounded(px(6.))
        .cursor_pointer()
        .when(is_active, move |el| el.bg(colors.active))
        .when(!is_active, move |el| el.hover(move |s| s.bg(colors.hover)))
        .on_click(on_click)
        .child(
            div()
                .flex_none()
                .text_color(colors.icon)
                .child(Icon::new(icon).size(px(15.))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_sm()
                .child(label.into()),
        )
}

fn section_header(label: &'static str, muted: Hsla) -> impl IntoElement {
    div()
        .w_full()
        .flex_none()
        .px(px(16.))
        .pt(px(14.))
        .pb(px(4.))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(muted)
        .child(label)
}

/// The host folder matching a device shortcut, for when the host pane is focused.
fn host_shortcut_path(label: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    match label {
        "Downloads" => dirs::download_dir().unwrap_or_else(|| home.join("Downloads")),
        "Pictures" => dirs::picture_dir().unwrap_or_else(|| home.join("Pictures")),
        "Music" => dirs::audio_dir().unwrap_or_else(|| home.join("Music")),
        "Movies" => dirs::video_dir().unwrap_or_else(|| home.join("Movies")),
        "Documents" => dirs::document_dir().unwrap_or_else(|| home.join("Documents")),
        "DCIM" => dirs::picture_dir()
            .unwrap_or_else(|| home.join("Pictures"))
            .join("DCIM"),
        _ => home,
    }
}
