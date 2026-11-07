# AndroidFiles

A GPUI desktop file manager for Android devices connected over adb, with a
local "This computer" pane beside the device pane. Rust 2024, one binary.

Everything a contributor needs to know is here: architecture, conventions,
GPUI and adb quirks, and the offscreen debugging harness.

## Quick start

```sh
cargo run          # the real app
cargo build
cargo test         # unit tests + headless render/interaction harness
cargo fmt && cargo clippy --all-targets -- -D warnings
```

Always finish a change with the last line. Fix lints rather than adding
`#[allow]`.

Building the `.deb` requires the `cargo-deb` subcommand to be installed
separately; it is not a crate dependency:

```sh
cargo install cargo-deb
cargo deb
```

The dev profile optimizes dependencies (`opt-level = 3` under
`[profile.dev.package."*"]`) because GPUI's layout, text shaping, and renderer
live in dependencies and are unusably slow when unoptimized. Keep those
settings when touching `Cargo.toml`.

Dependencies are pinned exactly:

- `gpui-kit = "=0.9.0"` — the single UI dependency; `use gpui_kit::*;`
  re-exports GPUI. Layers: `gpui_kit::component` (styled components, built on
  `gpui-component`), `gpui_kit::base` (behavior, `gpui-base`), `gpui_kit::assets`,
  `gpui_kit::platform`.
- `adb_client = "=2.1.19"`.

Never invent a gpui API. Check the actual signature in
`~/.cargo/registry/src/*/gpui-kit-0.9.0/`, `gpui-component-0.9.0/`,
`gpui-base-0.9.0/`, or `gpui-pre-0.3.11/`. A plausible-looking method that does
not exist is the most common failure here.

## Layout of the source

| Path | Purpose |
| --- | --- |
| `src/main.rs` | Window options, platform chrome, `AppView` construction |
| `src/app.rs` | `AppView` root entity: state, actions, listing, navigation, transfers, title bar, toolbar, status bar |
| `src/file_view.rs` | Pane rendering: path bar, virtualized list/grid, divider, click/drag/drop |
| `src/sidebar.rs` | Sidebar: shortcuts, favorites, storage locations, transfers |
| `src/dialogs.rs` | Settings, new folder/file, rename, delete confirm, pull target, properties |
| `src/settings.rs` | Versioned `settings.json` load/save and preference enums |
| `src/model.rs` | `FileEntry`, `EntryKind`, `Side`, device/transfer/clipboard/favorite types |
| `src/appearance.rs` | Layout constants and the light-theme palette override |
| `src/icons.rs` | File/folder artwork and Lucide glyph selection |
| `src/format.rs` | Size, speed, date, mode, device-label formatting |
| `src/actions.rs` | `actions!` declarations only |
| `src/backend/adb.rs` | `AdbContext`: device listing plus every adb operation |
| `src/backend/host.rs` | Local filesystem operations |
| `src/backend/progress.rs` | `CountingReader`/`CountingWriter` and `ProgressSink` |
| `src/backend/polling.rs` | Background device poll thread |
| `src/backend/resolve.rs` | adb binary discovery |
| `src/backend/backup.rs` | Selective backup walk over adb |
| `src/tests/` | Test-only headless rendering and interaction harness |
| `src/assets/` | Unused SVGs; icons are drawn with divs |

`app.rs`, `file_view.rs`, `sidebar.rs`, and `dialogs.rs` are sibling
`impl AppView` modules, not separate entities. Add to the nearest one instead
of creating a new state owner. `Sidebar`/`SettingsView` are small helper
entities only where a controlled input or switch must re-render.

## How the app is put together

`main` calls `gpui_kit::init`, then opens one window whose root is
`cx.new(|cx| AppView::new(window, cx))`. `AppView::render` is a column:

```
title bar → main (sidebar + host pane or device pane) → status bar
```

With no device, the host pane fills the main area. With a device attached,
the device pane fills it by default; opening the host pane splits the two.
Clicking **This computer** expands the PC pane over the device (`host_only`),
and clicking it again returns to the device. Selecting or navigating a device
leaves PC-only mode.

All device and host I/O happens on the background executor (`cx.spawn` +
`background_executor().spawn`); results are applied in an entity update. Never
block the UI thread with adb or filesystem calls.

State lives in `AppView`:

- `settings`, `adb_path`, `devices`, `selected_device`.
- Per side: `*_cwd`, `*_entries`, `*_loading`, navigation back/forward stacks,
  and a `VirtualListScrollHandle`.
- One `selected: HashSet<String>` and one `focused_side` shared by both panes.
- `clipboard`, `transfers`, `backup_entries`, `status_line`, `error`.
- View state: `view_mode`, `show_host_pane`, `host_pane_width`,
  `host_pane_custom`, `pane_drag`, `titlebar_drag`, `drag_over`, `select_drag`.
- `entries_revision` + `visible_cache` (a `RefCell` display-order cache).

### Display-order cache

`visible_entries`/`visible_len` filter (`show_hidden`, search) and sort
(directories first, case-insensitive name) into `VisibleCache`, keyed by
`entries_revision`, `search`, and `show_hidden`. Whenever a pane's entry list
is replaced you MUST call `bump_entries()`; mutating entries without bumping
leaves stale rows. Tests cover the case where one pane is empty and must not
blank the other.

### Paths and sides

Host paths are `PathBuf`; device paths are POSIX `String`s. Use
`remote_join` for device paths and `shell_quote` for anything interpolated
into a shell command. `Side::{Host,Device}` selects pane, cwd, entries, and
navigation stack throughout.

### Opening and selecting

`open_entry` navigates directories on both sides; host files are opened with
the OS handler (`xdg-open`, `open`, `cmd /C start`) from a background spawn and
failures land in `status_line`. Files on the device have no opener.

Default is double-click to open. `Settings::open_on_single_click` switches both
the list row and grid tile to open on one click; Ctrl/Cmd-click still only
extends selection. Right-clicking an unselected row or tile selects it before
opening the context menu; right-clicking a selected one keeps the selection.

Pressing the left button on an unselected row or tile and moving over others
sweeps them into the selection (`select_drag`). A selected row starts a
transfer drag instead, so the sweep must start on an unselected item. Clicking
empty list space clears the selection; row and tile presses stop propagation
so the blank-space handler does not run. Pointer downs suppress window text
selection, including the pane-divider drag.

Context-menu Copy, Cut, Delete, Push, and Pull act on the whole selection;
Rename and Properties act on the clicked item. The clipboard stores every
selected item, and cross-side pastes push or pull them together. The drag
ghost sits just below-right of the pointer instead of inheriting the row grab
offset. Folder rows highlight as drop targets, not the whole pane; the
Internal storage row and favorite rows also accept device/host and external
drops.

Pasting with an empty clipboard pushes an info toast instead of doing nothing.

### Window chrome and theme

- Minimum window 900×600; default 1120×720, centered.
- macOS keeps native decorations and native traffic lights (an outlined capsule
  is drawn around them); other platforms use client decorations with drawn
  controls on the side chosen by `Window controls` (`Auto`/`Left`/`Right`).
- `Auto` resolves per platform in `controls_on_left`.
- Background: blur on macOS, Mica on Windows, transparent elsewhere, unless
  translucency is disabled in settings.
- Theme defaults to `System`. `ThemePref::Light` additionally applies the
  palette in `appearance.rs`; dark uses gpui-component's own dark theme.
  `apply_theme` runs on construction and on window appearance changes.
- Pressing empty title bar space starts a window drag; toolbar controls, the
  search field, and caption buttons stop pointer-down propagation.

### Key bindings

Bindings are registered in `AppView::new` with `cx.bind_keys`, scoped to
`KEY_CONTEXT`, and dispatched by `on_action` handlers on the root element.
Cmd/Ctrl pairs exist for macOS and Linux/Windows. The root tracks
`focus_handle` and takes focus in `new`, so the dispatch path always includes
the handlers even when no child is focused. The context scope lets a focused
input win on shared keys (delete, copy/cut/paste, alt-left/right) while
app-only chords still fire.

Settings (`ctrl-,`), refresh (`ctrl-r`), alt-up (navigate up), delete,
copy/cut/paste, new folder (`ctrl-shift-n`), new file (`ctrl-n`), enter
(rename), properties (`ctrl-i`), hidden files (`ctrl-h`/`ctrl-shift-h`), split
view (`ctrl-shift-b`), focus search (`ctrl-f`), back/forward (alt-left/right).

**Backspace is deliberately unbound.** Use alt-up to navigate up.

## GPUI and gpui-kit notes

- A bare `div()` is display block. Use `h_flex()`/`v_flex()`, or add `.flex()`,
  before relying on `flex_col`/`flex_1`.
- gpui-kit renders SVG assets as **alpha masks** — a colored SVG becomes a flat
  silhouette. The folder artwork in `icons.rs` is layered divs and gradients
  with a Lucide glyph on top.
- Text is not selectable in this gpui version. The properties dialog makes
  values copyable with borderless, read-only `Input`s
  (`.readonly(true).appearance(false).bordered(false).focus_bordered(false).small()`).
  The path value uses a read-only `Textarea` with `auto_grow` so long paths wrap.
- Inputs are stateful: hold `Entity<InputState>`, render with `Input::new(&state)`,
  read with `.read(cx).value()`, and subscribe to `InputEvent::Change`.
- Dialogs: `window.open_dialog(cx, ...)` with `.centered()` and
  `.entrance(DialogEntrance::FadeSlide)`. Assert with `window.has_active_dialog(cx)`.
  Enter and Escape run the dialog's `on_ok`/`on_cancel`; a dialog with a custom
  `.footer(...)` must also install `.on_ok(...)`, or Enter closes it without
  running the footer action.
- Use `cx.theme()` tokens for color. The only raw hex is the light palette in
  `appearance.rs`, where the last two digits are alpha.
- Repeated elements need stable, domain-derived `ElementId`s, not indexes.
- Lists and grids are virtualized (`v_virtual_list`); keep per-row work
  constant, and keep pure view state out of render-time mutation.

## Settings

`settings.json` lives at `dirs::config_dir()/AndroidFiles/settings.json` and is
serialized with `#[serde(default)]` on the container, so older files keep
loading when fields are added. `version` is 2; a v1 file is migrated by
resetting the theme to `System` (v1's default was fixed dark). Bump the version
only when an old *default* must be overwritten, and never re-run a one-time
migration on later files.

Fields: `theme`, `window_controls`, `translucent_window`, `adb_path`,
`scrcpy_path`, `concurrent_transfers`, `show_hidden`,
`open_on_single_click`, `confirm_before_delete`, `default_download_folder`,
`favorites`, `wireless_devices`.

`concurrent_transfers` is persisted but not yet used by any code path.
`scrcpy_path` overrides the binary launched by the toolbar screen-mirror
button; that launch is refused unless the selected device is ready.

## adb backend

`AdbContext` holds the optional binary path and the adb server socket
(127.0.0.1:5037). `device(serial)` constructs an `ADBServerDevice` per call.
The library calls `connect()` — a fresh TCP connection, transport selection,
and host-features round trip — for **every** command and sync transfer, so
per-command cost is inherent; do not construct extra devices per command where
one can be shared.

Wireless debugging uses the adb server's `host:pair` and `host:connect`
commands: `pair_wireless` takes the pairing address and code, and
`connect_wireless` takes the address from the Wireless debugging screen.
Both accept IPv4 `IP:port` only because adb_client uses `SocketAddrV4`.
"Already paired" and "already connected" responses count as success. Pairing
is a separate dialog with only the pairing IP, port, and code; it never asks
for the connection address. The **Add wireless device** dialog lists saved
hosts, fills the device IP from one, and connects with the device address and
port shown on the Wireless debugging screen. Successful pairings and
connections are saved in `wireless_devices`. Device listing collapses TCP
transports sharing a host and, when `ro.serialno` matches, the USB and
wireless transports for one phone, preferring a ready USB link. A row keeps
the hidden wireless serial so its disconnect action works even when the USB
transport is the one shown; `disconnect_wireless_serial` uses `host:disconnect`
for `IP:port` serials and leaves the address paired. mDNS serials
(`adb-…_adb-tls-connect._tcp`) cannot go through adb_client's
`SocketAddrV4` API, so they are disconnected with `adb disconnect <serial>`.

### Shell commands

Pass the command as a single argument:

```rust
dev.shell_command(&[cmd], &mut out)
```

adb_client sends it as `shell,v2,...,raw:<cmd>`, and the device runs that raw
string through `sh -c` itself. Wrapping it in `["sh", "-c", cmd]` produces
`sh -c <first word>` on the device and silently mangles every multi-word
command.

`shell_command` returns `Ok` at EOF, not on exit status. `run_status` appends
`; echo __ANDROIDFILES_EXIT__:$?` and parses the trailing marker; mutations use
it to report real failures.

### Listing

`list_dir` asks adb for names/time/size/mode, then resolves symlinks in
batched shell chunks (64 per command): each record is `index|d/f|target`, so
the target may contain any bytes after the second separator. Sorting is
directories-first then case-insensitive, using `sort_by_cached_key` so names
are lowercased once. If resolution fails, the link is shown as `Symlink` with
an empty target.

### Transfers

`pull_dir`/`push_dir` scan the tree once (`scan_remote`/`scan_local`), use that
same tree both for the progress total and for the copy plan, share one device
for the whole tree, and batch remote `mkdir -p` calls (64 paths per command).
Cancel is an `AtomicBool`. Push does not follow directory symlinks (loops).
Symlinks are skipped in tree scans, matching prior behavior.

`pull_file`/`push_file` are the single-entry entry points used by the app and
by `backup.rs`. A failed pull keeps its transfer row in the failed state and
also opens an alert dialog with the error; permission-denied gets its own
title.

`guard_delete` refuses mount roots, shared-storage roots, and a fixed list of
system directories. Extend that list rather than relying on `rm -rf` failures.

`resolve_adb_binary` order: user override → `ANDROID_HOME`/`ANDROID_SDK_ROOT` →
`PATH` → common SDK locations.

`polling.rs` runs a thread that emits only on change, re-resolves the binary
only while it is missing, and backs off 1s → 30s (100 ms shutdown checks)
when idle. While any device is present it polls every 2s so disconnects show
up quickly.

### Known sharp edges

- `backup.rs` predates the tree-scan helpers: it calls `remote_dir_size` (a
  full listing walk) and then lists every directory again, and it builds a new
  device per directory. A backup of a large tree pays for two listings.
- `list_device_infos` runs one `getprop` shell command per device on every poll
  tick that reaches the server; fine at the 30 s idle cadence, wasteful if a
  caller polls quickly.
- Progress is coalesced by `ChannelSink` to one UI update per 100 ms; adding a
  second sink should do the same.

## Debugging and visual checks

No display server is needed: the harness opens hidden headless windows, draws
real frames, and writes binary PPMs to `/tmp`.

### Frames

```sh
cargo test --bin AndroidFiles render_app_snapshot
for frame in /tmp/androidfiles*.ppm; do ffmpeg -loglevel error -y -i "$frame" "${frame%.ppm}.png"; done
```

| File | Window | State |
| --- | --- | --- |
| `/tmp/androidfiles.ppm` | 1120×720 | grid, single pane |
| `/tmp/androidfiles_left_controls.ppm` | 1120×720 | controls capsule on the left |
| `/tmp/androidfiles_list.ppm` | 1120×720 | list, single pane |
| `/tmp/androidfiles_split.ppm` | 1120×720 | grid, split panes |
| `/tmp/androidfiles_split_list.ppm` | 1120×720 | list, split panes |
| `/tmp/androidfiles_narrow_split_list.ppm` | 900×600 | list, split panes |
| `/tmp/androidfiles_no_device.ppm` | 1100×700 | PC pane full, no phone attached |
| `/tmp/androidfiles_host_only.ppm` | 1120×720 | PC pane full over an attached device |
| `/tmp/androidfiles_long_path.ppm` | 1100×700 | deep host path so the bar wraps |
| `/tmp/androidfiles_settings.ppm` | 1120×720 | settings dialog |
| `/tmp/androidfiles_properties.ppm` | 1120×720 | properties dialog, copyable values |
| `/tmp/androidfiles_wireless.ppm` | 1120×720 | add wireless device dialog |
| `/tmp/androidfiles_sidebar_devices.ppm` | 1120×720 | two devices, selected row, Paths shortcuts |
| `/tmp/androidfiles_sidebar_transfers.ppm` | 1120×720 | sidebar scrolled through many transfers |
| `/tmp/androidfiles_disconnected.ppm` | 1120×720 | device removed after a loaded folder |
| `/tmp/androidfiles_pair_wireless.ppm` | 1120×720 | standalone pairing dialog |
| `/tmp/androidfiles_scrollbar.ppm` | 1120×720 | split panes with overflowing lists |

`settings_button_opens_dialog` also asserts that clicking the toolbar gear
activates a dialog, and captures after `HeadlessAppContext::advance_clock`
because the fade entrance samples the executor clock and headless time only
moves when advanced.

### Add a frame

All frames live in `render_app_snapshot` in `src/tests/mod.rs`.

1. Set state on the view. `load_demo_snapshot_state()` fills demo devices, host
   and device entries, and a selection with no phone attached;
   `load_host_only_demo_state()` is the no-device case with a populated host
   pane; `load_large_demo_state(n)` fills the device pane for benchmarking.
   Test-only view fields (`view_mode`, `show_host_pane`, `host_pane_custom`,
   `host_pane_width`, `focused_side`) are directly writable.
2. Draw and clear one frame:
   `window.refresh(); window.draw(cx).clear(cx);`
3. Read it with `cx.capture_screenshot(handle)` and write the RGB bytes as
   `P6\n{width} {height}\n255\n...`.

Headless windows do not re-layout when resized after creation; to test another
size, open a new window with those bounds (the 900×600 frame shows the pattern).

### Interactions in tests

Dispatch real input through the window:

```rust
window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { /* ... */ }), cx);
window.dispatch_keystroke(Keystroke { modifiers, key, key_char }, cx);
```

Frame listeners and hitboxes belong to the **last drawn frame**, so draw before
dispatching inside the same `update_window` call; an undrawn update in between
drops them.

Tests must not call `Settings::save`; it writes the real user config. Exercise
pure settings helpers instead.

Covered: title-bar drag vs. toolbar press, divider drag, empty-device listing,
sidebar routing to the selected device, right-click selection, settings dialog,
properties dialog, backspace/delete editing the search field, ctrl-shift-n
opening a dialog, multi-item clipboard, blank-space selection clearing,
favorite reordering, transfer expiry, the pane scrollbars, sidebar scrolling,
PC-only expansion over a device, and the empty-paste toast.

### What a correct frame looks like

- Root column fills the window: title bar, panes, status strip.
- Title bar spans the full width; window controls follow the setting.
- Every pane has a path bar above its listing; the focused pane's bar is tinted
  and its crumbs navigate; crumbs wrap and never paint over the other pane.
- Sidebar shortcuts target the selected device; with no device they target the
  PC.
- **Devices** lists every attached device and highlights the selected one.
  Clicking a device selects it and opens its internal storage. The only device
  is selected automatically.
- **Paths** contains exactly `/` and `/sdcard` (`/storage/emulated/0`) for the
  selected device. Device rows show the model (serial only as fallback).
  Favorites are filtered to the selected device.
- **Add wireless device** sits at the bottom of the sidebar, centred.
- Favorite rows can be dragged onto another favorite row to reorder them;
  the order is saved immediately. Finished transfers disappear five seconds
  after completing. The whole sidebar scrolls when its content overflows.
- List panes narrower than `COMPACT_LIST_WIDTH` (480 px) show Name and Size
  only; wider panes add Date Modified and Permissions. Name stays left-aligned;
  the info columns are right-aligned with compact widths.
- Each pane overlays a slim scrollbar at its right edge. It appears while
  scrolling only when content overflows, then fades after idle; a window
  resize that makes the content fit leaves no scrollbar behind.
- Divider sits at `SIDEBAR_WIDTH + host_pane_width` with a 5 px grab band.
  Until dragged the split is equal and follows the window; afterwards the
  chosen width is clamped so the device pane keeps `MIN_DEVICE_PANE_WIDTH`.
- With no device, the PC pane fills the main area; the device path bar,
  Internal storage, Devices, and Paths disappear. Favorites stays for host
  shortcuts, quick locations target the PC, and device-only favorites are
  hidden. Stale listings are cleared, including in-flight loads that finish
  after the device goes away.
- With a device attached, **This computer** expands the PC pane to the same
  full width while the device pane is hidden; the device rows stay in the
  sidebar so a click can bring the device back.
- Status summary is left, item count is the far-right trailing item.

### Live capture

Prefer the harness. If the bug needs a real compositor:

- Sway/Hyprland: `grim /tmp/app.png`
- GNOME: `gnome-screenshot -f /tmp/app.png`
- Plasma: `spectacle -b -n -o /tmp/app.png`
- X11: `import -window root /tmp/app.png` or `maim /tmp/app.png`

X11 root capture under Xwayland can be black; use the compositor's tool or the
harness.

### Render benchmark

```sh
cargo test --bin AndroidFiles bench_large_folder_render -- --ignored --nocapture
```

It fills the device pane with 5,000 entries and prints per-frame cost for grid,
list, and list with half the rows selected. A healthy build is roughly 1–3 ms
per frame; hundreds of ms means rows are being rebuilt instead of virtualized.

## Conventions

- Comments explain *why* only: constraints, workarounds, invariants, protocol
  quirks. No banners, dividers, TODOs, or doc comments on private/trivial
  items. Keep two lines max.
- `?` over forwarding `match`; `let else`/`if let`/`matches!` for single-arm
  cases; iterator chains over manual push loops; field-init shorthand.
- Follow the existing module split and naming; no new abstraction for a
  one-off. No feature flags or config options that were not asked for.
- Keep layout numbers in `appearance.rs`; keep color in theme tokens.
- Error handling belongs at trust boundaries (user input, filesystem, adb) —
  not around cases that cannot happen.
- User-facing text names the object and the verb ("Delete \"X\"?", not
  "Are you sure?").
- Match `cargo fmt` output; clippy is treated as an error gate.

## Change checklist

- UI change: `cargo test`, inspect the relevant frame(s), check narrow and
  split layouts, confirm focus/selection/disabled states are visible.
- adb change: the build is the only automated check unless a device is
  attached. Say so explicitly when reporting that something is unverified on
  hardware. Keep one device per logical operation and avoid extra shell round
  trips.
- Settings change: add the field with a sane `Default`; container-level
  `serde(default)` keeps old files loading. Only bump `version` if an old
  default must be overwritten.
- Always run `cargo fmt` and `cargo clippy --all-targets -- -D warnings`
  before finishing.
