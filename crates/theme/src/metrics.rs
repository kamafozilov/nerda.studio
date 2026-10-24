//! Sizes on a 4 px grid. Components use these instead of their own numbers.

use gpui::{Pixels, px};

/// Height of the window's top row, across the sidebar and the main area. The
/// macOS traffic lights are centered on it.
pub const HEADER_HEIGHT: Pixels = px(44.);
/// Width of the sidebar.
pub const SIDEBAR_WIDTH: Pixels = px(240.);

/// Gap between the main area's card and the window's edges; on the left only
/// while the sidebar is closed, since the open sidebar meets the card.
pub const MAIN_INSET: Pixels = px(6.);
/// Corners of the main area's card.
pub const MAIN_RADIUS: Pixels = px(12.);

/// Widest a tab in the top row grows; tabs share the row below that.
pub const TAB_MAX_WIDTH: Pixels = px(224.);
/// Narrowest the selected tab shrinks while the others keep giving way.
pub const TAB_SELECTED_MIN_WIDTH: Pixels = px(112.);
/// Narrower than this, a tab drops its close button and a click anywhere on it
/// selects it.
pub const TAB_CLOSE_MIN_WIDTH: Pixels = px(72.);
/// Space between neighboring tabs.
pub const TAB_GAP: Pixels = px(2.);

/// Side of a square icon button, which is also its hit area.
pub const CONTROL_SIZE: Pixels = px(28.);
pub const CONTROL_RADIUS: Pixels = px(6.);
/// Opacity of a control or a menu item that can't act yet: it stays in view,
/// its tooltip saying why. Disabled controls are exempt from contrast minimums.
pub const DISABLED_OPACITY: f32 = 0.4;
/// Side of an icon inside a control.
pub const ICON_SIZE: Pixels = px(16.);
/// Side of an icon in a sidebar row, a sidebar header's button or a menu
/// item: a touch under `ICON_SIZE`, beside `TEXT_BODY`.
pub const ICON_SIZE_ROW: Pixels = px(14.);
/// Side of an icon inside a field, such as its clear button.
pub const ICON_SIZE_SMALL: Pixels = px(12.);
/// Hit area of a button inside a field, around its `ICON_SIZE` visible part: WCAG
/// 2.5.8's 24 px minimum target.
pub const FIELD_TARGET_SIZE: Pixels = px(24.);
/// Height of one row in a list or menu; rows use `CONTROL_RADIUS` corners.
pub const ROW_HEIGHT: Pixels = px(28.);
/// Side of a project badge, the colored square with the project's initials.
pub const BADGE_SIZE: Pixels = px(16.);
/// The initials on a project badge: two capitals fit its 16 px with room.
pub const BADGE_TEXT: Pixels = px(9.);
/// Space between neighboring rows in the sidebar, where a selected row and a
/// hovered one can sit side by side and must stay two shapes.
pub const ROW_GAP: Pixels = px(1.);
/// Height over which a list fades out into the bar it scrolls under while
/// more of it lies past that edge.
pub const SCROLL_EDGE_FADE: Pixels = px(32.);
/// Width of a menu that opens from a control.
pub const MENU_WIDTH: Pixels = px(220.);
/// Corners of a menu: a little rounder than its items' `CONTROL_RADIUS`,
/// which stand `SPACE_1` in from its edge.
pub const MENU_RADIUS: Pixels = px(8.);
/// A hairline: the 1 px border of a field, a card or a menu. Content inside a
/// bordered box is padded this much less, so it lines up with unbordered rows.
pub const HAIRLINE: Pixels = px(1.);

/// Width of a palette: a dialog of one field over a list, such as Add project.
pub const PALETTE_WIDTH: Pixels = px(576.);
/// Height of a palette's field row.
pub const PALETTE_FIELD_HEIGHT: Pixels = px(44.);
/// Height of a palette's list, fixed so the palette keeps its size as it filters.
pub const PALETTE_LIST_HEIGHT: Pixels = px(288.);
/// Corners of a dialog, rounder than a control's.
pub const DIALOG_RADIUS: Pixels = px(12.);
/// Height of a tooltip, hairline included.
pub const TOOLTIP_HEIGHT: Pixels = px(24.);
/// Tallest a notice's text grows before it scrolls: six and a half lines, the
/// half showing there is more, so the list and the footer keep their place.
pub const NOTICE_MAX_HEIGHT: Pixels = px(136.);
/// Height and least width of a key cap, the shortcut beside a button's label or in
/// a hint: `FIELD_TARGET_SIZE` less a 2 px inset each side.
pub const KEY_CAP_SIZE: Pixels = px(20.);
/// Height and least width of a small key cap, the shortcut in a tooltip or a
/// menu item: it stands 3 px inside a tooltip's hairline.
pub const KEY_CAP_SIZE_SMALL: Pixels = px(16.);

/// Widest the session composer grows; past it the composer stays centered in
/// the card, `SPACE_3` clear of its sides below it.
pub const COMPOSER_MAX_WIDTH: Pixels = px(720.);
/// Corners of the composer's box: rounder than a field's, since it holds a
/// toolbar as well as text.
pub const COMPOSER_RADIUS: Pixels = px(12.);
/// Lines of text the composer starts at, so a draft's composer and a docked
/// one stand the same height.
pub const COMPOSER_MIN_LINES: usize = 3;
/// Lines of text the composer grows to before it scrolls inside.
pub const COMPOSER_MAX_LINES: usize = 10;
/// Widest an attachment chip grows; a longer name truncates in the middle.
pub const CHIP_MAX_WIDTH: Pixels = px(200.);
/// Tallest the composer's row of chips grows before it scrolls: two rows and
/// half a third, the half showing there is more, so a draft's composer with
/// ten lines of text still fits the card at the window's minimum size.
pub const CHIPS_MAX_HEIGHT: Pixels = px(78.);

/// The widest share of the conversation's column a message of the person's
/// takes, so it reads as theirs, apart from the agent's full-width replies.
pub const MESSAGE_MAX_WIDTH: f32 = 0.8;

/// A heading over the main area's content, such as a draft session's question.
pub const TEXT_HEADING: Pixels = px(18.);
/// Body text and titles; macOS's standard control size.
pub const TEXT_BODY: Pixels = px(13.);
/// Supporting lines under or over a title, such as a session's project and
/// branch, or a sidebar section's title; `text_secondary` keeps its contrast
/// at any size.
pub const TEXT_SMALL: Pixels = px(12.);
/// The smallest text: a session row's age.
pub const TEXT_CAPTION: Pixels = px(11.);

pub const SPACE_1: Pixels = px(4.);
pub const SPACE_2: Pixels = px(8.);
pub const SPACE_3: Pixels = px(12.);
