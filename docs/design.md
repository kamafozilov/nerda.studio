# Design

Nerda Studio looks like a native glass app: the desktop shows through the window, blurred,
and the work sits on a main area raised over it. The `theme` crate holds every value below;
components read them through `Theme::of(cx)` and the constants in `theme`, never as literals.

## Materials

| Layer | What it is | Token |
| --- | --- | --- |
| Window base | The blurred desktop under a tint. The top row and the sidebar sit on it directly. | `window_background` |
| Main area | A card raised over the base: `MAIN_RADIUS` (10) corners and a hairline, `MAIN_INSET` (6) clear of the window's right and bottom edges, and of its left while the sidebar is closed. | `panel_background`, `border` |

- **Glass** on macOS and Windows: the window uses `WindowBackgroundAppearance::Blurred`
  and the base tint is translucent. Measured on macOS 27, GPUI's blur
  (`NSVisualEffectView`, `UnderWindowBackground`, behind the window) adds no tint: with a
  zero tint, a white desktop shows through as 255. Windows' blur adds none either. The
  tint alone keeps text readable.
- **Solid** on Linux, where no blur is guaranteed, and wherever the user turned on
  "Reduce transparency" (macOS) or turned off "Transparency effects" (Windows): the base
  is opaque, the same colors otherwise. `crates/studio/src/system_theme.rs` rereads the
  setting when the window comes back to front, so a change made in System Settings applies
  without a restart.
- **The glass is in the top row and the sidebar.** The main area stays near solid, about
  6% of the desktop showing through, because its content (code, chat, a design canvas)
  must read the same on any desktop and keep its own colors.
- Do not stack another translucent surface on the main area. GPUI blurs only what is behind
  the window, not the app's own content, so a menu or popover needs an opaque background of
  its own.
- The window follows the system's light or dark appearance and switches live.

## Colors

Colors are roles: a token names its job, and a component uses it only in that job. A role
that is missing gets a new token, computed and measured as below, never a borrowed one.

Every value comes from OKLCH with one cool neutral hue (h 265, C 0.006; text C 0.009), in
`crates/theme/src/palette.rs` as `0xRRGGBBAA`.

| Role | Dark | Light | Use |
| --- | --- | --- | --- |
| `window_background` (glass) | `0e0f12` at 78% (`c7`) | `f3f5f9` at 81% (`ce`) | Window base, sidebar |
| `window_background` (solid) | `0e0f12` | `f3f5f9` | Window base without blur |
| `panel_background` | `151619` at 72% | `fbfdff` at 72% | Main area over the base |
| `border` | text primary at 10% | text primary at 10% | Sidebar edge, dividers |
| `hover` | text primary at 6% | text primary at 6% | Hovered control |
| `text_primary` | `eff2f8` (L 0.96) | `191b1f` (L 0.22) | Titles, body, hovered icons |
| `text_secondary` | `aeb1b7` (L 0.76) | `505358` (L 0.44) | Supporting text, icons at rest |
| `text_secondary_on_surface` | `c7cad0` (L 0.837) | `45474c` (L 0.399) | Supporting text on a row's `hover` or `row_selected` surface |
| `scrim` | `151619` at 64% | `fbfdff` at 60% | Dims the window under a dialog; nothing is read on it |
| `key_background` | text primary at 10% | text primary at 10% | Key cap fill; `text_secondary` on it keeps 4.92 / 5.51 even under `hover` |
| `badges` | fill L 0.38 C 0.075, initials L 0.87 C 0.10 | fill L 0.91 C 0.045, initials L 0.43 C 0.12 | Project badges, eight hues (25, 60, 95, 145, 185, 235, 285, 330), chroma cut to stay in sRGB; the initials keep at least 6.73:1 (dark) and 5.98:1 (light) on their fill, 5.27 and 5.39 under a row's surface |

### Contrast on glass

Each tint is the most transparent 8-bit alpha that keeps `text_secondary` at WCAG AA,
4.5:1, over the worst desktop for its appearance: white behind dark, black behind light.
A clearer glass would fail it there. WCAG 2 contrast on the composited surface, the lower
over a white and a black desktop (dark / light):

| Text | Sidebar | Main area |
| --- | --- | --- |
| `text_primary` | 8.72 / 10.08 | 14.06 / 14.79 |
| `text_primary` on `hover` | 7.40 / 9.04 | 11.91 / 13.16 |
| `text_secondary` | 4.55 / 4.51 | 7.34 / 6.62 |
| `text_secondary` on `hover` / `row_selected` | 4.04 / 4.27, 3.71 / 4.09 | — |
| `text_secondary_on_surface` on `row_selected` | 4.70 / 4.83 | — |

The composite was checked on screen against this arithmetic: over a white backdrop the
dark sidebar renders 67, 68, 70 (predicted 67), the light one over black 196, 198, 201
(predicted 196). Changing a tint or a text color means recomputing this table. There is
no tertiary text color: one light enough to look tertiary falls near 3:1 on the sidebar.
Under a row's surface `text_secondary` falls short, so a row whose surface is drawn
because of state (the selection, or the sidebar's hover, which redraws the row) puts its
supporting text in `text_secondary_on_surface`, the faintest gray that keeps 4.5:1 on
either surface over the worst desktop (4.51 / 4.50 with the wash under the text; 4.70 /
4.83 as the session list draws it, over the text).

## Layout and type

Sizes sit on a 4 px grid, in `crates/theme/src/metrics.rs`, but for the card's
`MAIN_INSET` (6) and the tabs' `TAB_GAP` (2):

- The window has one top row, `HEADER_HEIGHT` (44), across the whole window on the glass:
  the macOS traffic lights, the sidebar toggle, Back and Forward, then the tabs over the
  card. Everything in it centers on it.
- The sidebar is `SIDEBAR_WIDTH` (260), under the top row, and meets the card. `Cmd-B`
  (`Ctrl-B` elsewhere) or its toggle hides it: the toggle stays in place, Back and Forward
  ride the sidebar's edge, `SPACE_2` inside it in line with the sidebar's own trailing
  controls, and come to rest after the toggle.
- An icon button is `CONTROL_SIZE` (28) square with `CONTROL_RADIUS` (6), its icon
  `ICON_SIZE` (16) in `text_secondary`, turning `text_primary` on `hover`.
- Text is the platform's UI font (GPUI's `.SystemUIFont`; SF Pro on macOS) at `TEXT_BODY`
  (13), the size of macOS controls. Emphasis is weight, not size. Two sizes are smaller:
  `TEXT_SMALL` (12), for a session card's supporting lines, in `text_secondary`, whose
  contrast does not depend on size; and `BADGE_TEXT` (9), for a project badge's two
  initials, which keep at least 5.27:1.
- A field or a message with a hairline pads its content `HAIRLINE` (1) less, so the
  content stands in the same columns as the unbordered rows around it: the search field's
  magnifier and text over the pages' icons and titles. A menu does too, so its items
  stand `SPACE_1` in from its outer edge and their corners nest in its own.

## Icons

- **The sidebar toggle** draws its glyph (`crates/studio/src/sidebar_glyph.rs`) from
  quads, so its inner panel narrows with the sidebar on the sidebar's own spring. GPUI's
  SVGs are static and could not do that.
- **Every other icon** is an outline SVG on a 24 grid at a 1.5 stroke, copied into
  `crates/studio/assets/icons/` and embedded by `crates/studio/src/assets.rs`; licenses
  Add one only when a screen needs it, as an `Icon` variant, from the
  same set; take one from elsewhere only when the set has nothing close, at the same
  stroke. The search field's magnifier keeps its ring to 15 of the 24 units: one that
  fills the box looks oversized beside 13 pt text.
  GPUI paints an SVG in its own `text_color` only, never its parent's. The branch on a
  session card is the pull request icon's set's branch without its arrow, which is lost
  at `ICON_SIZE_SMALL`.
- **A toggle** shows the set's outline icon while off and the same outline with its main
  shape filled while on, in the icon button's colors.

## Controls and lists

- **Icon-only buttons** carry a tooltip naming the action ("Add project", "New session (⌘N)").
  A button that can't act yet stays visible at 40% opacity, its tooltip saying why.
- **Tooltips** sit centered above their control, or below it in the top row, where there
  is no room above; `SPACE_1` clear of it, never at the pointer, so they don't cover the
  controls beside it (`crates/studio/src/tooltip.rs`; GPUI's `.tooltip()` follows the
  pointer and isn't used). One is `TOOLTIP_HEIGHT` (24) high, hairline included. It shows
  after the pointer rests `TOOLTIP_DELAY` (500 ms); within `TOOLTIP_WARM` (300 ms) of
  another, at once. A press hides it.
- **Rows** are `ROW_HEIGHT` (28), spanning the list with `SPACE_2` sides and a rounded
  surface inside: `row_selected` under the selection, `hover` under the pointer or the
  keyboard highlight. In the sidebar, where a selected row and a hovered one can be
  neighbors, each row's slot is `ROW_GAP` (2) taller and centers the surface, so
  neighboring surfaces stand 2 apart. A sidebar list draws one selection surface and one
  hover surface, which glide from row to row (see Motion), rather than a fill per row;
  the slot, gap included, tracks the pointer, so no gap is dead. In the session list the
  surfaces are drawn over the rows, in their scrolled space: both washes are text
  primary at a low alpha, so text primary under them reads as it was. Text truncates
  with an ellipsis; secondary labels truncate first.
- GPUI shapes text before hover is known, so a `.hover()` style never changes a text
  color: it changes a fill or an icon. The sidebar's lists track the pointer in their own
  state and redraw, so a row there may change its text color under the pointer.
- **Tabs** in the top row are the pages open, Notes, Design and Routines, and the
  sessions open in the project on screen (`crates/studio/src/tabs/`). Each project keeps
  its own session tabs, saved with the workspace; showing another project's session
  switches the row to that project's tabs, and the pages stay in it. A page on screen
  keeps its project's tabs beside it. A tab is `CONTROL_SIZE` high, its title in
  `text_primary` on `row_selected` when selected and on `hover` under the pointer. Tabs
  share the row up to `TAB_MAX_WIDTH` (224) each, `TAB_GAP` apart, however many there
  are; the selected one keeps `TAB_SELECTED_MIN_WIDTH` (112). Under the pointer a tab
  shows its close button if it is at least `TAB_CLOSE_MIN_WIDTH` (72) wide or selected.
  New session follows the last tab.
- **Opening and closing tabs.** A session opened from the sidebar, a shortcut or New
  session gets a tab at the end unless it has one; so does a page, which has one tab at
  most, however often it is opened. Closing the selected tab selects its right neighbor,
  else its left; closing the last tab on screen opens the project's draft, so a project on
  screen always shows something, and before the first project leaves the welcome. A tab
  closes from its button, a middle click, `Cmd-W`, or its menu (a right click): Close
  Tab, Close Other Tabs, Close Tabs to the Right or to the Left, Close All Tabs. Dragged
  along the row, a tab reorders.
- **Tab shortcuts** (Ctrl for Cmd elsewhere): `Cmd-T` or `Cmd-N` new session, `Cmd-W`
  close, `Cmd-Alt-T` close the others, `Cmd-Shift-W` close all, `Ctrl-Tab` or
  `Cmd-Shift-]` the next tab and `Ctrl-Shift-Tab` or `Cmd-Shift-[` the previous, wrapping,
  `Cmd-1` to `Cmd-8` by place and `Cmd-9` the last.
- **Back and Forward** (`Cmd-[`, `Cmd-]`) retrace the tabs shown, as a browser's
  history does; a closed tab drops out of them, and a new visit clears Forward.
- **The menu bar** (macOS, `crates/studio/src/app_menu.rs`) lists every command beside
  its shortcut: Nerda Studio (Services, Hide `Cmd-H`, Hide Others `Cmd-Alt-H`, Show All,
  Quit), File (New Session, Add Project…, the Close Tab commands), Edit (Cut, Copy,
  Paste, Select All, enabled in a text field), View (Toggle Sidebar, Search Sessions
  `Cmd-K`, Back, Forward, Toggle Full Screen `Ctrl-Cmd-F`) and Window (Minimize `Cmd-M`,
  Zoom, the next and previous tab). A new command goes into its menu as it gets a
  shortcut.
- **Quit.** `Cmd-Q` (`Ctrl-Q` elsewhere) in the window quits on a second press within 2 s:
  the first shows "Press ⌘ Q again to quit", centered, in `popover_background` with key
  caps, taking no clicks. Held down, the key's repeat is the second press. The menu's
  Quit, and `Cmd-Q` with no window open, quit at once.
- **Closing the window** leaves the app running, as Mac apps do; a click on its Dock icon
  opens the window again on the same projects. Elsewhere closing it quits.
- **Menus** are `popover_background` (opaque) with a hairline, a large shadow,
  `MENU_RADIUS` (10: the items' `CONTROL_RADIUS` plus the `SPACE_1` between them and the
  menu's outer edge, so the corners nest) and `MENU_WIDTH` (220), growing to 360 before
  they scroll. A window-wide backdrop under the menu takes the click that closes it, so
  that click presses nothing else; Escape closes it too.
- **Text fields** look like a place to type before they're focused: `field_background`
  fill, a `border` hairline, `CONTROL_RADIUS`; focus turns the hairline `focus_ring`. That
  is text primary at 48% (dark) and 53% (light), the faintest that keeps 3:1 against the
  field and the sidebar over a white and a black desktop: measured 3.03 / 3.02. The
  cursor and selection span the font's ascent to descent (15 pt at `TEXT_BODY`), not the
  21 pt line. A button inside a field, such as Clear, shows `ICON_SIZE` but takes clicks
  over `FIELD_TARGET_SIZE` (24).
- **Key caps** show a shortcut beside a button's label or in a hint: `KEY_CAP_SIZE` (20)
  high and at least as wide, `key_background` fill with no hairline, `SPACE_1` corners,
  the key in `text_secondary`. Return is an icon, other keys text (⌘, `esc`). Inside a
  button `FIELD_TARGET_SIZE` (24) high, the cap sits 2 px from its top, bottom and right.
- **The sidebar's** header, session cards, messages, empty states and footer follow
  [sidebar.md](sidebar.md).
- **Pages** are empty for now: a page on screen leaves the main area blank.
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
  entrances: `PANEL_ENTER` responds in 0.3 s, `PANEL_EXIT` in 0.25 s.
- **Reduce motion** in the system settings snaps every animation to its end.
  `crates/studio/src/accessibility.rs` reads the setting on macOS and Windows when the
  window opens, comes to front or changes appearance, on each toggle and each change to
  the tabs, and hands it to GPUI, which does not read it itself; Linux has no one
  setting. The cards' glide reads it as it starts.
- **The sidebar** animates its width and fades with it. Its contents stay pinned to the
  window's left edge while the sidebar's edge sweeps over them, so nothing passes under the
  traffic lights or the toggle, which never move. On the same spring the card's left inset
  grows and Back and Forward ride the sidebar's edge. The tabs keep the width they have
  beside the open sidebar, the narrower of their two, so their titles truncate once, not
  on every frame, and no tab or New session passes the window's edge.
- **The sidebar's lists** glide their selection and hover surfaces from row to row, and
  their cards to new places when the list changes; [sidebar.md](sidebar.md) has the rules
  and the springs.
- **Tabs**: an opened tab's slot grows from nothing on `TAB_ENTER` (0.25 s) and a closed
  one collapses in place on `TAB_EXIT` (0.2 s) before it leaves, so its neighbors give way
  gradually; the tab inside keeps its settled width and the slot's edge cuts it. Reopened
  mid-collapse, it grows back from where it is. A dropped tab, and the tabs it passed,
  glide into their slots on `TAB_ENTER`. Switching projects replaces the row at once: it
  is not a close.

Measured on macOS at 120 Hz: the sidebar closes to 90% in 159 ms and settles in 374 ms;
it opens to 90% in 204 ms. Reversed mid-flight, it carries on for two frames and turns
back without a jump. A tab opens to 90% in about 160 ms and settles in 350 ms; a closed
one is gone in 200 ms (debug build).

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
