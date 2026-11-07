//! The host and device file panes: rows, selection, menus, and drag & drop.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::{Scrollbar, ScrollbarMode};
use gpui_kit::component::{ActiveTheme, Icon, h_flex, v_flex, v_virtual_list};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::app::{AppView, DragGhost, DragPayload, ViewMode};
use crate::appearance;
use crate::format::{format_date, format_mode, format_size};
use crate::icons::{file_icon, folder_icon, icon_for_entry};
use crate::model::{ClipboardOp, FileEntry, Side};

impl AppView {
    pub fn render_main(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(self.render_sidebar(window, cx))
            .when(self.selected_device.is_none(), |el| {
                el.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.render_file_pane(Side::Host, window, cx)),
                )
            })
            .when(self.selected_device.is_some(), |el| {
                el.when(self.show_host_pane, |el| {
                    el.child(
                        div()
                            .h_full()
                            .w(self.clamped_host_width(window.viewport_size().width))
                            .flex_none()
                            .child(self.render_file_pane(Side::Host, window, cx)),
                    )
                    .child(self.render_pane_divider(cx))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.render_file_pane(Side::Device, window, cx)),
                )
            })
    }

    /// A 5px grab band around a 1px line.
    fn render_pane_divider(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let hover = cx.theme().list_hover;
        h_flex()
            .id("pane-divider")
            .h_full()
            .w(appearance::PANE_DIVIDER_WIDTH)
            .flex_none()
            .justify_center()
            .cursor_col_resize()
            .hover(move |s| s.bg(hover))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.begin_pane_resize(
                        f32::from(event.position.x),
                        window.viewport_size().width,
                        cx,
                    );
                }),
            )
            .child(div().h_full().w(px(1.)).bg(border))
    }

    /// Pane width, used to pick the list-column layout.
    fn pane_width(&self, side: Side, window: &Window) -> Pixels {
        match side {
            Side::Host => self.clamped_host_width(window.viewport_size().width),
            Side::Device => {
                let host = if self.show_host_pane {
                    self.clamped_host_width(window.viewport_size().width)
                        + appearance::PANE_DIVIDER_WIDTH
                } else {
                    px(0.)
                };
                (window.viewport_size().width - appearance::SIDEBAR_WIDTH - host).max(px(0.))
            }
        }
    }

    /// One crumb per path component; the highlighted pane tints its bar so the
    /// sidebar's target is visible before clicking.
    fn render_path_bar(
        &self,
        side: Side,
        path: &str,
        pane_width: Pixels,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let foreground = cx.theme().foreground;
        let hover = cx.theme().list_hover;
        let border = cx.theme().border;
        let background = if focused {
            cx.theme().list_active
        } else {
            cx.theme().background
        };
        let this = cx.weak_entity();
        let crumbs = path_crumbs(side, path);
        let last = crumbs.len().saturating_sub(1);
        // Crumbs wrap and truncate rather than paint over the other pane.
        let max_label = (pane_width - px(70.)).max(px(60.));
        let (bar_id, lead_icon) = match side {
            Side::Host => ("host-path-bar", IconName::Laptop),
            Side::Device => ("device-path-bar", IconName::Smartphone),
        };

        h_flex()
            .id(bar_id)
            .w_full()
            .min_w_0()
            .min_h(appearance::PATH_BAR_HEIGHT)
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(px(2.))
            .px(px(10.))
            .py(px(4.))
            .overflow_hidden()
            .bg(background)
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_none()
                    .mr(px(4.))
                    .text_color(if focused { foreground } else { muted })
                    .child(Icon::new(lead_icon).size(px(13.))),
            )
            .children(
                crumbs
                    .into_iter()
                    .enumerate()
                    .map(move |(index, (label, target))| {
                        let is_last = index == last;
                        let this = this.clone();
                        let color = if is_last { foreground } else { muted };
                        let crumb_id =
                            ElementId::from(format!("path-crumb-{}-{target}", side_id(side)));
                        h_flex()
                            .items_center()
                            .gap(px(2.))
                            .when(index > 0, |el| {
                                el.child(
                                    div()
                                        .flex_none()
                                        .text_color(muted)
                                        .child(Icon::new(IconName::ChevronRight).size(px(10.))),
                                )
                            })
                            .child(
                                div()
                                    .id(crumb_id)
                                    .px(px(4.))
                                    .py(px(1.))
                                    .rounded(px(4.))
                                    .max_w(max_label)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_xs()
                                    .text_color(color)
                                    .when(is_last, |el| el.font_weight(FontWeight::MEDIUM))
                                    .when(!is_last, move |el| {
                                        el.cursor_pointer().hover(move |s| s.bg(hover)).on_click(
                                            move |_, _, cx| {
                                                this.update(cx, |this, cx| match side {
                                                    Side::Host => this
                                                        .navigate_host(PathBuf::from(&target), cx),
                                                    Side::Device => {
                                                        this.navigate_device(target.clone(), cx)
                                                    }
                                                })
                                                .ok();
                                            },
                                        )
                                    })
                                    .child(label),
                            )
                    }),
            )
    }

    pub fn render_file_pane(
        &self,
        side: Side,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let compact = self.pane_width(side, window) < appearance::COMPACT_LIST_WIDTH;
        let path = match side {
            Side::Host => self.host_cwd.to_string_lossy().to_string(),
            Side::Device => self.device_cwd.clone(),
        };
        let loading = match side {
            Side::Host => self.host_loading,
            Side::Device => self.device_loading,
        };
        let entry_count = self.visible_len(side);
        let pane_id = match side {
            Side::Host => "host-pane",
            Side::Device => "device-pane",
        };
        let has_entries = entry_count > 0;
        let pane_width = self.pane_width(side, window);
        let mode = self.view_mode;
        let focused = self.focused_side == side;
        let scroll = match side {
            Side::Host => self.host_scroll.clone(),
            Side::Device => self.device_scroll.clone(),
        };

        // Only the visible slice is built each frame; a media folder would
        // otherwise lay out thousands of rows per repaint.
        let content: AnyElement = if !has_entries {
            // Centre the card in the pane, not at the top.
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(self.render_empty_state(side, cx))
                .into_any_element()
        } else {
            let (columns, rows, row_height) = match mode {
                ViewMode::List => (1, entry_count, px(appearance::LIST_ROW_HEIGHT)),
                ViewMode::Grid => {
                    let columns = grid_columns(pane_width);
                    (
                        columns,
                        entry_count.div_ceil(columns),
                        px(appearance::GRID_ROW_HEIGHT),
                    )
                }
            };
            let item_sizes = Rc::new(vec![size(pane_width, row_height); rows]);

            v_virtual_list(
                cx.entity(),
                format!("{pane_id}-virtual"),
                item_sizes,
                move |this, range, _window, cx| {
                    let entries = this.visible_entries(side);
                    // Shared by every row; cloning per row is quadratic.
                    let drag_selection: Arc<Vec<String>> = Arc::new(this.selected_paths(side));
                    match mode {
                        ViewMode::List => range
                            .map(|ix| {
                                this.render_file_row(
                                    entries[ix],
                                    side,
                                    compact,
                                    &drag_selection,
                                    cx,
                                )
                                .into_any_element()
                            })
                            .collect(),
                        ViewMode::Grid => range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(entries.len());
                                let mut row_el =
                                    h_flex().w_full().gap_x(px(appearance::GRID_COLUMN_GAP));
                                for entry in &entries[start..end] {
                                    row_el = row_el.child(this.render_file_tile(
                                        entry,
                                        side,
                                        &drag_selection,
                                        cx,
                                    ));
                                }
                                row_el.into_any_element()
                            })
                            .collect(),
                    }
                },
            )
            .track_scroll(&scroll)
            .px(px(if mode == ViewMode::Grid {
                appearance::GRID_HORIZONTAL_PADDING
            } else {
                0.
            }))
            .pt(px(if mode == ViewMode::Grid { 14. } else { 4. }))
            .pb(px(if mode == ViewMode::Grid { 18. } else { 10. }))
            .into_any_element()
        };

        // Read theme colors before the closures borrow the theme.
        let drop_bg = cx.theme().drop_target;
        let this = cx.weak_entity();

        v_flex()
            .id(pane_id)
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex_col()
            .bg(cx.theme().background)
            // Any click in the pane makes it the one navigation acts on.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.focused_side != side {
                        this.focused_side = side;
                        cx.notify();
                    }
                }),
            )
            .when(
                !(side == Side::Device && self.selected_device.is_none()),
                |el| el.child(self.render_path_bar(side, &path, pane_width, focused, cx)),
            )
            .when(
                self.view_mode == ViewMode::List && has_entries && !loading,
                |el| el.child(self.render_list_header(compact, cx)),
            )
            .child(
                div()
                    .id(format!("{pane_id}-list"))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .on_drop::<ExternalPaths>({
                        let this = this.clone();
                        let target = path.clone();
                        move |paths, _, cx| {
                            this.update(cx, |this, cx| {
                                this.handle_external_drop(paths.paths(), side, target.clone(), cx)
                            })
                            .ok();
                        }
                    })
                    .on_drop::<DragPayload>({
                        let this = this.clone();
                        let target = path.clone();
                        move |payload, w, cx| {
                            this.update(cx, |this, cx| {
                                this.handle_internal_drop(payload, side, target.clone(), w, cx)
                            })
                            .ok();
                        }
                    })
                    .on_drag_move::<ExternalPaths>({
                        let this = this.clone();
                        move |_, _, cx| {
                            this.update(cx, |this, cx| this.set_drag_over(side, cx))
                                .ok();
                        }
                    })
                    .on_drag_move::<DragPayload>({
                        let this = this.clone();
                        move |_, _, cx| {
                            this.update(cx, |this, cx| this.set_drag_over(side, cx))
                                .ok();
                        }
                    })
                    .when(self.drag_over == Some(side), move |el| el.bg(drop_bg))
                    .when(loading, |el| el.opacity(0.55))
                    .child(content)
                    .child(
                        Scrollbar::vertical(&scroll)
                            .id(format!("{pane_id}-scrollbar"))
                            .mode(ScrollbarMode::Always)
                            .styles(|styles| {
                                styles
                                    .track(|track| track.width(px(8.)))
                                    .thumb(|thumb| thumb.width(px(4.)).inset(px(2.)).radius(px(2.)))
                                    .thumb_hover(|thumb| {
                                        thumb.width(px(6.)).inset(px(1.)).radius(px(3.))
                                    })
                            }),
                    )
                    .context_menu({
                        let this = this.clone();
                        move |menu, _w, _cx| {
                            menu.item(
                                PopupMenuItem::new("Paste")
                                    .icon(Icon::new(IconName::Clipboard))
                                    .on_click({
                                        let this = this.clone();
                                        move |_, w, cx| {
                                            this.update(cx, |this, cx| {
                                                this.focused_side = side;
                                                this.paste(w, cx)
                                            })
                                            .ok();
                                        }
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("New folder")
                                    .icon(Icon::new(IconName::FolderPlus))
                                    .on_click({
                                        let this = this.clone();
                                        move |_, w, cx| {
                                            this.update(cx, |this, cx| {
                                                this.focused_side = side;
                                                this.open_new_folder_dialog(side, w, cx)
                                            })
                                            .ok();
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("New file")
                                    .icon(Icon::new(IconName::FilePlus))
                                    .on_click({
                                        let this = this.clone();
                                        move |_, w, cx| {
                                            this.update(cx, |this, cx| {
                                                this.focused_side = side;
                                                this.open_new_file_dialog(side, w, cx)
                                            })
                                            .ok();
                                        }
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Refresh")
                                    .icon(Icon::new(IconName::RefreshCw))
                                    .on_click({
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            this.update(cx, |this, cx| this.refresh(cx)).ok();
                                        }
                                    }),
                            )
                        }
                    }),
            )
    }

    fn render_list_header(&self, compact: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        h_flex()
            .w_full()
            .h(px(24.))
            .items_center()
            .gap_2()
            .px(px(14.))
            .bg(cx.theme().list_head)
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(muted)
            .child(div().flex_1().min_w_0().child("Name"))
            .when(!compact, |el| {
                el.child(div().w(px(132.)).flex_none().child("Date Modified"))
            })
            .child(div().w(px(78.)).flex_none().text_right().child("Size"))
            .when(!compact, |el| {
                el.child(div().w(px(84.)).flex_none().child("Permissions"))
            })
    }

    fn render_empty_state(&self, side: Side, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let this = cx.weak_entity();

        let (icon, title, detail, show_server) =
            if side == Side::Device && self.selected_device.is_none() {
                (
                    IconName::Smartphone,
                    "No device connected".to_string(),
                    "Connect a device over USB with debugging enabled.".to_string(),
                    true,
                )
            } else if let Some(error) = &self.error {
                (
                    IconName::TriangleAlert,
                    "Can't read this folder".to_string(),
                    error.clone(),
                    false,
                )
            } else if let Some((state, help)) = self.unready_device_hint(side) {
                (IconName::Smartphone, state, help, false)
            } else if !self.search.trim().is_empty() {
                (
                    IconName::Search,
                    "No matches".to_string(),
                    format!(
                        "Nothing here matches \u{201c}{}\u{201d}.",
                        self.search.trim()
                    ),
                    false,
                )
            } else {
                (
                    IconName::FolderOpen,
                    "Folder is empty".to_string(),
                    String::new(),
                    false,
                )
            };

        v_flex()
            .w_full()
            .py(px(24.))
            .items_center()
            .justify_center()
            .gap_2()
            .child(div().text_color(muted).child(Icon::new(icon).size(px(30.))))
            .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title))
            .when(!detail.is_empty(), |el| {
                el.child(
                    div()
                        .max_w(px(340.))
                        .text_xs()
                        .text_center()
                        .when(self.error.is_some(), |el| el.text_color(danger))
                        .text_color(muted)
                        .child(detail),
                )
            })
            .when(show_server, |el| {
                el.child(
                    Button::new("empty-refresh-devices")
                        .label("Refresh devices")
                        .icon(Icon::new(IconName::RefreshCw))
                        .primary()
                        .on_click(move |_, _, cx| {
                            this.update(cx, |this, cx| this.refresh_devices(cx)).ok();
                        }),
                )
            })
    }

    /// Explains an unbrowsable device instead of showing an empty pane.
    fn unready_device_hint(&self, side: Side) -> Option<(String, String)> {
        if side != Side::Device {
            return None;
        }
        let serial = self.selected_device.as_ref()?;
        let device = self.devices.iter().find(|d| &d.serial == serial)?;
        if device.state.is_ready() {
            return None;
        }
        let label = device.state.label();
        let detail = device
            .state
            .help()
            .map(str::to_string)
            .unwrap_or_else(|| format!("The device is {}.", label.to_lowercase()));
        Some((format!("Device {}.", label.to_lowercase()), detail))
    }

    fn render_file_row(
        &self,
        entry: &FileEntry,
        side: Side,
        compact: bool,
        drag_selection: &Arc<Vec<String>>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = self.is_selected(&entry.path);
        let hover_bg = cx.theme().list_hover;
        let selection = cx.theme().selection;
        let fg = if selected {
            rgba(0xffffffff).into()
        } else {
            cx.theme().foreground
        };
        let sub = if selected {
            rgba(0xffffffd9).into()
        } else {
            cx.theme().muted_foreground
        };

        let this = cx.weak_entity();
        let path = entry.path.clone();
        let name = entry.name.clone();
        let is_dir = entry.is_dir();
        let open_single = self.settings.open_on_single_click;
        let drag_paths = if selected {
            drag_selection.clone()
        } else {
            Arc::new(vec![path.clone()])
        };

        h_flex()
            .id(ElementId::from(format!(
                "row-{}-{}",
                side_id(side),
                entry.path
            )))
            .w_full()
            .h(px(appearance::LIST_ROW_HEIGHT))
            .mx(px(4.))
            .items_center()
            .gap_2()
            .px(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(fg)
            .when(selected, move |el| el.bg(selection))
            .when(!selected, move |el| el.hover(move |s| s.bg(hover_bg)))
            .on_click({
                let this = this.clone();
                let path = path.clone();
                move |event, _, cx| {
                    let dbl = matches!(event, ClickEvent::Mouse(e) if e.up.click_count >= 2);
                    let add = event.modifiers().control || event.modifiers().platform;
                    this.update(cx, |this, cx| {
                        if dbl || (open_single && !add) {
                            this.open_entry(&path, side, cx);
                        } else {
                            this.select_entry(&path, side, add, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_drag(
                DragPayload {
                    side,
                    paths: drag_paths,
                },
                move |payload, _, _window, cx| {
                    cx.new(|_cx| DragGhost::new(payload.paths.len().to_string()))
                },
            )
            .on_drop::<ExternalPaths>({
                let this = this.clone();
                let target = path.clone();
                move |paths, _, cx| {
                    let t = if is_dir {
                        target.clone()
                    } else {
                        String::new()
                    };
                    this.update(cx, |this, cx| {
                        if !t.is_empty() {
                            this.handle_external_drop(paths.paths(), side, t, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_drop::<DragPayload>({
                let this = this.clone();
                let target = path.clone();
                move |payload, w, cx| {
                    let t = if is_dir {
                        target.clone()
                    } else {
                        String::new()
                    };
                    this.update(cx, |this, cx| {
                        if !t.is_empty() {
                            this.handle_internal_drop(payload, side, t, w, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let this = this.clone();
                let path = path.clone();
                move |_, _, cx| {
                    this.update(cx, |this, cx| this.select_for_context_menu(&path, side, cx))
                        .ok();
                }
            })
            .context_menu({
                let this = this.clone();
                let path = path.clone();
                move |menu, _w, _cx| build_entry_menu(menu, this.clone(), side, path.clone())
            })
            .child(
                div()
                    .flex_none()
                    .text_color(fg)
                    .child(Icon::new(icon_for_entry(entry)).size(px(16.))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(name.clone()),
            )
            .when(!compact, |el| {
                el.child(
                    div()
                        .w(px(132.))
                        .flex_none()
                        .text_xs()
                        .text_color(sub)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_date(entry.modified)),
                )
            })
            .child(
                div()
                    .w(px(78.))
                    .flex_none()
                    .text_xs()
                    .text_color(sub)
                    .text_right()
                    .child(format_size(entry.size)),
            )
            .when(!compact, |el| {
                el.child(
                    div()
                        .w(px(84.))
                        .flex_none()
                        .text_xs()
                        .text_color(sub)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_mode(entry.mode)),
                )
            })
    }

    fn render_file_tile(
        &self,
        entry: &FileEntry,
        side: Side,
        drag_selection: &Arc<Vec<String>>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let selected = self.is_selected(&entry.path);
        let icon_bg = cx.theme().list_active;
        let hover_bg = cx.theme().list_hover;
        let selection = cx.theme().selection;
        let fg = cx.theme().foreground;

        let this = cx.weak_entity();
        let path = entry.path.clone();
        let name = entry.name.clone();
        let is_dir = entry.is_dir();
        let open_single = self.settings.open_on_single_click;
        let drag_paths = if selected {
            drag_selection.clone()
        } else {
            Arc::new(vec![path.clone()])
        };

        // The artwork has different aspect ratios, so branch before building.
        let art: AnyElement = if is_dir {
            folder_icon(entry, px(78.)).into_any_element()
        } else {
            file_icon(entry, px(44.)).into_any_element()
        };
        let label_fg = if selected {
            rgba(0xffffffff).into()
        } else {
            fg
        };

        v_flex()
            .id(ElementId::from(format!(
                "tile-{}-{}",
                side_id(side),
                entry.path
            )))
            .w(px(appearance::GRID_TILE_WIDTH))
            .h(px(appearance::GRID_TILE_HEIGHT))
            .flex_none()
            .items_center()
            .pt(px(6.))
            .gap(px(7.))
            .cursor_pointer()
            .on_click({
                let this = this.clone();
                let path = path.clone();
                move |event, _, cx| {
                    let dbl = matches!(event, ClickEvent::Mouse(e) if e.up.click_count >= 2);
                    let add = event.modifiers().control || event.modifiers().platform;
                    this.update(cx, |this, cx| {
                        if dbl || (open_single && !add) {
                            this.open_entry(&path, side, cx);
                        } else {
                            this.select_entry(&path, side, add, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_drag(
                DragPayload {
                    side,
                    paths: drag_paths,
                },
                move |payload, _, _window, cx| {
                    cx.new(|_cx| DragGhost::new(payload.paths.len().to_string()))
                },
            )
            .on_drop::<ExternalPaths>({
                let this = this.clone();
                let target = path.clone();
                move |paths, _, cx| {
                    let t = if is_dir {
                        target.clone()
                    } else {
                        String::new()
                    };
                    this.update(cx, |this, cx| {
                        if !t.is_empty() {
                            this.handle_external_drop(paths.paths(), side, t, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_drop::<DragPayload>({
                let this = this.clone();
                let target = path.clone();
                move |payload, w, cx| {
                    let t = if is_dir {
                        target.clone()
                    } else {
                        String::new()
                    };
                    this.update(cx, |this, cx| {
                        if !t.is_empty() {
                            this.handle_internal_drop(payload, side, t, w, cx);
                        }
                    })
                    .ok();
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let this = this.clone();
                let path = path.clone();
                move |_, _, cx| {
                    this.update(cx, |this, cx| this.select_for_context_menu(&path, side, cx))
                        .ok();
                }
            })
            .context_menu({
                let this = this.clone();
                let path = path.clone();
                move |menu, _w, _cx| build_entry_menu(menu, this.clone(), side, path.clone())
            })
            .child(
                div()
                    .w(px(106.))
                    .h(px(84.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .when(selected, move |el| el.bg(icon_bg))
                    .when(!selected, move |el| el.hover(move |s| s.bg(hover_bg)))
                    .child(art),
            )
            .child(
                div()
                    .max_w(px(106.))
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(5.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_center()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(label_fg)
                    .when(selected, move |el| el.bg(selection))
                    .child(name.clone()),
            )
    }

    fn select_entry(&mut self, path: &str, side: Side, add: bool, cx: &mut Context<Self>) {
        self.focused_side = side;
        if add {
            if !self.selected.insert(path.to_string()) {
                self.selected.remove(path);
            }
        } else {
            self.selected.clear();
            self.selected.insert(path.to_string());
        }
        cx.notify();
    }

    pub(crate) fn select_for_context_menu(
        &mut self,
        path: &str,
        side: Side,
        cx: &mut Context<Self>,
    ) {
        if self.is_selected(path) {
            self.focused_side = side;
            cx.notify();
        } else {
            self.select_entry(path, side, false, cx);
        }
    }

    fn open_entry(&mut self, path: &str, side: Side, cx: &mut Context<Self>) {
        let entry = self
            .entries_for(side)
            .iter()
            .find(|e| e.path == path)
            .cloned();
        let Some(entry) = entry else { return };
        if entry.is_dir() {
            match side {
                Side::Host => self.navigate_host(PathBuf::from(&entry.path), cx),
                Side::Device => self.navigate_device(entry.path.clone(), cx),
            }
        } else if side == Side::Host {
            self.open_with_system(&entry.path, cx);
        }
    }

    /// Opens a host file with the desktop's default handler. The spawn runs on
    /// the background executor so a slow opener cannot stall rendering.
    fn open_with_system(&mut self, path: &str, cx: &mut Context<Self>) {
        let path = PathBuf::from(path);
        let shown = path.to_string_lossy().to_string();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    #[cfg(any(unix, windows))]
                    {
                        #[cfg(target_os = "macos")]
                        let mut command = std::process::Command::new("open");
                        #[cfg(target_os = "windows")]
                        let mut command = {
                            let mut command = std::process::Command::new("cmd");
                            command.args(["/C", "start", ""]);
                            command
                        };
                        #[cfg(all(unix, not(target_os = "macos")))]
                        let mut command = std::process::Command::new("xdg-open");
                        command.arg(&path);
                        command.spawn().map(|_| ())
                    }
                    #[cfg(not(any(unix, windows)))]
                    {
                        let _ = path;
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "opening files is unavailable on this platform",
                        ))
                    }
                })
                .await;
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.status_line = Some(format!("Couldn't open {shown}: {error}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn single_selected_entry(&self, side: Side) -> Option<FileEntry> {
        let paths = self.selected_paths(side);
        if paths.len() != 1 {
            return None;
        }
        self.entries_for(side)
            .iter()
            .find(|e| e.path == paths[0])
            .cloned()
    }
}

fn side_id(side: Side) -> &'static str {
    match side {
        Side::Host => "host",
        Side::Device => "device",
    }
}

/// `(label, absolute target)` per path segment; device `/sdcard` reads
/// "Internal storage".
fn path_crumbs(side: Side, path: &str) -> Vec<(String, String)> {
    match side {
        Side::Host => {
            let mut crumbs = Vec::new();
            let mut accumulated = PathBuf::new();
            for component in Path::new(path).components() {
                accumulated.push(component.as_os_str());
                let label = match component {
                    std::path::Component::RootDir => "/".to_string(),
                    _ => component.as_os_str().to_string_lossy().to_string(),
                };
                crumbs.push((label, accumulated.to_string_lossy().to_string()));
            }
            crumbs
        }
        Side::Device => {
            let mut crumbs = Vec::new();
            let mut accumulated = String::new();
            for part in path.split('/').filter(|part| !part.is_empty()) {
                accumulated.push('/');
                accumulated.push_str(part);
                let label = if accumulated == "/sdcard" {
                    "Internal storage".to_string()
                } else {
                    part.to_string()
                };
                crumbs.push((label, accumulated.clone()));
            }
            if crumbs.is_empty() {
                crumbs.push(("Device root".to_string(), "/".to_string()));
            }
            crumbs
        }
    }
}

fn grid_columns(pane_width: Pixels) -> usize {
    let content = (f32::from(pane_width) - appearance::GRID_HORIZONTAL_PADDING * 2.).max(0.);
    let per_tile = appearance::GRID_TILE_WIDTH + appearance::GRID_COLUMN_GAP;
    (((content + appearance::GRID_COLUMN_GAP) / per_tile).floor() as usize).max(1)
}

/// Each item re-resolves the weak handle against the click's context.
fn build_entry_menu(
    menu: PopupMenu,
    this: WeakEntity<AppView>,
    side: Side,
    path: String,
) -> PopupMenu {
    let mut menu = menu
        .item(
            PopupMenuItem::new("Open")
                .icon(Icon::new(IconName::FolderOpen))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, _, cx| {
                        this.update(cx, |this, cx| this.open_entry(&path, side, cx))
                            .ok();
                    }
                }),
        )
        .separator();

    if side == Side::Host {
        menu = menu.item(
            PopupMenuItem::new("Push to device")
                .icon(Icon::new(IconName::Upload))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            let dest = this.device_cwd.clone();
                            this.push_paths_to(vec![PathBuf::from(path.clone())], dest, cx)
                        })
                        .ok();
                    }
                }),
        );
    } else {
        menu = menu.item(
            PopupMenuItem::new("Pull to…")
                .icon(Icon::new(IconName::Download))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, w, cx| {
                        this.update(cx, |this, cx| this.open_pull_target_dialog(&path, w, cx))
                            .ok();
                    }
                }),
        );
    }

    menu.separator()
        .item(
            PopupMenuItem::new("Copy")
                .icon(Icon::new(IconName::Copy))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.selected.clear();
                            this.selected.insert(path.clone());
                            this.set_clipboard(ClipboardOp::Copy, side, cx)
                        })
                        .ok();
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Cut")
                .icon(Icon::new(IconName::Scissors))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.selected.clear();
                            this.selected.insert(path.clone());
                            this.set_clipboard(ClipboardOp::Cut, side, cx)
                        })
                        .ok();
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new("Rename")
                .icon(Icon::new(IconName::Pencil))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, w, cx| {
                        this.update(cx, |this, cx| {
                            this.selected.clear();
                            this.selected.insert(path.clone());
                            this.open_rename_dialog(w, cx)
                        })
                        .ok();
                    }
                }),
        )
        .item(
            PopupMenuItem::new("Delete")
                .icon(Icon::new(IconName::Trash))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, w, cx| {
                        this.update(cx, |this, cx| {
                            this.selected.clear();
                            this.selected.insert(path.clone());
                            this.delete_selected(w, cx)
                        })
                        .ok();
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(if side == Side::Device {
                "Copy device path"
            } else {
                "Copy path"
            })
            .icon(Icon::new(IconName::Link))
            .on_click({
                let path = path.clone();
                move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                }
            }),
        )
        .item(
            PopupMenuItem::new("Properties")
                .icon(Icon::new(IconName::Info))
                .on_click({
                    let this = this.clone();
                    let path = path.clone();
                    move |_, w, cx| {
                        this.update(cx, |this, cx| {
                            this.selected.clear();
                            this.selected.insert(path.clone());
                            this.open_properties_dialog(w, cx)
                        })
                        .ok();
                    }
                }),
        )
}
