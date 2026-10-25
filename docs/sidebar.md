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
  beside it with their choices (Last activity or Created; All projects, No project once
  a session has been started in it with a hairline after it, then the folder projects),
  each naming its current one, then Archived. The sort and the filter are saved; the
  archive shows until it is switched off or the app quits. A filter to one project keeps
  the menu button on `hover`.
- **Search**: the Search button or `Cmd-K` (`Ctrl-K` elsewhere) turns the sessions'
  title into the search field and puts the cursor in it, its text selected, opening the
  sidebar first if it is closed and closing any open menu; `Cmd-K` does nothing while a
  palette is open over the window. The field has no box: the magnifier in
  `text_tertiary`, the text in the title's size and weight, the placeholder in
  `text_tertiary`. Search becomes Close search beside it. Typing filters the list live;
  Up and Down move the row the hover marks, wrapping, as the pointer does, and Enter
  opens it. The field stays while focus goes elsewhere; Escape or Close search clears
  it, puts it away and gives focus back to where it was taken from, else to the window,
  as Enter does after opening the result. Closing the sidebar gives focus back too:
  typing never goes to a field out of sight.
- **Sessions** are one list, newest first under the sort. Projects are not a level of
  the list: the filter narrows it to one, and a session shown from elsewhere (New Session
  in Project, or adding a folder already added) widens it to all if it would hide that
  session. Each session is a row `SESSION_ROW_HEIGHT`
  (78) tall with `SESSION_ROW_RADIUS` (8) corners, `SESSION_ROW_SPACING` (2) clear above
  and below it, so with `ROW_GAP` two rows' surfaces stand 5 apart. Its content stands
  `SESSION_ROW_INSET` (10) from its sides and `SPACE_2` from its top and bottom, on
  three lines. First, `SESSION_LINE_HEIGHT` (20) tall in `TEXT_SMALL`, `SPACE_1` × 1.5
  apart: the project's icon, its name as the sidebar shows it in medium weight, and how
  long ago the session was active ("now", "5m", "3h", "2d", "4w", "1y") at the trailing
  edge, in a column at least 32 wide. Then, `SPACE_1` lower and as tall, the title in
  `TEXT_SESSION_TITLE` (14), medium weight. Last, `SPACE_1` / 2 lower and 16 tall in
  `TEXT_SMALL`: the branch the project's checkout is on, cut in the middle where it is
  too long, and at the trailing edge the logo of the agent the session runs, at
  `ICON_SIZE_ROW` in the agent's color at 60%; an agent with no logo in this build shows
  none, and a project outside git, or No project, no branch. The title is
  `text_primary`; the project and the age `text_secondary`, the branch `text_tertiary`;
  on the wash of the selected row or the one under the pointer or the search highlight
  both turn `text_secondary_on_surface`.
- **A project's icon** stands in a `PROJECT_ICON` (16) slot: the favicon or logo the
  project keeps (`favicon.svg`, `.png` or `.ico` at its top, in `public/` or `static/`;
  `icon` or `favicon.ico` in `app/` or `src/app/`; `icon` or `logo` at its top or in
  `assets/`; `logo` in `public/`; else the first of its `apps/` that has one; never built
  output, nor a file over 512 KB), filling the slot with `PROJECT_ICON_RADIUS` (4)
  corners. It is made ready off the UI thread with the
  branches, and drawn again only when the file changes: a vector at the slot's pixel
  size, so a 16 px favicon stays sharp, a bitmap scaled down once, either centered so a
  mark that isn't square keeps its shape. A wordmark more than twice as wide as tall (or
  tall as wide) is refused. A dark mark on transparency stands on a plate of
  `text_primary` at 90% on the dark glass, inset 2, and a light one likewise on the
  light glass; a mark with colors or a ground of its own stands as it is. A project
  without an icon, or whose icon doesn't read, shows two initials of its own name on a
  tile with the same corners, in its badge's initials color, the tile that color at 14%:
  in the monospaced font, bold, `PROJECT_INITIALS_SIZE` (8.25), each letter
  `PROJECT_INITIAL_WIDTH` (6) wide, so two span 12 of the 16. Words are runs of letters
  and digits; the first initial is the first word's first letter, the second the first
  digit after it, else the last word's first letter, else the first word's last
  ("arcada.next" AN, "web" WB, "app2" A2, "oʻzbek loyiha" OL). The hue comes from its
  folder and so stays across launches; a name with no letter shows a folder in that
  color. No project shows a computer in the line's color.
- **A session's actions.** Under the pointer the row's button fades in over the age as
  the age fades out, over `ROW_FADE` as the row's fill does, and back as the pointer
  leaves: only its icon, `ICON_SIZE_ROW` in `text_secondary`, turning `text_primary`
  under the pointer, which turns to a hand, in a `SESSION_LINE_HEIGHT` square on the
  project's line, the icon's trailing edge on the content's. The age keeps its column,
  so the project's name truncates the same whatever shows there. Archive, or Restore in
  the archive. It acts on one click; a double click acts once, so its second click never
  lands on the row that moved up under it. A right click on a row opens its menu at the
  pointer, the row keeping its wash meanwhile: New Session in Project, Show in Finder
  for a folder project, Close Tab while the session's tab is on screen, then past a
  hairline Archive, or Restore.
- **The archive.** Archiving takes a session out of the list and closes its tab as
  closing it would; it keeps its title, its conversation and its agent. Archived in the
  sessions' menu, checked while on, shows the archive in the list's place, titled
  "Archived": the archived sessions under the same sort, filter and search, or "No
  archived sessions". Restore puts a session back where its activity places it, as a
  message sent in it does.
- **Branches and ages** are read off the UI thread: branches, which the composer shows,
  from folder projects' checkouts when projects come and go, when the window comes to
  front and with the ages; at launch the window waits up to 50 ms for them, so its
  first frame already shows them. Ages redraw when the first of them next reads
  differently, and at least once a minute and when the window comes to front, since the
  timer stops while the Mac sleeps.
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
  without a project yet" under a filter, "No sessions match …") is one `text_secondary`
  line under the header.
- **Titles** come from a session's first message, its first line that isn't blank, cut
  to 200 characters, unless the session has a title of its own. They show as one line:
  line breaks, tabs and runs of spaces read as one space,
  and a blank title reads "Untitled session", in the rows and the tabs alike. Search
  matches them ignoring case and runs of spaces, е for ё, and a plain apostrophe for ʻ, ʼ,
  the curly ones and the backtick.
- **Names truncate** at the end: a session's title, a project's name in a menu.
- **Before the first project** the list stays blank under the sessions' title; the main area
  says what a project is and offers "Add project", centered, unless a page is on screen.
- **The sidebar footer** is one row, its bottom on the card's bottom edge: Settings as a
  row like the pages above the list, taking the width, then Usage and Pull requests as
  icon buttons with tooltips at its trailing edge, all `ROW_GAP` apart, the buttons'
  trailing edge in line with the sessions' title row. The buttons are pages beside the
  listed ones: a click shows the page's tab, opening it if it isn't open, and the page on
  screen keeps the selected wash. Settings opens over the whole window, its menu in the
  sidebar's place ([settings.md](settings.md)), as `Cmd-,` (`Ctrl-,` elsewhere) and the
  app menu's Settings… do.
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
