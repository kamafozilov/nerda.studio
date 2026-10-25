# Design

Nerda Studio looks like a native glass app: the desktop shows through the window, blurred,
and the work sits on a main area raised over it. The `theme` crate holds every value below;
components read them through `Theme::of(cx)` and the constants in `theme`, never as literals.

## Materials

| Layer | What it is | Token |
| --- | --- | --- |
| Window base | The blurred desktop under a tint. The top row and the sidebar sit on it directly. | `window_background` |
| Main area | A card raised over the base: `MAIN_RADIUS` (12) corners and a hairline, `MAIN_INSET` (6) clear of the window's right and bottom edges, and of its left while the sidebar is closed. | `panel_background`, `border` |

- **Glass** on macOS and Windows: the window uses `WindowBackgroundAppearance::Blurred`
  and the light base tint is translucent (dark's is opaque, see Colors). Measured on
  macOS 27, GPUI's blur (`NSVisualEffectView`, `UnderWindowBackground`, behind the window)
  adds no tint: with a zero tint, a white desktop shows through as 255. Windows' blur adds
  none either. The tint alone keeps text readable.
- **Solid** on Linux, where no blur is guaranteed, and wherever the user turned on
  "Reduce transparency" (macOS) or turned off "Transparency effects" (Windows): the base
  is opaque, the same colors otherwise. `crates/studio/src/system_theme.rs` rereads the
  setting when the window comes back to front, so a change made in System Settings applies
  without a restart.
- **The glass is in the top row and the sidebar.** The main area stays near solid, about
  5% of the desktop showing through in light and none in dark, because its content (code,
  chat, a design canvas) must read the same on any desktop and keep its own colors.
- Do not stack another translucent surface on the main area. GPUI blurs only what is behind
  the window, not the app's own content, so a menu or popover needs an opaque background of
  its own.
- The window follows the system's light or dark appearance, live.

## Colors

Colors are roles: a token names its job, and a component uses it only in that job. A role
that is missing gets a new token, computed and measured as below, never a borrowed one.

Every value comes from OKLCH with one cool neutral hue (h 265, C 0.006; text C 0.009), in
`crates/theme/src/palette.rs` as `0xRRGGBBAA`, except dark, which is neutral: grays and
text with no hue, washes of white. Dark appearance has no glass: its base is opaque, so
the sidebar and the tabs read the same over any desktop. Light keeps its glass.

| Role | Dark | Light | Use |
| --- | --- | --- | --- |
| `window_background` (glass) | `0a0a0b`, opaque | `f3f5f9` at 81% (`ce`) | Window base, sidebar |
| `window_background` (solid) | `0a0a0b` | `f3f5f9` | Window base without blur |
| `panel_background` | `131314` | `fbfdff` at 72% | Main area over the base |
| `tab_background` | `131313` | text primary at 4% | A tab that isn't selected |
| `composer_background` | `202021` | `ffffff` | The composer's box |
| `popover_background` | `202021` | `fbfdff` | Menus, tooltips, palettes, dialogs; opaque |
| `popover_border` | white at 12% | text primary at 14% | Edge of a menu, tooltip or hint; brighter than `border` |
| `popover_shadow` | black at 35% | black at 13% | Each of the three shadows under a menu |
| `composer_glass` | `202021` at 88% | `ffffff` at 88% | The composer's box while a draft shows a pattern or an image |
| `field_background` | white at 6% | text primary at 6% | Inline code, code blocks, tables, tool output, the person's messages, a segmented control's track |
| `chip_background` | white at 6% | text primary at 6% | An attachment chip at rest |
| `text_selection` | white at 20% | text primary at 20% | Highlight behind selected text |
| `tab_selected` | `202021`, under a `border` hairline | text primary at 10% | The selected tab |
| `border` | white at 8% | text primary at 10% | Sidebar edge, dividers |
| `hover`, `row_selected` | white at 8%, both | text primary at 6%, 10% | Hovered control or row; the selected row |
| `text_primary` | `f5f5f5` (L 0.97) | `191b1f` (L 0.22) | Titles, body, hovered icons |
| `text_secondary` | `a0a0a1` (L 0.71) | `505358` (L 0.44) | Supporting text, icons at rest |
| `text_secondary_on_surface` | `a0a0a1` | `45474c` (L 0.399) | Supporting text on a row's `hover` or `row_selected` surface |
| `text_tertiary` | `797979` (L 0.57), 4.55 on the base | `505358` | Section titles, a session's branch, menu icons |
| `scrim` | `151619` at 64% | `fbfdff` at 60% | Dims the window under a dialog; nothing is read on it |
| `key_background` | text primary at 10% | text primary at 10% | Key caps; `text_secondary` on it reads 4.57 / 6.17 in a tooltip, but under `hover` falls below 4.5:1: 3.58 in dark on a highlighted menu item, 3.38 in light on the sidebar's New session row over a black desktop |
| `badges` | fill L 0.38 C 0.075, initials L 0.87 C 0.10 | fill L 0.91 C 0.045, initials L 0.43 C 0.12 | Projects' hues, eight (25, 60, 95, 145, 185, 235, 285, 330), chroma cut to fit sRGB; a project without its own icon shows its initials on its fill; the initials keep at least 6.73:1 (dark) and 5.98:1 (light) on their fill, 5.55 and 5.40 under a row's surface |

Roles that belong to one screen are set where it is described: `danger`, `diff_added`
and its washes, `link`, `mention`, `syntax` and `activity` in [session.md](session.md);
`claude`, `effort`, `backdrop` and `backdrop_warm` in [backdrop.md](backdrop.md) and
[composer.md](composer.md).

### Contrast on glass

Dark is opaque, so its contrast holds over any desktop: `text_primary` reads 18.15 on
the base, 17.03 on the card and a tab, 14.93 on the selected tab; `text_secondary` 7.57,
7.11 and 6.23. In light each tint is the most
transparent 8-bit alpha that keeps `text_secondary` at WCAG AA, 4.5:1, over the worst
desktop for its appearance: white behind dark, black behind light. A clearer glass would
fail it there. WCAG 2 contrast on the composited surface, the lower over a white and a
black desktop (dark / light):

| Text | Sidebar | Main area |
| --- | --- | --- |
| `text_primary` | 18.15 / 10.08 | 17.03 / 14.79 |
| `text_primary` on `hover` | 15.52 / 9.06 | 14.05 / 13.19 |
| `text_secondary` | 7.57 / 4.51 | 7.11 / 6.62 |
| `text_secondary` on `hover` / `row_selected` | 7.03 / 4.27, 7.03 / 4.09 | — |
| `text_secondary_on_surface` on `row_selected` | 7.03 / 4.83 | — |

The composite was checked on screen against this arithmetic: the light sidebar over
black renders 196, 198, 201 (predicted 196). Changing a tint or a text color means
recomputing this table. Light has no lighter gray that keeps 4.5:1 on glass, so its
`text_tertiary` is `text_secondary`; dark's falls under 4.5:1 on a row's surface, where
such text turns `text_secondary`. Under a row's surface `text_secondary` falls short, so
a row whose surface is drawn because of state (the selection, or the sidebar's hover,
which redraws the row) puts its supporting text in `text_secondary_on_surface`, the
faintest gray that keeps 4.5:1 on either surface over the worst desktop (4.50 with the
wash under the text; 4.83 as the session list draws it, over the text, as the table
does).

## Layout and type

Sizes sit on a 4 px grid, in `crates/theme/src/metrics.rs`, but for hairlines and gaps
(`HAIRLINE` 1, `ROW_GAP` 1, `TAB_GAP` 2), the card's `MAIN_INSET` (6), the controls'
`CONTROL_RADIUS` (6), the sidebar's icons (`ICON_SIZE_ROW` 14) and the session row's
insets and sizes:

- The window has one top row, `HEADER_HEIGHT` (44), across the whole window on the glass:
  the macOS traffic lights, the sidebar toggle, Back and Forward, then the tabs over the
  card. Everything in it centers on it.
- The sidebar is `SIDEBAR_WIDTH` (264), under the top row, and meets the card. Its edge
  drags it between `SIDEBAR_MIN_WIDTH` (216) and `SIDEBAR_MAX_WIDTH` (400) on an
  `SIDEBAR_RESIZE_TARGET` (8) strip centered on the edge, under the resize cursor; a
  double-click there puts it back to `SIDEBAR_WIDTH`. The width is saved in
  `workspace.json` when the drag ends (`crates/studio/src/shell/sidebar_edge.rs`). `Cmd-B`
  (`Ctrl-B` elsewhere) or its toggle hides it: the toggle stays in place, Back and Forward
  ride the sidebar's edge, `SPACE_2` inside it in line with the sidebar's own trailing
  controls, and come to rest after the toggle.
- An icon button is `CONTROL_SIZE` (28) square with `CONTROL_RADIUS` (6), its icon
  `ICON_SIZE` (16) in `text_secondary`, turning `text_primary` on `hover`. In the
  sidebar's rows, its header buttons and a menu's items the icon is `ICON_SIZE_ROW` (14).
- Text is the platform's UI font (GPUI's `.SystemUIFont`; SF Pro on macOS) at `TEXT_BODY`
  (13), the size of macOS controls. Emphasis is weight, not size, but a session's title
  is `TEXT_SESSION_TITLE` (14). Two sizes are smaller: `TEXT_SMALL` (12), for a session
  row's supporting lines, whose contrast does not depend on size; and `TEXT_CAPTION`
  (11), for a code block's and a tool row's labels. Headings in the main area are
  `TEXT_HEADING` (18). What is read at length, the conversation's messages and replies,
  is a step larger: `TEXT_READING` (14), its headings `TEXT_READING_HEADINGS` (20, 17,
  15) and its code `TEXT_CODE` (13); session.md has the rest.
- A field or a message with a hairline pads its content `HAIRLINE` (1) less, so the
  content stands in the same columns as the unbordered rows around it. A menu does too,
  so its items stand `SPACE_1` in from its outer edge.

## Icons

- **The sidebar toggle** draws its glyph (`crates/studio/src/sidebar_glyph.rs`) from
  quads, so its inner panel narrows with the sidebar on the sidebar's own spring. GPUI's
  SVGs are static and could not do that.
- **Every other icon** is an outline SVG on a 24 grid at a 1.5 stroke, copied into
  `crates/studio/assets/icons/` and embedded by `crates/studio/src/assets.rs`. Add one only when a screen needs it, as an `Icon` variant, from the
  same set; take one from elsewhere only when the set has nothing close, at the same
  stroke. The search field's magnifier keeps its ring to 15 of the 24 units: one that
  fills the box looks oversized beside 13 pt text.
  GPUI paints an SVG in its own `text_color` only. The branch is the pull
  request icon's set's branch without its arrow, which is lost when small.
- **A toggle** shows the set's outline icon while off and the same outline with its main
  shape filled while on, in the icon button's colors.

## Controls and lists

- **Icon-only buttons** carry a tooltip naming the action ("Add project"), then its
  shortcut in a small key cap ("New session ⌘T").
  A button that can't act yet stays visible at 40% opacity, its tooltip saying why.
- **Tooltips** sit centered above their control, or below it in the top row, `SPACE_1`
  clear of it, never at the pointer; their look and timing are in
  [key-caps.md](key-caps.md).
- **Rows** are `ROW_HEIGHT` (28), spanning the list with `SPACE_2` sides and a rounded
  surface inside: `row_selected` under the selection, `hover` under the pointer or the
  keyboard highlight. In the sidebar rows stand `ROW_GAP` (1) apart, so a selected row
  and a hovered neighbor stay two shapes, and each row's fill fades on its own (see
  Motion). Text truncates with an ellipsis; secondary labels truncate first.
- GPUI shapes text before hover is known, so a `.hover()` style never changes a text
  color: it changes a fill or an icon. The sidebar's lists track the pointer in their own
  state and redraw, so a row there may change its text color under the pointer.
- **Tabs**, opening and closing them, their shortcuts and Back and Forward follow
  [tabs.md](tabs.md).
- **The menu bar** (macOS, `crates/studio/src/app_menu.rs`) lists every command beside
  its shortcut: Nerda Studio (Settings… `Cmd-,`, Services, Hide `Cmd-H`, Hide Others
  `Cmd-Alt-H`, Show All, Quit), File (New Session, Add Project…, the Close Tab commands), Edit (Cut, Copy,
  Paste, Select All, enabled in a text field), View (Toggle Sidebar, Search Sessions
  `Cmd-K`, Back, Forward, Toggle Full Screen `Ctrl-Cmd-F`) and Window (Minimize `Cmd-M`,
  Zoom, the next and previous tab). A new command goes into its menu as it gets a
  shortcut.
- **Quit.** `Cmd-Q` (`Ctrl-Q` elsewhere) in the window quits on a second press within 2 s
  (`crates/studio/src/shell/quit.rs`): the first shows "Press ⌘Q again to quit" (its
  large `Kbd` cap) at the top center of the window, 24 px under the top row, in
  `TEXT_HEADING_2` (16) medium, so it reads at a glance. Its surface is a menu's
  (`popover_background`, `popover_border`, `popover_shadow`) with `DIALOG_RADIUS`, 42
  tall with its hairline and 16 px sides.
  It takes no clicks or focus and doesn't animate. Held down, the key's repeat is the
  second press. The menu's Quit, and `Cmd-Q` with no window open, quit at once.
- **Closing the window** leaves the app running, as Mac apps do; a click on its Dock icon
  opens the window again on the same projects. Elsewhere closing it quits.
- **Menus** are `popover_background` (opaque) under a `popover_border` hairline,
  brighter than `border` to keep its edge over the composer, three tight shadows
  (`popover_shadow`), `MENU_RADIUS` (8), at least `MENU_WIDTH` (220) wide (256 with a
  search field, 176 for a submenu), growing to fit their widest line up to 320, and to
  360 tall, a search field included, before
  their list scrolls; a menu that opens near the window's edge is no taller than the
  room there, `SPACE_2` clear of the edge (`crates/studio/src/menu/`). Item icons are `ICON_SIZE_ROW` in `text_tertiary`;
  separators span the menu. An item may also carry a check (before its label, or after it
  where a list marks the current one), a shortcut in a small key cap and a short detail in
  secondary text. Arrows move the highlight, as the pointer does; Enter or a
  click picks. An item with a submenu names its current choice and a chevron; the
  pointer resting on it, a click, Enter or the right arrow opens the submenu beside it,
  and the left arrow goes back. A window-wide backdrop under the menu takes the click
  that closes it, so it presses nothing else, the second press of a double-click
  excepted; Escape closes it too, and so does focus going elsewhere. Supporting text
  under the highlight: `text_secondary_on_surface`.
- **Text fields** have no box of their own: the sidebar's search is the magnifier and
  its text in the title's place, and a menu's or a palette's field is a row over a
  `border` hairline; the composer is its own box (composer.md). Focus shows only as
  the cursor. `focus_ring`, text primary at 48% (dark) and 53% (light), marks the
  composer's box while files are dragged over it and the chosen card in the backdrop
  panel, keeping 4.82:1 (dark) and 3.65:1 (light) against those surfaces. The cursor and selection span the font's ascent to descent (15 pt at
  `TEXT_BODY`), not the 21 pt line. The cursor blinks, 530 ms on and 530 off, while its
  field has focus in the active window; each move or edit shows it solid and starts the
  blink over, and in a window in the background it hides, as the system's fields do.
  Every field is one `TextInput` with this rhythm.
- **A close button** inside a tab, a chip or a message is the small x alone, without a
  plate: `ICON_SIZE_SMALL` (12) at a heavier stroke so it still reads, `text_tertiary`
  at rest and `text_primary` under the pointer, taking clicks over `FIELD_TARGET_SIZE`
  (`close_button` in `crates/studio/src/sidebar/header.rs`).
- **Key caps**, the shortcut beside a label, in a tooltip, a menu item or a hint, follow
  [key-caps.md](key-caps.md).
- **The sidebar's** header, session cards, messages, empty states and footer follow
  [sidebar.md](sidebar.md).
- **The composer**, where a session's message is written, follows [composer.md](composer.md).
- **Pages** are empty for now: a page on screen shows its title in medium-weight
  `text_primary` and one `text_secondary` line on what it will hold, centered in the
  main area, `SPACE_1` apart.
- **Palettes** are dialogs of one field over a list:
  `PALETTE_WIDTH` (576) centered a tenth of the way down the window and never above the
  top row, `popover_background` with a hairline, a large shadow and `DIALOG_RADIUS` (12),
  over a window-wide `scrim` that takes the click closing it (the second press of a
  double-click excepted, so a double-clicked button leaves it open). The list keeps
  `PALETTE_LIST_HEIGHT` (288) whatever it holds; a footer names the keys. Arrows move the
  keyboard highlight, which starts on nothing; Escape closes and gives focus back. They
  don't animate, like menus.
- **Add project** is a palette that browses folders as the path is typed, starting at
  `~/` (`crates/studio/src/add_project/`). The text up to the last separator names the
  folder listed, the rest filters its subfolders by prefix, ignoring case. Hidden ones show
  once the filter starts with a dot; `./` and `../` start from the current project. Enter
  opens the highlighted folder (".." its parent) once it has listed; with nothing
  highlighted, or with Cmd-Enter, it adds the typed path, as the button in the field does.
  The button reads "Create & Add" when the folder doesn't exist yet, and adding creates
  it. The footer offers the system's folder picker.

## Motion

Motion has to earn its place: it shows where something went. Springs from
`crates/theme/src/motion.rs` drive it, described by response and damping ratio, so a
toggle pressed again mid-flight turns back from where it is, keeping its speed. GPUI's
`with_spring` holds that state per element id.

- **Every way in animates the same.** A click and its shortcut (`Cmd-B`) run the same
  motion, so an action always looks the same. The motion never holds input: typing goes
  on while it plays.
- **No overshoot** where nothing was thrown: damping ratio 1. Exits are quicker than
  entrances: `PANEL_ENTER` responds in 0.2 s, `PANEL_EXIT` in 0.17 s.
- **Reduce motion** in the system settings snaps every animation to its end.
  `crates/studio/src/accessibility.rs` reads the setting on macOS and Windows when the
  window opens, comes to front or changes appearance, on each toggle and each change to
  the tabs, and hands it to GPUI, which does not read it itself; Linux has no one
  setting. The sidebar rows' fills read it as they start.
- **The sidebar** animates its width and fades with it. Its contents stay pinned to the
  window's left edge while the sidebar's edge sweeps over them, so nothing passes under the
  traffic lights or the toggle, which never move. On the same spring the card's left inset
  grows and Back and Forward ride the sidebar's edge. The tabs keep the width they have
  beside the open sidebar, the narrower of their two, so their titles truncate once, not
  on every frame, and no tab or New session passes the window's edge.
- **The sidebar's rows** fade their own fills over `ROW_FADE` (100 ms); nothing slides
  from row to row ([sidebar.md](sidebar.md)).
- **The composer** of a new session glides down to the card's bottom as its first message
  goes, on `COMPOSER_DOCK` (0.4 s, slower than a panel since it travels most of the card;
  [composer.md](composer.md)).
- **Tabs**: an opened tab's slot grows from nothing on `TAB_ENTER` (0.25 s) and a closed
  one collapses in place on `TAB_EXIT` (0.2 s) before it leaves, so its neighbors give way
  gradually; the tab inside keeps its settled width and the slot's edge cuts it. Reopened
  mid-collapse, it grows back from where it is. A dropped tab, and the tabs it passed,
  glide into their slots on `TAB_ENTER`. Switching projects replaces the row at once: it
  is not a close.

`TAB_ENTER` carries a tab to 90% in about 155 ms and settles in about 370 ms; a closed
tab's slot is 99% shut in about 210 ms and leaves the row after about 290 ms
(`collapse_time`, the time `TAB_EXIT` takes to settle).

## Window chrome

- macOS: the titlebar is transparent and the traffic lights sit over the glass, centered on
  the top row. `crates/studio/src/traffic_lights.rs` places them and keeps the row's content
  clear of them, from their size measured on macOS 27: they center at 22 of the 44 pt row.
- macOS: the app owns the titlebar (`app_owns_titlebar_drag`), so AppKit acts on no click in
  the top row. The shell's `titlebar` strip spans the row: dragging it moves the window and
  double-clicking it does what System Settings says (zoom or minimize). A control in the
  row (the toggle, Back and Forward, each tab, New session) draws after the strip and
  calls `.occlude()`, so its clicks, double-clicks included, stay its own and never move
  or zoom the window.
- Windows: the system titlebar stays until the app draws its own caption buttons.
- The window opens at 1200 × 800, centered, with a minimum of 720 × 480. After that it
  opens where it was when the app last quit, on the same display, with the sidebar open
  or closed as it was left (`crates/studio/src/instance/`).
