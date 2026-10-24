# The sidebar

The sidebar under the top row, on the glass: the header, the session list and the
footer (`crates/studio/src/sidebar/`). The materials, colors, rows and motion it uses
are in [design.md](design.md).

- **The sidebar header** is two parts. First New session and the pages, Notes, Design
  and Routines, as rows of `ROW_HEIGHT` `ROW_GAP` apart: an `ICON_SIZE_ROW` icon and a
  medium-weight title, both `text_secondary` at rest and `text_primary` on the wash of
  the row on screen or under the pointer. New session is on screen while a draft is,
  and shows its shortcut (⌘N) as a key cap under the pointer; it starts the session in
  the project the list is filtered to, else where the shortcut would. Before the first
  project it stays at 40% with a tooltip saying why. Then, `SPACE_3` lower, the title
  "Sessions" in medium-weight `TEXT_SMALL` `text_tertiary`, with Search and the
  sessions' menu at its trailing edge.
- **The sessions' menu** (⋯) holds Add project, then Sort and Project, which open
  beside it with their choices (Last activity or Created; All projects, No project
  once a session has run in it, then a hairline and the folder projects), each naming
  its current one. The sort and the filter are saved. A filter to one project
  keeps the menu button on `hover`.
- **Search**: the Search button or `Cmd-K` (`Ctrl-K` elsewhere) turns the sessions'
  title into the search field and puts the cursor in it, its text selected, opening the
  sidebar first if it is closed and closing any open menu; `Cmd-K` does nothing while a
  palette is open over the window. The field has no box: the magnifier in
  `text_tertiary`, the text in the title's size and weight, the placeholder in
  `text_tertiary`. Search becomes Close search beside it. The field stays while focus
  goes elsewhere; Escape or Close search clears it, puts it away and gives focus back
  to where it was taken from, else to the window, as Enter does after opening the
  result the hover marks. Closing the sidebar gives focus back too: typing never goes
  to a field out of sight.
- **Sessions** are one list, newest first under the sort. Projects are not a level of the
  list: the filter narrows it to one. Each session is a row: the project's badge, the
  title, and how long ago the session was active ("now", "5m", "3h", "2d", "4w", "1y") in
  `TEXT_CAPTION` `text_tertiary`. The title is `text_secondary`, and `text_primary` on
  the wash of the selected row or the one under the pointer or the search highlight,
  where the age turns `text_secondary`. The badge is `BADGE_SIZE` (16) with `SPACE_1`
  corners, the project's initials (the first letters of its first two words:
  arcada.next is AN, oʻquv is OQ) in semibold `BADGE_TEXT` (9), in the project's hue,
  which comes from its folder and so stays across launches. No project's badge is a
  computer at `ICON_SIZE_SMALL` in `text_secondary` on `key_background`.
- **Branches and ages** are read off the UI thread: branches, which the composer shows,
  from folder projects' checkouts when projects come and go, when the window comes to front
  and with the ages; at launch the window waits up to 50 ms for them, so its first frame
  already shows them. Ages redraw when the first of them next reads differently,
  and at least once a minute and when the window comes to front, since the timer stops
  while the Mac sleeps.
- **Problems** show as a message under the sidebar header until dismissed: what failed and
  what to do next, without parser or error-code detail the reader can't act on ("Nerda
  Studio isn’t allowed to write to ~/Library/…", not the system's error), folders with
  `~` for home. Its text stands in the sessions' title column, scrolls past about six
  lines so the list and the footer keep their place, and its Dismiss takes clicks over
  `FIELD_TARGET_SIZE`. A problem inside a dialog shows in the dialog, under its field,
  until the next edit.
- **Only sessions that have seen work** are listed. A new session is a draft in its tab
  until its first message, and stays out of the list, search and the projects' order;
  New session in its project opens the same draft again.
- **An empty list** ("No sessions yet", "No sessions in web yet" or "No sessions
  without a project yet" under a filter, "No sessions match …") is one `text_secondary` line under the header.
- **Titles** come from a session's first message, its first line that isn't blank, cut
  to 200 characters, unless the session has a title of its own. They show as one line:
  line breaks, tabs and runs of spaces read as one space,
  and a blank title reads "Untitled session", in the rows and the tabs alike. Search
  matches them ignoring case, е for ё, and a plain apostrophe for ʻ, ʼ and the curly
  ones.
- **Names truncate** at the end: a session's title, a project's name in a menu.
- **Before the first project** the list stays blank under the sessions' title; the main area
  says what a project is and offers "Add project", centered, unless a page is on screen.
- **The sidebar footer** is one row, its bottom on the card's bottom edge: Settings as a
  row like the pages above the list, taking the width, then Usage and Pull requests as
  icon buttons with tooltips at its trailing edge, all `ROW_GAP` apart, the buttons'
  trailing edge in line with the sessions' title row. They are pages beside the listed ones:
  a click shows the page's tab, opening it if it isn't open, and the page on screen keeps
  the selected wash. `Cmd-,` (`Ctrl-,` elsewhere) and the app menu's Settings… show Settings.
- **The list's bottom edge** has no hairline. While more sessions lie below, the rows
  fade out over `SCROLL_EDGE_FADE` (32) into the footer, which takes on the sidebar's
  material (`window_background`) under its row; both thin out over the last 32 px of
  scrolling, so nothing appears or goes at once. A list that fits shows neither.

## Motion

- **Each row fades its own fill.** A row's wash fades in over `ROW_FADE` (100 ms) as it
  is chosen or the pointer enters it, and out as the pointer leaves, so moving down the
  list one row fades out as the next fades in; nothing slides from row to row. Its text
  turns at once. A row drawn for the first time, a list a search or a filter replaced,
  and Reduce motion show the fill at once.
- A session chosen elsewhere, a tab or a shortcut, scrolls into view.
