# Design

Nerda Studio looks like a native glass app: the desktop shows through the window, blurred,
and the work sits on a main area raised over it. The `theme` crate holds every value below;
components read them through `Theme::of(cx)` and the constants in `theme`, never as literals.

## Materials

| Layer | What it is | Token |
| --- | --- | --- |
| Window base | The blurred desktop under a tint. The sidebar sits on it directly. | `window_background` |
| Main area | Raised over the base, flush with the window's edges, a hairline from the sidebar. | `panel_background`, `border` |

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
- **The glass is in the sidebar.** The main area stays near solid, about 6% of the desktop
  showing through, because its content (code, chat, a design canvas) must read the same
  on any desktop and keep its own colors.
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
| `scrim` | `151619` at 64% | `fbfdff` at 60% | Dims the window under a dialog; nothing is read on it |
| `key_background` | text primary at 10% | text primary at 10% | Key cap fill; `text_secondary` on it keeps 4.92 / 5.51 even under `hover` |

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

The composite was checked on screen against this arithmetic: over a white backdrop the
dark sidebar renders 67, 68, 70 (predicted 67), the light one over black 196, 198, 201
(predicted 196). Changing a tint or a text color means recomputing this table. There is
no tertiary text color: one light enough to look tertiary falls near 3:1 on the sidebar.

## Layout and type

Sizes sit on a 4 px grid, in `crates/theme/src/metrics.rs`:

- The window has one top row, `HEADER_HEIGHT` (44), across the sidebar and the main area.
  The macOS traffic lights and everything else in it center on it.
- The sidebar is `SIDEBAR_WIDTH` (260); `Cmd-B` (`Ctrl-B` elsewhere) or its toggle in the
  top row hides it, and the toggle then stays in place over the main area.
- An icon button is `CONTROL_SIZE` (28) square with `CONTROL_RADIUS` (6), its icon
  `ICON_SIZE` (16) in `text_secondary`, turning `text_primary` on `hover`.
- Text is the platform's UI font (GPUI's `.SystemUIFont`; SF Pro on macOS) at
  `TEXT_BODY` (13), the size of macOS controls. The app name is medium weight; emphasis is
  weight, not size.

## Icons

- **The sidebar toggle** draws its glyph (`crates/studio/src/sidebar_glyph.rs`) from
  quads, so its inner panel narrows with the sidebar on the sidebar's own spring. GPUI's
  SVGs are static and could not do that.
  outline style), copied into `crates/studio/assets/icons/` and embedded by
  Add one only when
  a screen needs it, as an `Icon` variant.
  icons, so pull requests and the Return key cap use
  GPUI paints an SVG in its own `text_color` only, never its parent's.

## Controls and lists

- **Icon-only buttons** carry a tooltip naming the action ("Add project", "New session (⌘N)").
  A button that can't act yet stays visible at 40% opacity, its tooltip saying why.
- **Tooltips** sit centered above their control, `SPACE_1` clear of it, never at the
  pointer, so they don't cover the controls beside it (`crates/studio/src/tooltip.rs`;
  GPUI's `.tooltip()` follows the pointer and isn't used). One shows after the pointer rests
  `TOOLTIP_DELAY` (500 ms); within `TOOLTIP_WARM` (300 ms) of another, at once. A press
  hides it.
- **Rows** are `ROW_HEIGHT` (28), spanning the list with `SPACE_2` sides and a rounded
  surface inside: `row_selected` under the selection, `hover` under the pointer or the
  keyboard highlight. Text truncates with an ellipsis; secondary labels truncate first.
- **Menus** are `popover_background` (opaque) with a hairline, a large shadow and
  `MENU_WIDTH` (220), growing to 360 before they scroll. A window-wide backdrop under the
  menu takes the click that closes it, so that click presses nothing else; Escape closes
  it too.
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
- **The sidebar header** is two rows: the search field across the sidebar, then the list's
  title ("Sessions") with filter and sort, Add project and New session at its trailing edge.
- **Problems** show as a message under the sidebar header until dismissed: what failed and
  what to do next, without parser or error-code detail the reader can't act on. A problem
  inside a dialog shows in the dialog, under its field, until the next edit.
- **Before the first project** the sidebar stays blank under its header; the main area
  says what a project is and offers "Add project", centered.
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
- **The sidebar footer** holds Usage, Pull requests and Settings at its trailing edge. They
  open nothing yet.

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
  `crates/studio/src/accessibility.rs` reads the setting on macOS and Windows on each
  toggle and hands it to GPUI, which does not read it itself; Linux has no one setting.
- **The sidebar** animates its width and fades with it. Its contents stay pinned to the
  window's left edge while the sidebar's edge sweeps over them, so nothing passes under the
  traffic lights or the toggle, which never move.

Measured on macOS at 120 Hz: the sidebar closes to 90% in 159 ms and settles in 374 ms;
it opens to 90% in 204 ms. Reversed mid-flight, it carries on for two frames and turns
back without a jump.

## Window chrome

- macOS: the titlebar is transparent and the traffic lights sit over the glass, centered on
  the top row. `crates/studio/src/traffic_lights.rs` places them and keeps the row's content
  clear of them, from their size measured on macOS 27: they center at 22 of the 44 pt row.
- macOS: the app owns the titlebar (`app_owns_titlebar_drag`), so AppKit acts on no click in
  the top row. The shell's `titlebar` strip spans the row: dragging it moves the window and
  double-clicking it does what System Settings says (zoom or minimize). A control in the
  row draws after the strip and calls `.occlude()`, so its clicks, double-clicks included,
  stay its own and never move or zoom the window.
- Windows: the system titlebar stays until the app draws its own caption buttons.
