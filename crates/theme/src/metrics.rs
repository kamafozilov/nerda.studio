//! Sizes on a 4 px grid. Components use these instead of their own numbers.

use gpui::{Pixels, px};

/// Height of the window's top row, across the sidebar and the main area. The
/// macOS traffic lights are centered on it.
pub const HEADER_HEIGHT: Pixels = px(44.);
/// Width of the sidebar.
pub const SIDEBAR_WIDTH: Pixels = px(260.);

/// Side of a square icon button, which is also its hit area.
pub const CONTROL_SIZE: Pixels = px(28.);
pub const CONTROL_RADIUS: Pixels = px(6.);
/// Side of an icon inside a control.
pub const ICON_SIZE: Pixels = px(16.);
/// Side of an icon inside a field, such as its clear button.
pub const ICON_SIZE_SMALL: Pixels = px(12.);
/// Hit area of a button inside a field, around its `ICON_SIZE` visible part: WCAG
/// 2.5.8's 24 px minimum target.
pub const FIELD_TARGET_SIZE: Pixels = px(24.);
/// Height of one row in a list or menu; rows use `CONTROL_RADIUS` corners.
pub const ROW_HEIGHT: Pixels = px(28.);
/// Width of a menu that opens from a control.
pub const MENU_WIDTH: Pixels = px(220.);

/// Width of a palette: a dialog of one field over a list, such as Add project.
pub const PALETTE_WIDTH: Pixels = px(576.);
/// Height of a palette's field row.
pub const PALETTE_FIELD_HEIGHT: Pixels = px(44.);
/// Height of a palette's list, fixed so the palette keeps its size as it filters.
pub const PALETTE_LIST_HEIGHT: Pixels = px(288.);
/// Corners of a dialog, rounder than a control's.
pub const DIALOG_RADIUS: Pixels = px(12.);
/// Height and least width of a key cap, the shortcut beside a button's label or in
/// a hint: `FIELD_TARGET_SIZE` less a 2 px inset each side.
pub const KEY_CAP_SIZE: Pixels = px(20.);

/// Body text and titles; macOS's standard control size.
pub const TEXT_BODY: Pixels = px(13.);

pub const SPACE_1: Pixels = px(4.);
pub const SPACE_2: Pixels = px(8.);
pub const SPACE_3: Pixels = px(12.);
