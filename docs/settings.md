# Settings

Settings (`crates/studio/src/settings/`) is not a tab: it takes the whole window. What it
sets is saved in `workspace.json` under `preferences`
(`crates/workspace/src/preferences.rs`).

## Opening and leaving

- `Cmd-,` (`Ctrl-,` elsewhere), the sidebar footer's Settings row and the menu bar's
  Settings… open it (`crates/studio/src/shell/settings_mode.rs`). Its menu takes the
  sidebar's place and its section the card; the top row keeps the traffic lights and the
  sidebar toggle alone, the tabs and Back and Forward out of it. The toggle and `Cmd-B`
  fold the menu as they fold the sidebar.
- Back in its menu ("‹ Settings"), Escape, `Cmd-,` again and `Cmd-W` leave it, and so does
  anything that shows a tab: a new session, `Cmd-1` to `Cmd-9`, the next or previous
  tab, Back and Forward, the sessions' search. The tabs come back as they were;
  Settings closes none of them. Before the first project it leaves to the welcome.
- On opening, Settings takes focus, so Escape and Tab reach it at once; on leaving,
  focus goes back to the composer on screen, if any.
- The section chosen stays while the app runs; the app starts on General.

## The menu

In the sidebar's place, its padding and rows the sidebar's (`nav.rs`):

- "Settings" with the Back icon, a row that leaves.
- The search: the magnifier and the field, without a box, as the sidebar's search is,
  "Search settings" its placeholder, `SPACE_2` over the list.
- The sections in groups: General, Appearance, Keyboard, Notifications; **Agents**:
  Agents and models; **Code**: Projects, Git; **App**: Advanced. A group's heading is a
  `ROW_HEIGHT` row in `TEXT_SMALL` medium `text_tertiary`, `SPACE_2` under the group
  before it; the first has none. A section's row is the sidebar's: its icon and title in
  medium `text_secondary`, the chosen one on `row_selected` and the one under the pointer
  on `hover`, both in `text_primary`, each fill fading over `ROW_FADE`. The list scrolls
  when the window is too short for it.
- **While the search holds text** the settings it finds take the sections' place, under
  their sections' titles: a setting is found by its label or its line, ignoring case;
  a section without settings yet is found by its title. A click shows that section and
  ends the search. With nothing found, "No settings match" in `text_secondary`. Escape
  clears the search; a second Escape leaves.

## The section

In the card, a column at most `SETTINGS_CONTENT_MAX_WIDTH` (768) wide, centered, 24 in
on either side and `SETTINGS_TOP` (64) down from the card's top, scrolling on its own:

- The section's title in `TEXT_TITLE` (22) medium `text_primary`, then its parts,
  `SETTINGS_PART_GAP` (40) apart. A section with nothing to set yet shows one
  `text_secondary` line on what it will hold.
- **A part** is a heading in `TEXT_SUBTITLE` (14) medium `text_primary`, `SPACE_3` over
  its group (`rows.rs`).
- **A group** is its rows on `field_background` under `MENU_RADIUS` corners and a `border`
  hairline, the same hairline between the rows. A row is 16 in on every side, its label
  in medium `text_primary` over its line in `text_secondary`, both `TEXT_BODY` and
  wrapping, 24 clear of its control at the end, which keeps its size.

## Controls

- **A switch** is a `SWITCH_WIDTH` × `SWITCH_HEIGHT` (28 × 16) pill with a `SWITCH_KNOB`
  (12) knob `SWITCH_INSET` (2) clear of its outer edge (`crates/studio/src/switch.rs`).
  There is no accent color, so on is the `switch_on` track (`text_primary`) with the
  `switch_on_knob` (the card's color, opaque) at its end; off is the `switch_off` track
  (text primary at 16% in light, 18% in dark) under a `popover_border` hairline with the
  white `switch_off_knob` at its start. The knob slides on `PANEL_ENTER`; flipped again
  mid-way it turns back from where it is, and Reduce motion puts it there at once.
- **A picker** is `SETTINGS_PICKER_WIDTH` (128) wide whatever it names, so the pickers'
  edges line up: `CONTROL_SIZE` high, `CONTROL_RADIUS`, `panel_background` under a
  `popover_border` hairline, the choice in `TEXT_SMALL` medium and a chevron at its end.
  Its menu hangs from its trailing edge, the choice checked before its label; Escape
  closes the menu alone.
- **Keyboard.** Tab and Shift-Tab move through the sections, then the controls, from the
  first section; a row or a control reached that way is ringed in `focus_ring`, a
  hairline outside it, which a click doesn't show. Space or Enter picks the section or
  works the control.

## General

| Part | Setting | Choices | Default |
| --- | --- | --- | --- |
| Startup | On launch | Last tabs: the tabs open at the last quit, as they were. New session: every tab, other projects' too, closes, and the draft of the project last on screen opens alone; without projects, the welcome. | Last tabs |
| Quitting | Ask before quitting | On: `Cmd-Q` in the window quits on a second press, as design.md's Quit says. Off: the first press quits. The menu's Quit always quits at once. | On |

On launch acts once, when the app starts and reads its workspace; a window opened again
from the Dock keeps its tabs.

## Saved

`preferences` holds `on_launch` (`"last-tabs"` or `"new-session"`) and `confirm_quit`.
A file without it, from before Settings, starts on the defaults. A value this build can't
read, such as a choice a newer build added or a wrong type, reads as its default alone:
the rest of the file loads, and the next save writes the default back. A key this build
doesn't know is dropped on the next save. A Settings tab an earlier build saved is
skipped, as a page this build doesn't know.
