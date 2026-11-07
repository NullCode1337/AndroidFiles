//! The root view and application state. Sidebar, file panes, and dialogs live
//! in sibling `impl AppView` modules.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use gpui_kit::assets::IconName;
use gpui_kit::base::InteractiveElementExt;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, Theme, ThemeMode, VirtualListScrollHandle, WindowExt,
    h_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::actions::*;
use crate::appearance;
use crate::backend::adb::{AdbContext, is_wireless_serial, remote_join, wireless_host};
use crate::backend::backup::{backup_device, default_options};
use crate::backend::host;
use crate::backend::polling::{PollEvent, spawn_poller};
use crate::backend::progress::ProgressSink;
use crate::backend::resolve::resolve_adb_binary;
use crate::model::{
    BackupEntry, ClipboardEntry, ClipboardOp, ClipboardSource, DeviceInfo, FileEntry, Side,
    TransferDirection, TransferProgress,
};
use crate::settings::{Settings, ThemePref, WindowControlsPref};

const KEY_CONTEXT: &str = "AndroidFiles";

/// How file entries are laid out in a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
}

/// Shared so a large drag is not deep-copied once per selected row.
#[derive(Clone)]
pub struct DragPayload {
    pub side: Side,
    pub paths: Arc<Vec<String>>,
}

/// Display order per pane, reused until an entry list, search, or hidden-file
/// change. A shared order made one pane render the other's indices.
#[derive(Default)]
struct VisibleCache {
    revision: u64,
    search: String,
    show_hidden: bool,
    host_order: Vec<usize>,
    device_order: Vec<usize>,
}

/// The small ghost rendered while dragging file rows internally.
pub struct DragGhost {
    label: SharedString,
}

impl DragGhost {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(cx.theme().primary)
            .text_color(cx.theme().primary_foreground)
            .text_sm()
            .child(self.label.clone())
    }
}

/// Coalesces adb's per-chunk progress to one update per 100 ms; otherwise a
/// large transfer rebuilds the interface thousands of times a second.
struct ChannelSink {
    tx: smol::channel::Sender<(u64, u64)>,
    last: Mutex<Option<Instant>>,
}

impl ChannelSink {
    fn new(tx: smol::channel::Sender<(u64, u64)>) -> Self {
        Self {
            tx,
            last: Mutex::new(None),
        }
    }
}

impl ProgressSink for ChannelSink {
    fn report(&self, done: u64, total: u64) {
        let now = Instant::now();
        let mut last = match self.last.lock() {
            Ok(last) => last,
            Err(poisoned) => poisoned.into_inner(),
        };
        let due = last.is_none_or(|at| now.duration_since(at) >= Duration::from_millis(100));
        if !due && done < total {
            return;
        }
        *last = Some(now);
        let _ = self.tx.try_send((done, total));
    }
}

pub struct AppView {
    pub(crate) settings: Settings,
    pub(crate) adb_path: Option<String>,

    pub(crate) devices: Vec<DeviceInfo>,
    pub(crate) selected_device: Option<String>,

    pub(crate) host_cwd: PathBuf,
    host_entries: Vec<FileEntry>,
    pub(crate) host_loading: bool,

    pub(crate) device_cwd: String,
    device_entries: Vec<FileEntry>,
    pub(crate) device_loading: bool,

    pub(crate) selected: HashSet<String>,
    pub(crate) focused_side: Side,

    clipboard: Option<ClipboardEntry>,

    pub(crate) transfers: Vec<TransferProgress>,
    next_transfer_id: u64,
    pub(crate) backup_entries: Vec<BackupEntry>,

    pub(crate) view_mode: ViewMode,
    pub(crate) show_host_pane: bool,
    /// Until dragged, the split is equal and follows the window.
    pub(crate) host_pane_width: Pixels,
    /// Set once the divider is dragged.
    pub(crate) host_pane_custom: bool,
    /// Pointer origin and width captured when a divider drag starts.
    pane_drag: Option<(f32, f32)>,
    /// Set while the pointer is pressed on the draggable title bar.
    titlebar_drag: bool,
    pub(crate) drag_over: Option<Side>,

    /// Bumped when a pane's entries are replaced; invalidates the cache.
    entries_revision: u64,
    visible_cache: RefCell<VisibleCache>,
    /// Scroll position per pane.
    pub(crate) host_scroll: VirtualListScrollHandle,
    pub(crate) device_scroll: VirtualListScrollHandle,

    /// Keeps the view on the key-dispatch path when no child is focused.
    focus_handle: FocusHandle,
    /// Used by async failures that need to open a dialog.
    window_handle: AnyWindowHandle,
    /// Live toolbar search; filters entries by name.
    pub(crate) search_input: Entity<InputState>,
    pub(crate) search: String,
    _search_sub: Option<Subscription>,

    /// Back/forward stacks for each side.
    host_back: Vec<PathBuf>,
    host_forward: Vec<PathBuf>,
    device_back: Vec<String>,
    device_forward: Vec<String>,

    pub(crate) status_line: Option<String>,
    pub(crate) error: Option<String>,

    _poller: Option<Arc<AtomicBool>>,
    _appearance_sub: Option<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        let adb_path = settings
            .adb_path
            .clone()
            .or_else(|| resolve_adb_binary(None));
        let host_cwd = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let window_handle = window.window_handle();

        let mut this = Self {
            settings,
            adb_path,
            devices: Vec::new(),
            selected_device: None,
            host_cwd,
            host_entries: Vec::new(),
            host_loading: false,
            device_cwd: "/sdcard".to_string(),
            device_entries: Vec::new(),
            device_loading: false,
            selected: HashSet::new(),
            focused_side: Side::Host,
            clipboard: None,
            transfers: Vec::new(),
            next_transfer_id: 0,
            backup_entries: Vec::new(),
            view_mode: ViewMode::Grid,
            show_host_pane: true,
            host_pane_width: px(0.),
            host_pane_custom: false,
            pane_drag: None,
            titlebar_drag: false,
            drag_over: None,
            entries_revision: 0,
            visible_cache: RefCell::new(VisibleCache::default()),
            host_scroll: VirtualListScrollHandle::new(),
            device_scroll: VirtualListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            window_handle,
            search_input,
            search: String::new(),
            _search_sub: None,
            host_back: Vec::new(),
            host_forward: Vec::new(),
            device_back: Vec::new(),
            device_forward: Vec::new(),
            status_line: None,
            error: None,
            _poller: None,
            _appearance_sub: None,
        };

        this._search_sub = Some(cx.subscribe(
            &this.search_input,
            |this, _input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.search = this.search_input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        ));

        this.apply_theme(window, cx);
        this._appearance_sub = Some(cx.observe_window_appearance(window, |this, window, cx| {
            this.apply_theme(window, cx);
        }));

        cx.bind_keys([
            KeyBinding::new("cmd-,", OpenSettings, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-,", OpenSettings, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-r", Refresh, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-r", Refresh, Some(KEY_CONTEXT)),
            // Global bindings win over focused inputs, so backspace must stay
            // free or the search field cannot delete characters.
            KeyBinding::new("alt-up", NavigateUp, Some(KEY_CONTEXT)),
            KeyBinding::new("delete", DeleteSelected, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-c", CopySelected, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-c", CopySelected, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-x", CutSelected, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-x", CutSelected, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-v", Paste, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-v", Paste, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-n", NewFolder, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-shift-n", NewFolder, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-n", NewFile, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-n", NewFile, Some(KEY_CONTEXT)),
            KeyBinding::new("enter", RenameSelected, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-i", ShowProperties, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-i", ShowProperties, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-h", ToggleHidden, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-b", ToggleHostPane, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-shift-h", ToggleHidden, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-shift-b", ToggleHostPane, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-f", FocusSearch, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-f", FocusSearch, Some(KEY_CONTEXT)),
            KeyBinding::new("alt-left", GoBack, Some(KEY_CONTEXT)),
            KeyBinding::new("alt-right", GoForward, Some(KEY_CONTEXT)),
        ]);

        this.spawn_poller(cx);
        this.spawn_transfer_cleanup(cx);
        this.load_host_dir(cx);
        window.focus(&this.focus_handle, cx);
        this
    }

    pub(crate) fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = match self.settings.theme {
            ThemePref::System => ThemeMode::from(window.appearance()),
            ThemePref::Light => ThemeMode::Light,
            ThemePref::Dark => ThemeMode::Dark,
        };
        Theme::change(mode, None, cx);
        if mode == ThemeMode::Light {
            appearance::apply_light_palette(cx);
        }

        window.set_background_appearance(self.window_background_for());
        cx.notify();
    }

    /// Toggles the host pane, keeping focus on a visible pane.
    pub(crate) fn toggle_host_pane(&mut self, cx: &mut Context<Self>) {
        if self.selected_device.is_none() {
            self.show_host_pane = true;
            self.focused_side = Side::Host;
            cx.notify();
            return;
        }
        self.show_host_pane = !self.show_host_pane;
        if !self.show_host_pane && self.focused_side == Side::Host {
            self.focused_side = Side::Device;
        }
        cx.notify();
    }

    /// Opens the host pane, or retracts it when already focused.
    pub(crate) fn activate_host_pane(&mut self, cx: &mut Context<Self>) {
        if self.selected_device.is_none() {
            self.show_host_pane = true;
            self.focused_side = Side::Host;
            cx.notify();
            return;
        }
        if !self.show_host_pane {
            self.show_host_pane = true;
            self.focused_side = Side::Host;
        } else if self.focused_side == Side::Host {
            self.show_host_pane = false;
            self.focused_side = Side::Device;
        } else {
            self.focused_side = Side::Host;
        }
        cx.notify();
    }

    /// Until dragged, the panes split evenly and follow the window; afterwards
    /// the chosen width is clamped so the device pane keeps its minimum.
    pub(crate) fn clamped_host_width(&self, viewport_width: Pixels) -> Pixels {
        let available = f32::from(viewport_width)
            - f32::from(appearance::SIDEBAR_WIDTH)
            - f32::from(appearance::PANE_DIVIDER_WIDTH);
        let min = f32::from(appearance::MIN_HOST_PANE_WIDTH);
        let max = (available - f32::from(appearance::MIN_DEVICE_PANE_WIDTH)).max(min);
        let desired = if self.host_pane_custom {
            f32::from(self.host_pane_width)
        } else {
            available / 2.
        };
        px(desired.clamp(min, max))
    }

    pub(crate) fn begin_pane_resize(
        &mut self,
        pointer_x: f32,
        viewport_width: Pixels,
        cx: &mut Context<Self>,
    ) {
        self.host_pane_width = self.clamped_host_width(viewport_width);
        self.pane_drag = Some((pointer_x, f32::from(self.host_pane_width)));
        cx.notify();
    }

    pub(crate) fn update_pane_resize(
        &mut self,
        pointer_x: f32,
        viewport_width: Pixels,
        cx: &mut Context<Self>,
    ) {
        let Some((start_x, start_width)) = self.pane_drag else {
            return;
        };
        let available = f32::from(viewport_width)
            - f32::from(appearance::SIDEBAR_WIDTH)
            - f32::from(appearance::PANE_DIVIDER_WIDTH);
        let min = f32::from(appearance::MIN_HOST_PANE_WIDTH);
        let max = (available - f32::from(appearance::MIN_DEVICE_PANE_WIDTH)).max(min);
        let width = px((start_width + pointer_x - start_x).clamp(min, max));
        let current = self.clamped_host_width(viewport_width);
        if width != current {
            self.host_pane_width = width;
            self.host_pane_custom = true;
            cx.notify();
        }
    }

    pub(crate) fn end_pane_resize(&mut self, cx: &mut Context<Self>) {
        if self.pane_drag.take().is_some() {
            cx.notify();
        }
    }

    /// Drag-move fires per pointer sample; repaint only on a pane change.
    pub(crate) fn set_drag_over(&mut self, side: Side, cx: &mut Context<Self>) {
        if self.drag_over != Some(side) {
            self.drag_over = Some(side);
            cx.notify();
        }
    }

    /// Apply a user-supplied adb path override, falling back to auto-detection.
    pub(crate) fn set_adb_path(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.settings.adb_path = path.clone();
        self.adb_path = path.or_else(|| resolve_adb_binary(None));
        let _ = self.settings.save();
        cx.notify();
    }

    fn window_background_for(&self) -> WindowBackgroundAppearance {
        if self.settings.translucent_window {
            if cfg!(target_os = "macos") {
                WindowBackgroundAppearance::Blurred
            } else if cfg!(target_os = "windows") {
                WindowBackgroundAppearance::MicaBackdrop
            } else {
                WindowBackgroundAppearance::Transparent
            }
        } else {
            WindowBackgroundAppearance::Opaque
        }
    }

    fn spawn_poller(&mut self, cx: &mut Context<Self>) {
        let adb_path = self.adb_path.clone();
        let (tx, rx) = smol::channel::unbounded::<PollEvent>();
        let shutdown = spawn_poller(adb_path, move |event| {
            let _ = tx.try_send(event);
        });
        self._poller = Some(shutdown);

        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                this.update(cx, |this, cx| this.handle_poll_event(event, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn handle_poll_event(&mut self, event: PollEvent, cx: &mut Context<Self>) {
        // The poller repeats the same state; only a change is worth a repaint.
        let mut changed = false;
        let mut status = None;
        match event {
            PollEvent::AdbMissing => {
                status = Some("adb binary not found. Set its path in Settings.".to_string());
            }
            PollEvent::ServerNotRunning => {
                status =
                    Some("adb server not reachable. Refresh devices from the sidebar.".to_string());
            }
            PollEvent::Devices(devices) => {
                if devices != self.devices {
                    let had_device = !self.devices.is_empty();
                    let was_ready = self
                        .selected_device_info()
                        .is_some_and(|d| d.state.is_ready());
                    self.devices = devices;
                    let reselect = match &self.selected_device {
                        None => true,
                        Some(serial) => !self.devices.iter().any(|d| &d.serial == serial),
                    };
                    if reselect {
                        let next = self
                            .devices
                            .iter()
                            .find(|d| d.state.is_ready())
                            .map(|d| d.serial.clone())
                            .or_else(|| self.devices.first().map(|d| d.serial.clone()));
                        self.selected_device = next;
                        self.selected.clear();
                        self.device_back.clear();
                        self.device_forward.clear();
                        self.load_device_dir(cx);
                    } else if !was_ready
                        && self
                            .selected_device_info()
                            .is_some_and(|d| d.state.is_ready())
                    {
                        // Newly authorized: the listing failed before, so load it.
                        self.load_device_dir(cx);
                    } else if was_ready
                        && !self
                            .selected_device_info()
                            .is_some_and(|d| d.state.is_ready())
                    {
                        // Drop the last listing so an offline device cannot
                        // leave its old folder visible.
                        self.device_entries.clear();
                        self.error = None;
                        self.device_loading = false;
                        self.bump_entries();
                    }
                    if self.selected_device.is_none() {
                        self.show_host_pane = true;
                        self.focused_side = Side::Host;
                    } else if !had_device {
                        self.show_host_pane = false;
                        self.focused_side = Side::Device;
                    }
                    changed = true;
                }
            }
        }
        if self.status_line != status {
            self.status_line = status;
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    /// Re-checks devices on demand; starting the server first makes this work
    /// before the background poller has.
    pub(crate) fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        let adb_path = self.adb_path.clone();
        self.status_line = Some("Refreshing devices…".to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    adb.start_server();
                    adb.list_device_infos()
                })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(devices) => {
                    this.status_line = None;
                    this.handle_poll_event(PollEvent::Devices(devices), cx);
                }
                Err(error) => {
                    this.status_line = Some(format!(
                        "Couldn't refresh devices: {error}. Check the adb path in Settings."
                    ));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn pair_wireless_device(
        &mut self,
        pairing: String,
        code: String,
        cx: &mut Context<Self>,
    ) {
        let adb_path = self.adb_path.clone();
        let host = wireless_host(&pairing).to_string();
        self.status_line = Some("Pairing wireless device…".to_string());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    adb.start_server();
                    adb.pair_wireless(&pairing, &code)
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.remember_wireless_host(&host);
                        this.status_line = Some(format!(
                            "Paired with {host}. Open Add wireless device and enter the device address and port shown on the Wireless debugging screen."
                        ));
                    }
                    Err(error) => {
                        this.status_line =
                            Some(format!("Couldn't pair wireless device: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn connect_wireless_device(&mut self, connect: String, cx: &mut Context<Self>) {
        let host = wireless_host(&connect).to_string();
        if let Some(serial) = self
            .devices
            .iter()
            .find(|device| {
                device.state.is_ready() && wireless_host(&device.serial) == host.as_str()
            })
            .map(|device| device.serial.clone())
        {
            self.select_device(serial.clone(), cx);
            self.status_line = Some(format!("Already connected to {serial}."));
            cx.notify();
            return;
        }

        let adb_path = self.adb_path.clone();
        let serial = connect.clone();
        self.status_line = Some("Connecting wireless device…".to_string());
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    adb.start_server();
                    let already = adb.ready_wireless_transport(&host)?;
                    if already.is_none() {
                        adb.connect_wireless(&connect)
                            .map_err(|error| format!("connection failed: {error}"))?;
                    }
                    let devices = adb.list_device_infos()?;
                    let raw_listed = adb
                        .list_devices()?
                        .iter()
                        .any(|device| device.identifier == connect);
                    Ok::<_, String>((devices, raw_listed, already))
                })
                .await;
            this.update(cx, |this, cx| match result {
                Ok((devices, raw_listed, already)) => {
                    let listed = devices.iter().any(|device| device.serial == serial);
                    this.handle_poll_event(PollEvent::Devices(devices), cx);
                    this.remember_wireless_device(&serial);
                    if listed {
                        this.select_device(serial.clone(), cx);
                        this.status_line = Some(format!("Wireless device {serial} connected."));
                    } else if let Some(existing) = already {
                        this.status_line = Some(format!("Already connected to {existing}."));
                    } else if raw_listed {
                        this.status_line = Some(format!(
                            "Connected to {serial}; using the existing connection for this device."
                        ));
                    } else {
                        this.status_line = Some(format!(
                            "Connected to {serial}, but it is not listed yet. Refresh devices."
                        ));
                    }
                    cx.notify();
                }
                Err(error) => {
                    this.status_line = Some(format!("Couldn't connect wireless device: {error}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn confirm_disconnect_device(
        &mut self,
        address: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !is_wireless_serial(&address) {
            self.status_line = Some("No wireless connection to disconnect.".to_string());
            cx.notify();
            return;
        }
        let this = cx.weak_entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(format!("Disconnect \"{name}\"?"))
                .description(
                    "This removes the wireless ADB connection. The device stays paired and can be reconnected from Add wireless device.",
                )
                .confirm()
                .ok_text("Disconnect")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("Cancel")
                .centered()
                .entrance(DialogEntrance::FadeSlide)
                .on_ok({
                    let this = this.clone();
                    let address = address.clone();
                    move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.disconnect_device(address.clone(), cx)
                        })
                        .ok();
                        true
                    }
                })
        });
    }

    fn disconnect_device(&mut self, serial: String, cx: &mut Context<Self>) {
        let adb_path = self.adb_path.clone();
        let shown = serial.clone();
        self.status_line = Some(format!("Disconnecting {serial}…"));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    adb.disconnect_wireless_serial(&serial)?;
                    adb.list_device_infos()
                })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(devices) => {
                    this.handle_poll_event(PollEvent::Devices(devices), cx);
                    this.status_line = Some(format!("Disconnected {shown}."));
                    cx.notify();
                }
                Err(error) => {
                    this.status_line = Some(format!("Couldn't disconnect {shown}: {error}"));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn remember_wireless_host(&mut self, host: &str) {
        if self
            .settings
            .wireless_devices
            .iter()
            .any(|saved| wireless_host(saved) == host)
        {
            return;
        }
        self.settings.wireless_devices.push(host.to_string());
        let _ = self.settings.save();
    }

    fn remember_wireless_device(&mut self, address: &str) {
        let host = wireless_host(address);
        let existing = self
            .settings
            .wireless_devices
            .iter()
            .position(|saved| wireless_host(saved) == host);
        match existing {
            Some(ix) if self.settings.wireless_devices[ix] == address => return,
            Some(ix) => self.settings.wireless_devices[ix] = address.to_string(),
            None => self.settings.wireless_devices.push(address.to_string()),
        }
        let _ = self.settings.save();
    }

    pub(crate) fn select_device(&mut self, serial: String, cx: &mut Context<Self>) {
        if self.selected_device.as_deref() == Some(&serial) {
            return;
        }
        self.selected_device = Some(serial);
        self.device_cwd = "/sdcard".to_string();
        self.device_back.clear();
        self.device_forward.clear();
        self.focused_side = Side::Device;
        self.selected.clear();
        self.load_device_dir(cx);
        cx.notify();
    }

    fn selected_device_info(&self) -> Option<&DeviceInfo> {
        let serial = self.selected_device.as_ref()?;
        self.devices.iter().find(|d| &d.serial == serial)
    }

    fn connected_device(&self) -> Option<&DeviceInfo> {
        self.selected_device_info()
            .filter(|device| device.state.is_ready())
    }

    pub(crate) fn launch_scrcpy(&mut self, cx: &mut Context<Self>) {
        let Some(serial) = self.connected_device().map(|device| device.serial.clone()) else {
            self.status_line =
                Some("Connect a device before starting screen mirroring.".to_string());
            cx.notify();
            return;
        };
        let program = self
            .settings
            .scrcpy_path
            .clone()
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| "scrcpy".to_string());

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::process::Command::new(program)
                        .arg("--serial")
                        .arg(serial)
                        .spawn()
                        .map(|_| ())
                })
                .await;
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.status_line = Some(format!("Couldn't launch scrcpy: {error}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn load_host_dir(&mut self, cx: &mut Context<Self>) {
        let path = self.host_cwd.clone();
        self.host_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { host::list_directory(&path) })
                .await;
            this.update(cx, |this, cx| {
                this.host_loading = false;
                match result {
                    Ok(entries) => {
                        this.host_entries = entries;
                        this.error = None;
                    }
                    Err(e) => {
                        this.error = Some(e.to_string());
                        this.host_entries = Vec::new();
                    }
                }
                this.bump_entries();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn load_device_dir(&mut self, cx: &mut Context<Self>) {
        let Some(serial) = self.selected_device.clone() else {
            self.device_loading = false;
            self.device_entries.clear();
            self.error = None;
            self.bump_entries();
            cx.notify();
            return;
        };
        let path = self.device_cwd.clone();
        let adb_path = self.adb_path.clone();
        let check_serial = serial.clone();
        let check_path = path.clone();
        self.device_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    adb.list_dir(&serial, &path)
                })
                .await;
            this.update(cx, |this, cx| {
                // A device that vanished or a folder that changed must not
                // repopulate the pane with a stale response.
                if this.selected_device.as_deref() != Some(check_serial.as_str())
                    || this.device_cwd != check_path
                    || !this
                        .selected_device_info()
                        .is_some_and(|device| device.state.is_ready())
                {
                    return;
                }
                this.device_loading = false;
                match result {
                    Ok(entries) => {
                        this.device_entries = entries;
                        this.error = None;
                    }
                    Err(e) => {
                        this.error = Some(e);
                        this.device_entries = Vec::new();
                    }
                }
                this.bump_entries();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn navigate_host(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path == self.host_cwd {
            self.focused_side = Side::Host;
            cx.notify();
            return;
        }
        self.host_back.push(self.host_cwd.clone());
        self.host_forward.clear();
        self.focused_side = Side::Host;
        self.set_host_dir(path, cx);
    }

    pub(crate) fn navigate_device(&mut self, path: String, cx: &mut Context<Self>) {
        if path == self.device_cwd {
            self.focused_side = Side::Device;
            cx.notify();
            return;
        }
        self.device_back.push(self.device_cwd.clone());
        self.device_forward.clear();
        self.focused_side = Side::Device;
        self.set_device_dir(path, cx);
    }

    fn set_host_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.host_cwd = path;
        self.selected.clear();
        self.load_host_dir(cx);
    }

    fn set_device_dir(&mut self, path: String, cx: &mut Context<Self>) {
        self.device_cwd = path;
        self.selected.clear();
        self.load_device_dir(cx);
    }

    /// Opens a sidebar shortcut in the highlighted pane. Shortcuts name device
    /// folders; a focused host pane maps them to the matching computer folder.
    pub(crate) fn open_location(
        &mut self,
        label: &str,
        device_path: &str,
        host_path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if self.selected_device.is_none()
            || (self.show_host_pane && self.focused_side == Side::Host)
        {
            if host_path.is_dir() {
                if self.selected_device.is_none() {
                    self.show_host_pane = true;
                }
                self.navigate_host(host_path, cx);
            } else {
                self.status_line = Some(format!("{label} is not available on this computer."));
                cx.notify();
            }
        } else {
            self.navigate_device(device_path.to_string(), cx);
        }
    }

    pub(crate) fn navigate_up(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Host => {
                if let Some(parent) = self.host_cwd.parent() {
                    self.navigate_host(parent.to_path_buf(), cx);
                }
            }
            Side::Device => {
                if let Some(idx) = self.device_cwd.rfind('/') {
                    let parent = if idx == 0 {
                        "/".to_string()
                    } else {
                        self.device_cwd[..idx].to_string()
                    };
                    self.navigate_device(parent, cx);
                }
            }
        }
    }

    pub(crate) fn can_go_back(&self, side: Side) -> bool {
        match side {
            Side::Host => !self.host_back.is_empty(),
            Side::Device => !self.device_back.is_empty(),
        }
    }

    pub(crate) fn can_go_forward(&self, side: Side) -> bool {
        match side {
            Side::Host => !self.host_forward.is_empty(),
            Side::Device => !self.device_forward.is_empty(),
        }
    }

    pub(crate) fn go_back(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Host => {
                let Some(prev) = self.host_back.pop() else {
                    return;
                };
                self.host_forward.push(self.host_cwd.clone());
                self.focused_side = Side::Host;
                self.set_host_dir(prev, cx);
            }
            Side::Device => {
                let Some(prev) = self.device_back.pop() else {
                    return;
                };
                self.device_forward.push(self.device_cwd.clone());
                self.focused_side = Side::Device;
                self.set_device_dir(prev, cx);
            }
        }
    }

    pub(crate) fn go_forward(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Host => {
                let Some(next) = self.host_forward.pop() else {
                    return;
                };
                self.host_back.push(self.host_cwd.clone());
                self.focused_side = Side::Host;
                self.set_host_dir(next, cx);
            }
            Side::Device => {
                let Some(next) = self.device_forward.pop() else {
                    return;
                };
                self.device_back.push(self.device_cwd.clone());
                self.focused_side = Side::Device;
                self.set_device_dir(next, cx);
            }
        }
    }

    /// Toolbar title for the focused pane.
    pub(crate) fn current_folder_name(&self, side: Side) -> String {
        match side {
            Side::Host => self
                .host_cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "This computer".to_string()),
            Side::Device => {
                if self.selected_device.is_none() {
                    "No device".to_string()
                } else if self.device_cwd == "/sdcard" {
                    "Internal storage".to_string()
                } else if self.device_cwd == "/" {
                    "Device root".to_string()
                } else {
                    self.device_cwd
                        .rsplit('/')
                        .find(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| self.device_cwd.clone())
                }
            }
        }
    }

    /// Filtered entries in display order, cached because several widgets ask
    /// during one frame.
    pub(crate) fn visible_entries(&self, side: Side) -> Vec<&FileEntry> {
        self.ensure_visible();
        let entries = self.entries_for(side);
        let cache = self.visible_cache.borrow();
        let order = match side {
            Side::Host => &cache.host_order,
            Side::Device => &cache.device_order,
        };
        order.iter().map(|&ix| &entries[ix]).collect()
    }

    /// Count without materializing the reference list.
    pub(crate) fn visible_len(&self, side: Side) -> usize {
        self.ensure_visible();
        let cache = self.visible_cache.borrow();
        match side {
            Side::Host => cache.host_order.len(),
            Side::Device => cache.device_order.len(),
        }
    }

    fn ensure_visible(&self) {
        let mut cache = self.visible_cache.borrow_mut();
        if cache.revision == self.entries_revision
            && cache.search == self.search
            && cache.show_hidden == self.settings.show_hidden
        {
            return;
        }

        let needle = self.search.trim().to_lowercase();
        let show_hidden = self.settings.show_hidden;
        let build = |entries: &[FileEntry]| -> Vec<usize> {
            let mut order: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    (show_hidden || !e.hidden)
                        && (needle.is_empty() || e.name.to_lowercase().contains(&needle))
                })
                .map(|(ix, _)| ix)
                .collect();
            // Lowercase keys are computed once per entry, not per comparison.
            order.sort_by_cached_key(|&ix| {
                let entry = &entries[ix];
                (!entry.is_dir(), entry.name.to_lowercase())
            });
            order
        };

        cache.host_order = build(&self.host_entries);
        cache.device_order = build(&self.device_entries);
        cache.revision = self.entries_revision;
        cache.search = self.search.clone();
        cache.show_hidden = show_hidden;
    }

    /// Invalidates the display-order cache.
    fn bump_entries(&mut self) {
        self.entries_revision = self.entries_revision.wrapping_add(1);
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        match self.focused_side {
            Side::Host => self.load_host_dir(cx),
            Side::Device => self.load_device_dir(cx),
        }
    }

    pub(crate) fn entries_for(&self, side: Side) -> &[FileEntry] {
        match side {
            Side::Host => &self.host_entries,
            Side::Device => &self.device_entries,
        }
    }

    pub(crate) fn selected_paths(&self, side: Side) -> Vec<String> {
        if self.selected.is_empty() {
            return Vec::new();
        }
        let entries = self.entries_for(side);
        // One scan, not every selected path against every entry.
        let mut out: Vec<String> = entries
            .iter()
            .filter(|entry| self.selected.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect();
        out.sort();
        out
    }

    /// Count without building the path list the status bar does not need.
    pub(crate) fn selected_count(&self, side: Side) -> usize {
        if self.selected.is_empty() {
            return 0;
        }
        self.entries_for(side)
            .iter()
            .filter(|entry| self.selected.contains(&entry.path))
            .count()
    }

    fn selected_entry_names(&self, side: Side) -> Vec<String> {
        self.entries_for(side)
            .iter()
            .filter(|e| self.selected.contains(&e.path))
            .map(|e| e.name.clone())
            .collect()
    }

    pub(crate) fn is_selected(&self, path: &str) -> bool {
        self.selected.contains(path)
    }

    pub(crate) fn set_clipboard(&mut self, op: ClipboardOp, side: Side, cx: &mut Context<Self>) {
        let paths = self.selected_paths(side);
        if paths.is_empty() {
            return;
        }
        // Keep the last entry plus every path on its side.
        let first = paths[0].clone();
        let name = first.rsplit('/').next().unwrap_or(&first).to_string();
        let is_dir = self
            .entries_for(side)
            .iter()
            .find(|e| e.path == first)
            .map(|e| e.is_dir())
            .unwrap_or(false);
        self.clipboard = Some(ClipboardEntry {
            op,
            source: if side == Side::Host {
                ClipboardSource::Host
            } else {
                ClipboardSource::Device
            },
            path: first,
            name,
            is_dir,
        });
        cx.notify();
    }

    pub(crate) fn paste(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(clip) = self.clipboard.clone() else {
            return;
        };
        let target = self.focused_side;
        let host_dir = self.host_cwd.clone();
        let device_dir = self.device_cwd.clone();
        let cut = clip.op == ClipboardOp::Cut;

        match (clip.source, target) {
            (ClipboardSource::Host, Side::Host) => {
                let dest = host_dir.join(&clip.name);
                let res = if cut {
                    host::move_entry(Path::new(&clip.path), &dest)
                } else {
                    host::copy(Path::new(&clip.path), &dest)
                };
                if let Err(e) = res {
                    self.error = Some(e.to_string());
                }
                if cut {
                    self.clipboard = None;
                }
                self.load_host_dir(cx);
            }
            (ClipboardSource::Device, Side::Device) => {
                let Some(serial) = self.selected_device.clone() else {
                    return;
                };
                let adb_path = self.adb_path.clone();
                let source = clip.path.clone();
                let dest = remote_join(&device_dir, &clip.name);
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let adb = AdbContext::new(adb_path);
                            if cut {
                                adb.move_entry(&serial, &source, &dest)
                            } else {
                                adb.copy(&serial, &source, &dest)
                            }
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Err(e) = result {
                            this.error = Some(e);
                        }
                        if cut {
                            this.clipboard = None;
                        }
                        this.load_device_dir(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            (ClipboardSource::Host, Side::Device) => {
                self.push_paths_to(vec![PathBuf::from(&clip.path)], device_dir, cx);
                // Cross-side cut degrades to copy; the source stays put.
                self.clipboard = None;
            }
            (ClipboardSource::Device, Side::Host) => {
                self.pull_paths_to(vec![clip.path.clone()], host_dir, cx);
                self.clipboard = None;
            }
        }
        cx.notify();
    }

    pub fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let side = self.focused_side;
        let paths = self.selected_paths(side);
        if paths.is_empty() {
            return;
        }
        let names = self.selected_entry_names(side);
        if self.settings.confirm_before_delete {
            self.open_delete_confirm(side, paths, names, window, cx);
        } else {
            self.perform_delete(side, paths, cx);
        }
    }

    pub(crate) fn perform_delete(
        &mut self,
        side: Side,
        paths: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        match side {
            Side::Host => {
                for p in paths.iter() {
                    if let Err(e) = host::delete(Path::new(p)) {
                        self.error = Some(e.to_string());
                    }
                }
                self.load_host_dir(cx);
            }
            Side::Device => {
                let Some(serial) = self.selected_device.clone() else {
                    return;
                };
                let adb_path = self.adb_path.clone();
                let paths = paths.clone();
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let adb = AdbContext::new(adb_path);
                            for p in paths.iter() {
                                adb.delete(&serial, p)?;
                            }
                            Ok::<(), String>(())
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Err(e) = result {
                            this.error = Some(format!("Delete failed: {e}"));
                        }
                        this.load_device_dir(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        self.selected.clear();
        cx.notify();
    }

    /// Creates a folder or empty file in the focused pane.
    pub(crate) fn create_entry(
        &mut self,
        side: Side,
        name: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        let name = name.trim().to_string();
        if name.is_empty() || name.contains('/') {
            self.error = Some("Invalid name.".to_string());
            cx.notify();
            return;
        }
        match side {
            Side::Host => {
                let path = self.host_cwd.join(&name);
                let res = if is_dir {
                    host::mkdir(&path)
                } else {
                    host::touch(&path)
                };
                if let Err(e) = res {
                    self.error = Some(e.to_string());
                }
                self.load_host_dir(cx);
            }
            Side::Device => {
                let Some(serial) = self.selected_device.clone() else {
                    return;
                };
                let adb_path = self.adb_path.clone();
                let path = remote_join(&self.device_cwd, &name);
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let adb = AdbContext::new(adb_path);
                            if is_dir {
                                adb.mkdir(&serial, &path)
                            } else {
                                adb.touch(&serial, &path)
                            }
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Err(e) = result {
                            this.error = Some(e);
                        }
                        this.load_device_dir(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        cx.notify();
    }

    /// Renames the single selected entry.
    pub(crate) fn rename_entry(&mut self, side: Side, new_name: String, cx: &mut Context<Self>) {
        let new_name = new_name.trim().to_string();
        let paths = self.selected_paths(side);
        if paths.len() != 1 {
            self.error = Some("Select one item to rename.".to_string());
            cx.notify();
            return;
        }
        let old = paths[0].clone();
        if new_name.is_empty() || new_name.contains('/') {
            self.error = Some("Invalid name.".to_string());
            cx.notify();
            return;
        }
        let old_name = old.rsplit('/').next().unwrap_or(&old).to_string();
        if old_name == new_name {
            return;
        }
        match side {
            Side::Host => {
                let parent = Path::new(&old)
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("/"));
                let dest = parent.join(&new_name);
                if let Err(e) = host::move_entry(Path::new(&old), &dest) {
                    self.error = Some(e.to_string());
                }
                self.load_host_dir(cx);
            }
            Side::Device => {
                let Some(serial) = self.selected_device.clone() else {
                    return;
                };
                let adb_path = self.adb_path.clone();
                let parent = match old.rfind('/') {
                    Some(0) => "/".to_string(),
                    Some(i) => old[..i].to_string(),
                    None => "/".to_string(),
                };
                let dest = remote_join(&parent, &new_name);
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let adb = AdbContext::new(adb_path);
                            adb.move_entry(&serial, &old, &dest)
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Err(e) = result {
                            this.error = Some(e);
                        }
                        this.load_device_dir(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        self.selected.clear();
        cx.notify();
    }

    pub(crate) fn prune_finished_transfers(&mut self) -> bool {
        let before = self.transfers.len();
        self.transfers.retain(|transfer| {
            transfer
                .finished_at
                .is_none_or(|finished| finished.elapsed() < Duration::from_secs(5))
        });
        self.transfers.len() != before
    }

    fn spawn_transfer_cleanup(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.prune_finished_transfers() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn next_transfer_id(&mut self) -> u64 {
        let id = self.next_transfer_id;
        self.next_transfer_id += 1;
        id
    }

    fn push_transfer(&mut self, id: u64, name: String) {
        self.transfers.push(TransferProgress {
            id,
            name,
            direction: TransferDirection::Push,
            bytes_done: 0,
            bytes_total: 0,
            done: false,
            failed: false,
            error: None,
            started: Utc::now(),
            finished_at: None,
        });
    }

    fn pull_transfer(&mut self, id: u64, name: String) {
        self.transfers.push(TransferProgress {
            id,
            name,
            direction: TransferDirection::Pull,
            bytes_done: 0,
            bytes_total: 0,
            done: false,
            failed: false,
            error: None,
            started: Utc::now(),
            finished_at: None,
        });
    }

    /// Pushes local paths into a remote directory.
    pub fn push_paths_to(
        &mut self,
        local_paths: Vec<PathBuf>,
        remote_dest: String,
        cx: &mut Context<Self>,
    ) {
        let Some(serial) = self.selected_device.clone() else {
            self.error = Some("No device selected.".to_string());
            cx.notify();
            return;
        };
        let adb_path = self.adb_path.clone();
        for local in local_paths {
            let name = local
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| local.to_string_lossy().to_string());
            let is_dir = host::is_directory(&local);
            let remote_full = remote_join(&remote_dest, &name);
            let id = self.next_transfer_id();
            self.push_transfer(id, name);

            let local = local.clone();
            let serial = serial.clone();
            let remote_full = remote_full.clone();
            let adb_path = adb_path.clone();
            cx.spawn(async move |this, cx| {
                let (tx, rx) = smol::channel::unbounded::<(u64, u64)>();
                let sink: Arc<dyn ProgressSink> = Arc::new(ChannelSink::new(tx));
                let local_for_size = local.clone();
                let task = cx.background_executor().spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    if is_dir {
                        adb.push_dir(&serial, &local, &remote_full, None, Some(sink))
                    } else {
                        let total = host::file_size(&local_for_size);
                        adb.push_file(&serial, &local_for_size, &remote_full, 0, total, Some(sink))
                    }
                });
                let progress_loop = async {
                    while let Ok((done, total)) = rx.recv().await {
                        this.update(cx, |this, cx| {
                            if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                                t.bytes_done = done;
                                t.bytes_total = total;
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                };
                let (result, _) = smol::future::zip(task, progress_loop).await;
                this.update(cx, |this, cx| {
                    if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                        t.done = true;
                        t.finished_at = Some(Instant::now());
                        if let Err(e) = result {
                            t.failed = true;
                            t.error = Some(e);
                        }
                        cx.notify();
                    }
                    this.load_device_dir(cx);
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Pulls remote paths into a local directory.
    pub fn pull_paths_to(
        &mut self,
        remote_paths: Vec<String>,
        local_dest: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let Some(serial) = self.selected_device.clone() else {
            self.error = Some("No device selected.".to_string());
            cx.notify();
            return;
        };
        let adb_path = self.adb_path.clone();
        let window_handle = self.window_handle;
        for remote in remote_paths {
            let name = remote
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(&remote)
                .to_string();
            let local_full = local_dest.join(&name);
            let id = self.next_transfer_id();
            let dialog_name = name.clone();
            self.pull_transfer(id, name);

            let serial = serial.clone();
            let remote = remote.clone();
            let local_full = local_full.clone();
            let adb_path = adb_path.clone();
            cx.spawn(async move |this, cx| {
                let (tx, rx) = smol::channel::unbounded::<(u64, u64)>();
                let sink: Arc<dyn ProgressSink> = Arc::new(ChannelSink::new(tx));
                let task = cx.background_executor().spawn(async move {
                    let adb = AdbContext::new(adb_path);
                    let is_dir = adb.is_dir_remote(&serial, &remote);
                    if is_dir {
                        adb.pull_dir(&serial, &remote, &local_full, None, Some(sink))
                    } else {
                        let total = adb.stat_size(&serial, &remote).unwrap_or(0);
                        adb.pull_file(&serial, &remote, &local_full, 0, total, Some(sink))
                    }
                });
                let progress_loop = async {
                    while let Ok((done, total)) = rx.recv().await {
                        this.update(cx, |this, cx| {
                            if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                                t.bytes_done = done;
                                t.bytes_total = total;
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                };
                let (result, _) = smol::future::zip(task, progress_loop).await;
                let failure = result.as_ref().err().cloned();
                this.update(cx, |this, cx| {
                    if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                        t.done = true;
                        t.finished_at = Some(Instant::now());
                        if let Some(error) = &failure {
                            t.failed = true;
                            t.error = Some(error.clone());
                        }
                        cx.notify();
                    }
                    this.load_host_dir(cx);
                })
                .ok();
                if let Some(error) = failure {
                    cx.update_window(window_handle, move |_, window, cx| {
                        crate::dialogs::open_pull_error_dialog(window, cx, &dialog_name, &error);
                    })
                    .ok();
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub fn start_backup(&mut self, cx: &mut Context<Self>) {
        let Some(serial) = self.selected_device.clone() else {
            self.error = Some("No device selected.".to_string());
            cx.notify();
            return;
        };
        let source = "/sdcard".to_string();
        let dest = PathBuf::from(&self.settings.default_download_folder)
            .join(format!("AndroidFiles-backup-{serial}"));

        self.backup_entries.push(BackupEntry {
            device_serial: serial.clone(),
            source: source.clone(),
            destination: dest.to_string_lossy().to_string(),
        });

        let id = self.next_transfer_id();
        self.pull_transfer(id, format!("Backup {serial}"));

        let adb_path = self.adb_path.clone();
        let options = default_options();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = smol::channel::unbounded::<(u64, u64)>();
            let sink: Arc<dyn ProgressSink> = Arc::new(ChannelSink::new(tx));
            let task = cx.background_executor().spawn(async move {
                let adb = AdbContext::new(adb_path);
                backup_device(&adb, &serial, &source, &dest, &options, None, Some(sink))
            });
            let progress_loop = async {
                while let Ok((done, total)) = rx.recv().await {
                    this.update(cx, |this, cx| {
                        if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                            t.bytes_done = done;
                            t.bytes_total = total;
                            cx.notify();
                        }
                    })
                    .ok();
                }
            };
            let (result, _) = smol::future::zip(task, progress_loop).await;
            this.update(cx, |this, cx| {
                if let Some(t) = this.transfers.iter_mut().find(|t| t.id == id) {
                    t.done = true;
                    t.finished_at = Some(Instant::now());
                    if let Err(e) = &result {
                        t.failed = true;
                        t.error = Some(e.clone());
                    }
                }
                match result {
                    Ok(summary) => {
                        this.status_line = Some(format!(
                            "Backup complete: {} files, {} dirs, {}",
                            summary.files,
                            summary.dirs,
                            crate::format::format_size(summary.bytes)
                        ));
                    }
                    Err(e) => {
                        this.error = Some(format!("Backup failed: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn handle_external_drop(
        &mut self,
        paths: &[PathBuf],
        target_side: Side,
        target_dir: String,
        cx: &mut Context<Self>,
    ) {
        self.drag_over = None;
        match target_side {
            Side::Host => {
                for p in paths {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let dest = Path::new(&target_dir).join(name);
                    // Self-copy truncates a file and recurses forever in a folder.
                    if dest == *p || dest.starts_with(p) {
                        continue;
                    }
                    if let Err(e) = host::copy(p, &dest) {
                        self.error = Some(e.to_string());
                    }
                }
                self.load_host_dir(cx);
            }
            Side::Device => {
                let local_paths: Vec<PathBuf> = paths.to_vec();
                self.push_paths_to(local_paths, target_dir, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn handle_internal_drop(
        &mut self,
        payload: &DragPayload,
        target_side: Side,
        target_dir: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag_over = None;
        let modifiers = window.modifiers();
        let copy =
            cfg!(target_os = "macos") || modifiers.alt || modifiers.control || modifiers.platform;

        match (payload.side, target_side) {
            (Side::Host, Side::Host) => {
                for p in payload.paths.iter() {
                    let name = p.rsplit('/').next().unwrap_or(p);
                    let dest = Path::new(&target_dir).join(name);
                    let src = Path::new(p);
                    if dest == src || dest.starts_with(src) {
                        continue;
                    }
                    let res = if copy {
                        host::copy(src, &dest)
                    } else {
                        host::move_entry(src, &dest)
                    };
                    if let Err(e) = res {
                        self.error = Some(e.to_string());
                    }
                }
                self.load_host_dir(cx);
            }
            (Side::Device, Side::Device) => {
                let Some(serial) = self.selected_device.clone() else {
                    return;
                };
                let adb_path = self.adb_path.clone();
                let paths = payload.paths.clone();
                cx.spawn(async move |this, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            let adb = AdbContext::new(adb_path);
                            for p in paths.iter() {
                                let name = p.rsplit('/').next().unwrap_or(p);
                                let dest = remote_join(&target_dir, name);
                                let inside_self = dest == *p
                                    || dest
                                        .strip_prefix(p)
                                        .is_some_and(|rest| rest.starts_with('/'));
                                if inside_self {
                                    continue;
                                }
                                if copy {
                                    adb.copy(&serial, p, &dest)?;
                                } else {
                                    adb.move_entry(&serial, p, &dest)?;
                                }
                            }
                            Ok::<(), String>(())
                        })
                        .await;
                    this.update(cx, |this, cx| {
                        if let Err(e) = result {
                            this.error = Some(e);
                        }
                        this.load_device_dir(cx);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            (Side::Host, Side::Device) => {
                let local_paths: Vec<PathBuf> = payload.paths.iter().map(PathBuf::from).collect();
                self.push_paths_to(local_paths, target_dir, cx);
            }
            (Side::Device, Side::Host) => {
                let remote_paths = payload.paths.as_ref().clone();
                self.pull_paths_to(remote_paths, PathBuf::from(target_dir), cx);
            }
        }
        cx.notify();
    }

    /// The setting wins; `Auto` follows the platform (macOS left, others right).
    pub(crate) fn controls_on_left(&self) -> bool {
        match self.settings.window_controls {
            WindowControlsPref::Auto => cfg!(target_os = "macos"),
            WindowControlsPref::Left => true,
            WindowControlsPref::Right => false,
        }
    }

    /// Only empty bar space drags the window; controls stop pointer-down so a
    /// press that starts on a button never moves it.
    fn render_titlebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let left = self.controls_on_left();
        let is_mac = cfg!(target_os = "macos");
        let is_web = cfg!(target_family = "wasm");
        let background = cx.theme().background;
        let border = cx.theme().border;

        // AppKit positions the native lights; y = 20 centres 12px lights in
        // the 52px bar. In right mode they move to the right edge.
        #[cfg(target_os = "macos")]
        {
            let x = if left {
                px(14.)
            } else {
                window.viewport_size().width - px(72.)
            };
            window.set_traffic_light_position(point(x, px(20.)));
        }

        h_flex()
            .id("title-bar")
            .h(appearance::TOOLBAR_HEIGHT)
            .w_full()
            .flex_none()
            .items_center()
            .bg(background)
            .border_b_1()
            .border_color(border)
            .when(left, |el| el.child(render_controls_left(cx)))
            .child(
                div()
                    .id("titlebar-drag-area")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .when(!is_web, |el| {
                        el.window_control_area(WindowControlArea::Drag)
                    })
                    .child(self.render_toolbar(window, cx)),
            )
            // The native lights need room; there is no widget to render.
            .when(!left && is_mac, |el| el.child(div().w(px(78.)).flex_none()))
            .when(!left && !is_mac, |el| {
                el.child(render_controls_right(window, cx))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.titlebar_drag = true),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, _| this.titlebar_drag = false))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.titlebar_drag = false),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.titlebar_drag {
                    this.titlebar_drag = false;
                    window.start_window_move();
                }
            }))
            .when(is_mac, |el| {
                el.on_double_click(cx.listener(|_, _, window, _| window.titlebar_double_click()))
            })
            .when(!is_mac, |el| {
                el.on_double_click(cx.listener(|_, _, window, _| window.zoom_window()))
            })
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = self.focused_side;
        let title = self.current_folder_name(side);
        let can_back = self.can_go_back(side);
        let can_forward = self.can_go_forward(side);
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let secondary = cx.theme().secondary;
        let foreground = cx.theme().foreground;
        let background = cx.theme().background;

        h_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                h_flex()
                    .items_center()
                    .p(px(2.))
                    .rounded(px(7.))
                    .bg(secondary)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("nav-back")
                            .ghost()
                            .small()
                            .rounded(px(5.))
                            .w(px(26.))
                            .h(px(24.))
                            .icon(Icon::new(IconName::ChevronLeft).small())
                            .tooltip("Back")
                            .disabled(!can_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let side = this.focused_side;
                                this.go_back(side, cx);
                            })),
                    )
                    .child(div().w(px(1.)).h(px(14.)).bg(border))
                    .child(
                        Button::new("nav-forward")
                            .ghost()
                            .small()
                            .rounded(px(5.))
                            .w(px(26.))
                            .h(px(24.))
                            .icon(Icon::new(IconName::ChevronRight).small())
                            .tooltip("Forward")
                            .disabled(!can_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let side = this.focused_side;
                                this.go_forward(side, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(crate::icons::mini_folder(px(20.)))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .max_w(px(280.))
                            .child(title),
                    ),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .items_center()
                    .p(px(2.))
                    .rounded(px(7.))
                    .bg(secondary)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("view-grid")
                            .ghost()
                            .small()
                            .rounded(px(5.))
                            .w(px(26.))
                            .h(px(24.))
                            .icon(Icon::new(IconName::LayoutGrid).small().text_color(
                                if self.view_mode == ViewMode::Grid {
                                    foreground
                                } else {
                                    muted
                                },
                            ))
                            .tooltip("Icons")
                            .when(self.view_mode == ViewMode::Grid, |button| {
                                button.bg(background)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::Grid;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("view-list")
                            .ghost()
                            .small()
                            .rounded(px(5.))
                            .w(px(26.))
                            .h(px(24.))
                            .icon(Icon::new(IconName::List).small().text_color(
                                if self.view_mode == ViewMode::List {
                                    foreground
                                } else {
                                    muted
                                },
                            ))
                            .tooltip("List")
                            .when(self.view_mode == ViewMode::List, |button| {
                                button.bg(background)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.view_mode = ViewMode::List;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Button::new("refresh")
                            .ghost()
                            .small()
                            .rounded(px(14.))
                            .w(px(28.))
                            .h(px(28.))
                            .icon(Icon::new(IconName::RefreshCw).small())
                            .tooltip("Refresh")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("backup")
                            .ghost()
                            .small()
                            .rounded(px(14.))
                            .w(px(28.))
                            .h(px(28.))
                            .icon(Icon::new(IconName::ArrowDownToLine).small())
                            .tooltip("Back up device")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_backup(cx);
                            })),
                    )
                    .child(
                        Button::new("scrcpy")
                            .ghost()
                            .small()
                            .rounded(px(14.))
                            .w(px(28.))
                            .h(px(28.))
                            .icon(Icon::new(IconName::MonitorSmartphone).small())
                            .tooltip(if self.connected_device().is_some() {
                                "Mirror screen"
                            } else {
                                "Connect a device to mirror its screen"
                            })
                            .disabled(self.connected_device().is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.launch_scrcpy(cx))),
                    )
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .rounded(px(14.))
                            .w(px(28.))
                            .h(px(28.))
                            .icon(Icon::new(IconName::Settings).small())
                            .tooltip("Settings")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_settings_dialog(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&self.search_input)
                            .small()
                            .appearance(true)
                            .bordered(false)
                            .cleanable(true)
                            .w(px(168.))
                            .h(px(28.))
                            .rounded(px(14.))
                            .bg(secondary)
                            .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                    ),
            )
    }

    fn render_status_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let side = self.focused_side;
        let count = self.visible_len(side);
        let total = self.entries_for(side).len();
        let selected = self.selected_count(side);

        let summary = if selected > 0 {
            format!("{selected} of {count} selected")
        } else if self.search.trim().is_empty() {
            format!("{count} item{}", if count == 1 { "" } else { "s" })
        } else {
            format!("{count} of {total} shown")
        };

        let state = self
            .selected_device_info()
            .map(|device| device.state.clone())
            .filter(|state| !state.is_ready());
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let status = self.status_line.clone().or_else(|| self.error.clone());

        h_flex()
            .size_full()
            .h(appearance::STATUS_BAR_HEIGHT)
            .items_center()
            .gap_3()
            .px_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .text_xs()
            .text_color(muted)
            .child(div().w(appearance::SIDEBAR_WIDTH).h_full().flex_none())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .when(self.error.is_some(), |el| el.text_color(danger))
                    .child(status.unwrap_or_default()),
            )
            // The item count and device state share the far right edge.
            .child(
                h_flex()
                    .flex_none()
                    .justify_end()
                    .items_center()
                    .gap_3()
                    .child(div().flex_none().child(summary))
                    .when_some(state, |el, state| {
                        let color = match state {
                            crate::model::DeviceStateInfo::Connected => cx.theme().success,
                            crate::model::DeviceStateInfo::Authorizing
                            | crate::model::DeviceStateInfo::Connecting => cx.theme().warning,
                            crate::model::DeviceStateInfo::Unauthorized
                            | crate::model::DeviceStateInfo::NoPermissions => cx.theme().danger,
                            _ => muted,
                        };
                        el.child(
                            h_flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().size(px(6.)).rounded_full().bg(color))
                                .child(state.label()),
                        )
                    }),
            )
    }
}

#[cfg(test)]
impl AppView {
    /// Set while a title-bar press may still become a window drag.
    pub(crate) fn is_titlebar_dragging(&self) -> bool {
        self.titlebar_drag
    }

    /// Representative content for the headless renderer; no phone needed.
    pub(crate) fn load_demo_snapshot_state(&mut self) {
        use crate::model::{DeviceInfo, DeviceStateInfo, EntryKind, FileEntry};

        self.devices = vec![DeviceInfo {
            serial: "DEMO123".into(),
            model: "Pixel 8".into(),
            state: DeviceStateInfo::Connected,
            wireless_serial: None,
        }];
        self.selected_device = Some("DEMO123".into());
        self.device_loading = false;
        self.error = None;

        let names = [
            "Applications",
            "Desktop",
            "Documents",
            "Downloads",
            "Emulators",
            "Library",
            "Movies",
            "Music",
            "Pictures",
            "Public",
            "School",
            "Torrents",
        ];
        self.device_entries = names
            .iter()
            .map(|name| FileEntry {
                name: (*name).to_string(),
                path: format!("/sdcard/{name}"),
                kind: EntryKind::Dir,
                size: 0,
                modified: None,
                mode: 0o755,
                owner: None,
                hidden: false,
            })
            .collect();

        self.host_cwd = std::path::PathBuf::from("/home/demo");
        self.host_loading = false;
        self.host_entries = names
            .iter()
            .map(|name| FileEntry {
                name: (*name).to_string(),
                path: format!("/home/demo/{name}"),
                kind: EntryKind::Dir,
                size: 0,
                modified: None,
                mode: 0o755,
                owner: None,
                hidden: false,
            })
            .collect();

        self.selected.clear();
        self.selected.insert("/sdcard/Pictures".into());
        self.focused_side = Side::Device;
        self.show_host_pane = false;
        self.bump_entries();
    }

    /// A selected entry whose path has to wrap inside Properties.
    #[cfg(test)]
    pub(crate) fn load_properties_demo_state(&mut self) {
        self.load_demo_snapshot_state();
        let path = "/sdcard/Pictures/2026/November/Vacation photos with a long folder name/DCIM";
        if let Some(entry) = self
            .device_entries
            .iter_mut()
            .find(|entry| entry.name == "Pictures")
        {
            entry.path = path.into();
        }
        self.selected.clear();
        self.selected.insert(path.into());
    }

    #[cfg(test)]
    pub(crate) fn apply_devices_for_test(
        &mut self,
        devices: Vec<DeviceInfo>,
        cx: &mut Context<Self>,
    ) {
        self.handle_poll_event(PollEvent::Devices(devices), cx);
    }

    /// No phone attached, second pane open.
    #[cfg(test)]
    pub(crate) fn load_host_only_demo_state(&mut self) {
        use crate::model::{EntryKind, FileEntry};

        self.devices.clear();
        self.selected_device = None;
        self.device_entries.clear();
        self.device_cwd = "/sdcard".into();
        self.show_host_pane = true;
        self.focused_side = Side::Host;

        self.host_cwd = std::path::PathBuf::from("/home/demo");
        self.host_entries = vec![FileEntry {
            name: "Downloads".into(),
            path: "/home/demo/Downloads".into(),
            kind: EntryKind::Dir,
            size: 0,
            modified: None,
            mode: 0o755,
            owner: None,
            hidden: false,
        }];
        self.bump_entries();
    }

    /// Fills the device pane for the render benchmark.
    #[cfg(test)]
    pub(crate) fn load_large_demo_state(&mut self, count: usize) {
        use crate::model::EntryKind;

        self.load_demo_snapshot_state();
        self.device_entries = (0..count)
            .map(|i| crate::model::FileEntry {
                name: format!("IMG_{i:05}.jpg"),
                path: format!("/sdcard/DCIM/IMG_{i:05}.jpg"),
                kind: EntryKind::File,
                size: 4_000_000 + i as u64,
                modified: None,
                mode: 0o644,
                owner: None,
                hidden: false,
            })
            .collect();
        self.bump_entries();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("root")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action::<OpenSettings>(cx.listener(|this, _, window, cx| {
                this.open_settings_dialog(window, cx);
            }))
            .on_action::<Refresh>(cx.listener(|this, _, _, cx| this.refresh(cx)))
            .on_action::<NavigateUp>(cx.listener(|this, _, _, cx| {
                let side = this.focused_side;
                this.navigate_up(side, cx);
            }))
            .on_action::<ToggleHidden>(cx.listener(|this, _, _, cx| {
                this.settings.show_hidden = !this.settings.show_hidden;
                let _ = this.settings.save();
                cx.notify();
            }))
            .on_action::<ToggleHostPane>(cx.listener(|this, _, _, cx| {
                this.toggle_host_pane(cx);
            }))
            .on_action::<FocusSearch>(cx.listener(|this, _, window, cx| {
                let handle = this.search_input.read(cx).focus_handle(cx);
                handle.focus(window, cx);
            }))
            .on_action::<GoBack>(cx.listener(|this, _, _, cx| {
                let side = this.focused_side;
                this.go_back(side, cx);
            }))
            .on_action::<GoForward>(cx.listener(|this, _, _, cx| {
                let side = this.focused_side;
                this.go_forward(side, cx);
            }))
            .on_action::<DeleteSelected>(cx.listener(|this, _, window, cx| {
                this.delete_selected(window, cx);
            }))
            .on_action::<CopySelected>(cx.listener(|this, _, _, cx| {
                let side = this.focused_side;
                this.set_clipboard(ClipboardOp::Copy, side, cx);
            }))
            .on_action::<CutSelected>(cx.listener(|this, _, _, cx| {
                let side = this.focused_side;
                this.set_clipboard(ClipboardOp::Cut, side, cx);
            }))
            .on_action::<Paste>(cx.listener(|this, _, window, cx| {
                this.paste(window, cx);
            }))
            .on_action::<RenameSelected>(cx.listener(|this, _, window, cx| {
                this.open_rename_dialog(window, cx);
            }))
            .on_action::<NewFolder>(cx.listener(|this, _, window, cx| {
                let side = this.focused_side;
                this.open_new_folder_dialog(side, window, cx);
            }))
            .on_action::<NewFile>(cx.listener(|this, _, window, cx| {
                let side = this.focused_side;
                this.open_new_file_dialog(side, window, cx);
            }))
            .on_action::<ShowProperties>(cx.listener(|this, _, window, cx| {
                this.open_properties_dialog(window, cx);
            }))
            // The pointer leaves the 5px seam almost immediately.
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.end_pane_resize(cx);
                } else if this.pane_drag.is_some() {
                    this.update_pane_resize(
                        f32::from(event.position.x),
                        window.viewport_size().width,
                        cx,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_pane_resize(cx);
                }),
            )
            .child(self.render_titlebar(window, cx))
            .child(self.render_main(window, cx))
            .child(self.render_status_bar(window, cx))
    }
}

/// macOS-style lights for platforms without native window controls.
fn traffic_light(id: &'static str, color: Rgba, action: fn(&mut Window)) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .size(px(12.))
        .rounded_full()
        .bg(color)
        .hover(|s| s.opacity(0.85))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, _| action(window))
}

/// The left capsule: native lights sit inside it on macOS, drawn lights elsewhere.
fn render_controls_left(cx: &mut App) -> impl IntoElement {
    let border = cx.theme().border;
    let background = cx.theme().secondary;

    let capsule = h_flex()
        .id("window-controls")
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .ml(px(6.))
        .h(px(32.))
        .w(px(68.))
        .rounded_full()
        .border_1()
        .border_color(border)
        .bg(background)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

    if cfg!(target_os = "macos") {
        capsule
    } else {
        capsule
            .child(traffic_light("window-close", rgba(0xff5f57ff), |window| {
                window.remove_window()
            }))
            .child(traffic_light("window-min", rgba(0xfebc2eff), |window| {
                window.minimize_window()
            }))
            .child(traffic_light("window-max", rgba(0x28c840ff), |window| {
                window.zoom_window()
            }))
    }
}

/// Windows-style caption buttons for the right-hand layout.
fn render_controls_right(window: &Window, cx: &mut App) -> AnyElement {
    if cfg!(target_family = "wasm") {
        return div().into_any_element();
    }

    // The window manager draws its own buttons under server-side decorations.
    #[cfg(target_os = "linux")]
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return div().into_any_element();
    }

    let is_linux = cfg!(target_os = "linux");
    let is_windows = cfg!(target_os = "windows");
    let supported = window.window_controls();

    let foreground = cx.theme().foreground;
    let hover_bg = cx.theme().secondary_hover;
    let active_bg = cx.theme().secondary_active;
    let hover_fg = cx.theme().secondary_foreground;
    let danger = cx.theme().danger;
    let danger_active = cx.theme().danger_active;
    let danger_fg = cx.theme().danger_foreground;

    let caption = move |id: &'static str,
                        icon: IconName,
                        area: WindowControlArea,
                        close: bool,
                        action: fn(&mut Window)| {
        div()
            .id(id)
            .flex_none()
            .h_full()
            .w(px(46.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(foreground)
            .hover(move |s| {
                s.bg(if close { danger } else { hover_bg })
                    .text_color(if close { danger_fg } else { hover_fg })
            })
            .active(move |s| {
                s.bg(if close { danger_active } else { active_bg })
                    .text_color(if close { danger_fg } else { hover_fg })
            })
            .when(is_windows, |el| el.window_control_area(area))
            .when(is_linux, move |el| {
                el.on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, _| action(window))
            })
            .child(Icon::new(icon).small())
    };

    h_flex()
        .id("window-controls")
        .flex_none()
        .h_full()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .when(supported.minimize, |el| {
            el.child(caption(
                "caption-min",
                IconName::WindowMinimize,
                WindowControlArea::Min,
                false,
                |window| window.minimize_window(),
            ))
        })
        .when(supported.maximize, |el| {
            el.child(caption(
                "caption-max",
                if window.is_maximized() {
                    IconName::WindowRestore
                } else {
                    IconName::WindowMaximize
                },
                WindowControlArea::Max,
                false,
                |window| window.zoom_window(),
            ))
        })
        .child(caption(
            "caption-close",
            IconName::WindowClose,
            WindowControlArea::Close,
            true,
            |window| window.remove_window(),
        ))
        .into_any_element()
}
