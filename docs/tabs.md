# Tabs

The top row's tabs (`crates/studio/src/tabs/`; their model in
`crates/workspace/src/tabs.rs`), and how they open, close and move. Their motion is in
[design.md](design.md)'s Motion.

- **Tabs** in the top row are the pages open (Notes, Design, Routines, and the sidebar
  footer's Usage, Pull requests and Settings), and the sessions open in the project on
  screen (`crates/studio/src/tabs/`). Each project keeps its own session tabs, saved with
  the workspace; showing another project's session switches the row to that project's
  tabs, and the pages stay in it. A page on screen keeps its project's tabs beside it. A
  tab is `CONTROL_SIZE` high on `tab_background`, its title in `text_secondary`; the
  selected one is `tab_selected` under a `border` hairline, its title in `text_primary`,
  and another turns `hover` under the pointer. Every tab keeps the hairline's room, so a
  title holds still as its tab is selected. The first tab stands at the card's left
  edge: on the sidebar's edge while it is open, `MAIN_INSET` clear of Back and Forward
  once it closes, moving on the sidebar's spring. A page's tab leads with its sidebar icon
  in `text_secondary`, `SPACE_2` before the title. Tabs share the row up to
  `TAB_MAX_WIDTH` (224) each, `TAB_GAP` apart, however many there are; the selected one
  keeps `TAB_SELECTED_MIN_WIDTH` (112). Under the pointer a tab shows its close button if
  it is at least `TAB_CLOSE_MIN_WIDTH` (72) wide or selected, unless it is a project's
  unused draft alone in the row, which has none and whose Close Tab is dimmed: closing it
  would only open it again. New session follows the last tab, once there is a project.
- **Opening and closing tabs.** A session opened from the sidebar, a shortcut or New
  session gets a tab at the end unless it has one; so does a page, which has one tab at
  most, however often it is opened. A used session opened while the project's unused
  draft is selected takes the draft's tab, in its place; from another project's draft it
  opens at the end and the draft's tab goes. Closing the selected tab selects its right neighbor,
  else its left; closing the last tab on screen opens the project's draft, so a project on
  screen always shows something, and before the first project leaves the welcome. A tab
  closes from its button, a middle click, `Cmd-W`, or its menu (a right click): Close
  Tab, Close Other Tabs, Close Tabs to the Right or to the Left, Close All Tabs. Dragged
  along the row, a tab reorders; while dragged it lies on `popover_background` with
  `shadow_md`, its fill over that, so the tabs it passes never show through it.
- **Tab shortcuts** (Ctrl for Cmd elsewhere): `Cmd-T` or `Cmd-N` new session, `Cmd-W`
  close, `Cmd-Alt-T` close the others, `Cmd-Shift-W` close all, `Ctrl-Tab`,
  `Cmd-Shift-]` or `Cmd-}` the next tab and `Ctrl-Shift-Tab`, `Cmd-Shift-[` or `Cmd-{` the
  previous, wrapping, `Cmd-1` to `Cmd-8` by place and `Cmd-9` the last, `Cmd-,` the
  Settings page.
- **Back and Forward** (`Cmd-[`, `Cmd-]`) retrace the tabs shown, as a browser's
  history does, the last 50 of them, kept in memory only; a closed tab drops out of
  them, and a new visit clears Forward.
