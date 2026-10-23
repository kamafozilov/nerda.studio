//! Sizes on a 4 px grid. Components use these instead of their own numbers.

use gpui::{Pixels, px};

/// Height of the window's top row, across the sidebar and the main area. The
/// macOS traffic lights are centered on it.
pub const HEADER_HEIGHT: Pixels = px(44.);
/// Width of the sidebar.
pub const SIDEBAR_WIDTH: Pixels = px(260.);

/// Gap between the main area's card and the window's edges; on the left only
/// while the sidebar is closed, since the open sidebar meets the card.
pub const MAIN_INSET: Pixels = px(6.);
/// Corners of the main area's card.
pub const MAIN_RADIUS: Pixels = px(10.);

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
/// Side of an icon inside a field, such as its clear button, or on a session
/// card's supporting line.
pub const ICON_SIZE_SMALL: Pixels = px(12.);
/// Hit area of a button inside a field, around its `ICON_SIZE` visible part: WCAG
/// 2.5.8's 24 px minimum target.
pub const FIELD_TARGET_SIZE: Pixels = px(24.);
/// Height of one row in a list or menu; rows use `CONTROL_RADIUS` corners.
pub const ROW_HEIGHT: Pixels = px(28.);
/// Height of a session row in the sidebar, a card of three lines in `SPACE_2`
/// padding: the project and the time, the title, then the branch, each line
/// `CARD_LINE` but the title's `CARD_TITLE_LINE`, `SPACE_1` under the first
/// and `CARD_LINE_GAP` over the last (8 + 16 + 4 + 18 + 2 + 16 + 8).
pub const SESSION_ROW_HEIGHT: Pixels = px(72.);
/// A supporting line of a session card, `TEXT_SMALL` text.
pub const CARD_LINE: Pixels = px(16.);
/// A session card's title line, `TEXT_BODY` text.
pub const CARD_TITLE_LINE: Pixels = px(18.);
/// Between a session card's title and the line under it.
pub const CARD_LINE_GAP: Pixels = px(2.);
/// Side of a project badge, the colored square with the project's initials.
pub const BADGE_SIZE: Pixels = px(16.);
/// The initials on a project badge: two capitals fit its 16 px with room.
pub const BADGE_TEXT: Pixels = px(9.);
/// Space between the surfaces of neighboring rows in the sidebar list, where a
/// selected row and a hovered one can sit side by side and must stay two shapes.
pub const ROW_GAP: Pixels = px(2.);
/// Width of a menu that opens from a control.
pub const MENU_WIDTH: Pixels = px(220.);
/// Corners of a menu: its items' `CONTROL_RADIUS` plus the `SPACE_1` between
/// them and its outer edge, hairline included, so the corners nest.
pub const MENU_RADIUS: Pixels = px(10.);
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

/// Widest the session composer grows; past it the composer stays centered in
/// the card, `SPACE_3` clear of its sides below it.
pub const COMPOSER_MAX_WIDTH: Pixels = px(720.);
/// Corners of the composer's box: rounder than a field's, since it holds a
/// toolbar as well as text.
pub const COMPOSER_RADIUS: Pixels = px(12.);
/// Lines of text the composer starts at, so a draft's composer and a docked
/// one stand the same height.
pub const COMPOSER_MIN_LINES: usize = 2;
/// Lines of text the composer grows to before it scrolls inside.
pub const COMPOSER_MAX_LINES: usize = 10;
/// Widest an attachment chip grows; a longer name truncates in the middle.
pub const CHIP_MAX_WIDTH: Pixels = px(200.);
/// Tallest the composer's row of chips grows before it scrolls: two rows and
/// half a third, the half showing there is more, so a draft's composer with
/// ten lines of text still fits the card at the window's minimum size.
pub const CHIPS_MAX_HEIGHT: Pixels = px(78.);

/// A heading over the main area's content, such as a draft session's question.
pub const TEXT_HEADING: Pixels = px(18.);
/// Body text and titles; macOS's standard control size.
pub const TEXT_BODY: Pixels = px(13.);
/// Supporting lines under or over a title, such as a session's project and
/// branch; `text_secondary` keeps its contrast at any size.
pub const TEXT_SMALL: Pixels = px(12.);

pub const SPACE_1: Pixels = px(4.);
pub const SPACE_2: Pixels = px(8.);
pub const SPACE_3: Pixels = px(12.);
