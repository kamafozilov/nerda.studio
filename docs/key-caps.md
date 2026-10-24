# Key caps and tooltips

How a shortcut is shown, and the tooltip that names a control. The rest of the
design rules are in [design.md](design.md).

## Key caps

Key caps (`Kbd` in `crates/studio/src/kbd.rs`) show a shortcut beside a button's
label, in a tooltip, a menu item or a hint. A shortcut is one cap, written the
platform's way: modifiers in ⌥⇧⌘ order run together on macOS (`⌘W`, `⇧⌘W`), joined
with a plus elsewhere (`Ctrl+Shift+W`); Return is an icon, Escape `esc` (`Esc`
elsewhere). A cap is `KEY_CAP_SIZE` (20) high and at least as wide, in `TEXT_BODY`,
`key_background` fill with no hairline, `SPACE_1` corners, the key in `text_secondary`;
in a tooltip or a menu item it is `KEY_CAP_SIZE_SMALL` (16) in `TEXT_SMALL`, and in a
hint that stands alone over the window (the second Cmd-Q) `KEY_CAP_SIZE_LARGE` (26) in
`TEXT_HEADING_2`. Inside a
button `FIELD_TARGET_SIZE` (24) high, the cap sits 2 px from its top, bottom and right.
A hint shows one cap per key (↑ ↓ Navigate).

## Tooltips

Tooltips sit centered above their control, or below it in the top row, where there
is no room above; `SPACE_1` clear of it, never at the pointer, so they don't cover the
controls beside it (`crates/studio/src/tooltip.rs`; GPUI's `.tooltip()` follows the
pointer and isn't used). One is `TOOLTIP_HEIGHT` (24) high, hairline included, on
`popover_background` under a `popover_border` hairline and `popover_shadow`, its name in
`text_primary` `SPACE_2` from the left, the key cap `SPACE_2` after it and 3 px inside
the hairline, as far from the right edge as from the top and bottom. It shows
after the pointer rests `TOOLTIP_DELAY` (500 ms); within `TOOLTIP_WARM` (300 ms) of
another, at once. A press hides it.
