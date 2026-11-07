//! The visual system: palette and shared chrome geometry.

use gpui_kit::component::Theme;
use gpui_kit::{App, Pixels, px, rgba};

pub const SIDEBAR_WIDTH: Pixels = px(212.);
pub const TOOLBAR_HEIGHT: Pixels = px(52.);
pub const STATUS_BAR_HEIGHT: Pixels = px(24.);
pub const PATH_BAR_HEIGHT: Pixels = px(30.);
pub const PANE_DIVIDER_WIDTH: Pixels = px(5.);
pub const MIN_HOST_PANE_WIDTH: Pixels = px(200.);
pub const MIN_DEVICE_PANE_WIDTH: Pixels = px(300.);
pub const COMPACT_LIST_WIDTH: Pixels = px(480.);
pub const LIST_DATE_WIDTH: Pixels = px(116.);
pub const LIST_SIZE_WIDTH: Pixels = px(56.);
pub const LIST_PERMISSIONS_WIDTH: Pixels = px(80.);

/// Dialogs follow the window so they do not look cramped on large screens.
pub fn dialog_width(view_width: Pixels) -> Pixels {
    scaled_dialog_width(view_width, 0.58, 560., 820.)
}

pub fn compact_dialog_width(view_width: Pixels) -> Pixels {
    scaled_dialog_width(view_width, 0.44, 360., 600.)
}

fn scaled_dialog_width(view_width: Pixels, ratio: f32, min: f32, max: f32) -> Pixels {
    px((view_width.as_f32() * ratio).clamp(min, max))
}

pub const LIST_ROW_HEIGHT: f32 = 26.;
pub const GRID_TILE_WIDTH: f32 = 118.;
pub const GRID_TILE_HEIGHT: f32 = 124.;
pub const GRID_COLUMN_GAP: f32 = 4.;
pub const GRID_ROW_GAP: f32 = 10.;
pub const GRID_HORIZONTAL_PADDING: f32 = 18.;
pub const GRID_ROW_HEIGHT: f32 = GRID_TILE_HEIGHT + GRID_ROW_GAP;

/// Palette over the light theme; hex literals carry alpha in the last two digits.
pub fn apply_light_palette(cx: &mut App) {
    Theme::update(cx, |theme| {
        theme.radius = px(6.);
        theme.radius_lg = px(12.);

        let c = &mut theme.colors;
        c.background = rgba(0xffffffff).into();
        c.foreground = rgba(0x1d1f23ff).into();
        c.border = rgba(0xdee4ebff).into();
        c.muted = rgba(0xeef1f5ff).into();
        c.muted_foreground = rgba(0x7b8794ff).into();

        c.title_bar = rgba(0xffffffff).into();
        c.title_bar_border = rgba(0xe4e9f0ff).into();

        c.sidebar = rgba(0xeaf3faff).into();
        c.sidebar_foreground = rgba(0x2b333bff).into();
        c.sidebar_accent = rgba(0xdbe5eeff).into();
        c.sidebar_accent_foreground = rgba(0x1d1f23ff).into();
        c.sidebar_border = rgba(0xd8e1e9ff).into();
        c.sidebar_primary = rgba(0x2e7cf6ff).into();
        c.sidebar_primary_foreground = rgba(0xffffffff).into();

        c.list = rgba(0xffffffff).into();
        c.list_active = rgba(0xe8e9ebff).into();
        c.list_active_border = rgba(0x3d8af7ff).into();
        c.list_hover = rgba(0xeff3f7ff).into();
        c.list_even = rgba(0xf8fafcff).into();
        c.list_head = rgba(0xf4f7faff).into();

        c.selection = rgba(0x0a66d8ff).into();

        c.primary = rgba(0x0a84ffff).into();
        c.primary_foreground = rgba(0xffffffff).into();
        c.primary_hover = rgba(0x2e93ffff).into();
        c.primary_active = rgba(0x0071e3ff).into();

        c.secondary = rgba(0xe9edf2ff).into();
        c.secondary_foreground = rgba(0x1d1f23ff).into();
        c.secondary_hover = rgba(0xe0e6edff).into();
        c.secondary_active = rgba(0xd6dee7ff).into();

        c.tab_bar = rgba(0xeef1f5ff).into();
        c.tab_bar_segmented = rgba(0xe8ecf1ff).into();
        c.tab = rgba(0xffffffff).into();
        c.tab_active = rgba(0xffffffff).into();
        c.tab_foreground = rgba(0x5e6b78ff).into();
        c.tab_active_foreground = rgba(0x1d1f23ff).into();

        c.popover = rgba(0xffffffff).into();
        c.popover_foreground = rgba(0x1d1f23ff).into();
        c.input = rgba(0xd9e0e8ff).into();
        c.ring = rgba(0x8fbdf5ff).into();
        c.drag_border = rgba(0x3d8af7ff).into();
        c.drop_target = rgba(0x3d8af726).into();

        c.scrollbar = rgba(0xffffff00).into();
        c.scrollbar_thumb = rgba(0xc9d2dbff).into();
        c.scrollbar_thumb_hover = rgba(0xb6c1ccff).into();
        c.switch = rgba(0xc9d2dbff).into();
        c.progress_bar = rgba(0x3d8af7ff).into();
    });
}
