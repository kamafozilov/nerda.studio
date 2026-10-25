//! Sizes on a 4 px grid. Components use these instead of their own numbers.

use gpui::{Pixels, px};

/// Height of the window's top row, across the sidebar and the main area. The
/// macOS traffic lights are centered on it.
pub const HEADER_HEIGHT: Pixels = px(44.);
/// Width of the sidebar until its edge is dragged, and again after a
/// double-click on its edge.
pub const SIDEBAR_WIDTH: Pixels = px(264.);
/// Narrowest the sidebar's edge drags to: a session row keeps its title, its
/// age and its project's name readable.
pub const SIDEBAR_MIN_WIDTH: Pixels = px(216.);
/// Widest the sidebar's edge drags to: at the window's 720 minimum the card
/// keeps over 300 points.
pub const SIDEBAR_MAX_WIDTH: Pixels = px(400.);
/// Width of the strip over the sidebar's edge that drags it, centered on the
/// edge, half over the card.
pub const SIDEBAR_RESIZE_TARGET: Pixels = px(8.);

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
/// Height of a session's row in the sidebar: its project over its title
/// on `SESSION_LINE_HEIGHT` lines, its branch under them on a 16 point one,
/// `SPACE_2` above and below, 4 and 2 points between: 8 + 20 + 4 + 20 + 2 +
/// 16 + 8.
pub const SESSION_ROW_HEIGHT: Pixels = px(78.);
/// Height of a session row's project line and title line.
pub const SESSION_LINE_HEIGHT: Pixels = px(20.);
/// A session row's content inset from its leading and trailing edges.
pub const SESSION_ROW_INSET: Pixels = px(10.);
/// Corners of a session row's surface.
pub const SESSION_ROW_RADIUS: Pixels = px(8.);
/// Clear space above and below each session row, so with `ROW_GAP` two
/// rows' surfaces stand 5 points apart.
pub const SESSION_ROW_SPACING: Pixels = px(2.);
/// A session's title, a touch over `TEXT_BODY`, so it leads its row.
pub const TEXT_SESSION_TITLE: Pixels = px(14.);
/// Side of the slot before a project's name in a session's row, which its
/// own favicon or logo fills, or its initials on a tile.
pub const PROJECT_ICON: Pixels = px(16.);
/// Corners of a project's icon or tile: a quarter of its side.
pub const PROJECT_ICON_RADIUS: Pixels = px(4.);
/// A project's initials on its tile, in the monospaced font, bold.
pub const PROJECT_INITIALS_SIZE: Pixels = px(8.25);
/// The width each of a project's initials takes on its tile, so two of them
/// span 12 of its 16 points, and one 6, centered.
pub const PROJECT_INITIAL_WIDTH: Pixels = px(6.);
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
/// Height and least width of a large key cap, in a hint that stands alone over
/// the window, such as the one asking for a second Cmd-Q.
pub const KEY_CAP_SIZE_LARGE: Pixels = px(26.);

/// Widest a draft's composer grows; past it the composer stays centered in
/// the card, `SPACE_3` clear of its sides below it.
pub const COMPOSER_MAX_WIDTH: Pixels = px(720.);
/// Widest a session's conversation grows, and the composer docked under
/// it, so tables and code have room; centered as the draft's composer.
pub const CONVERSATION_MAX_WIDTH: Pixels = px(880.);
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

/// What the conversation is read in: the agent's replies and the person's
/// messages, a step over `TEXT_BODY`, which suits controls but tires over a
/// long reply; on lines `READING_LINE_HEIGHT` times as tall.
pub const TEXT_READING: Pixels = px(14.);
pub const READING_LINE_HEIGHT: f32 = 1.6;
/// The agent's headings over `TEXT_READING`, first to third level, on lines
/// `HEADING_LINE_HEIGHT` times as tall.
pub const TEXT_READING_HEADINGS: [Pixels; 3] = [px(20.), px(17.), px(15.)];
pub const HEADING_LINE_HEIGHT: f32 = 1.35;
/// A code block's code, under `TEXT_READING` as monospace reads larger.
pub const TEXT_CODE: Pixels = px(13.);

/// A heading over the main area's content, such as a draft session's question.
pub const TEXT_HEADING: Pixels = px(18.);
/// A second level of heading, as the quit hint's.
pub const TEXT_HEADING_2: Pixels = px(16.);
/// Body text and titles; macOS's standard control size.
pub const TEXT_BODY: Pixels = px(13.);
/// Supporting lines under or over a title, such as a session's project and
/// branch, or a sidebar section's title; `text_secondary` keeps its contrast
/// at any size.
pub const TEXT_SMALL: Pixels = px(12.);
/// The smallest text: a code block's and a tool row's labels.
pub const TEXT_CAPTION: Pixels = px(11.);

pub const SPACE_1: Pixels = px(4.);
pub const SPACE_2: Pixels = px(8.);
pub const SPACE_3: Pixels = px(12.);

/// A switch: its track, and the knob inside it, `SWITCH_INSET` clear of its edge.
pub const SWITCH_WIDTH: Pixels = px(28.);
pub const SWITCH_HEIGHT: Pixels = px(16.);
pub const SWITCH_KNOB: Pixels = px(12.);
pub const SWITCH_INSET: Pixels = px(2.);

/// The Settings page's column: at most this wide, its padding inside, centered
/// in the card, and `SETTINGS_TOP` down from the card's top.
pub const SETTINGS_CONTENT_MAX_WIDTH: Pixels = px(768.);
pub const SETTINGS_TOP: Pixels = px(64.);
/// Between a Settings section's title and its parts, and between its parts.
pub const SETTINGS_PART_GAP: Pixels = px(40.);
/// Every picker on the Settings page is this wide, so their edges line up.
pub const SETTINGS_PICKER_WIDTH: Pixels = px(128.);
/// A Settings section's title, and the headings of its parts.
pub const TEXT_TITLE: Pixels = px(22.);
pub const TEXT_SUBTITLE: Pixels = px(14.);
