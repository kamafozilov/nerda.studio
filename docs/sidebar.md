# The sidebar

The sidebar under the top row, on the glass: the header, the session list and the
footer (`crates/studio/src/sidebar/`). The materials, colors, rows and motion it uses
are in [design.md](design.md).

- **The sidebar header** is three parts: the search field across the sidebar, flush with
  the top; then the pages, Notes, Design and Routines, as rows with their icon, `SPACE_2`
  under it, the selection's surface under the one on screen; then the title "Sessions" in
  medium-weight `text_primary` at the leading edge, lined up with the sessions' titles,
  with filter and sort, Add project and New session at the trailing edge. The title and
  the filter show once there is a project. The sort and the filter are saved. A filter to
  one project keeps the filter button on `hover`, and New session then starts the session
  in that project, its tooltip naming it ("New session in web"); the shortcut starts it
  in the project on screen.
- **Search**: `Cmd-K` (`Ctrl-K` elsewhere) puts the cursor in the search field, its text
  selected, opening the sidebar first if it is closed and closing any open menu; it does
  nothing while a palette is open over the window. The field shows the shortcut as a key
  cap at its trailing edge while it is focused or under the pointer and empty, so at rest
  the placeholder has it to itself. Enter opens the result the hover surface marks;
  Enter, and Escape on an empty search, give focus back to where `Cmd-K` took it from,
  else to the window, and so does closing the sidebar: typing never goes to a field out
  of sight.
- **Sessions** are one list, newest first under the sort. Projects are not a level of the
  list: the filter narrows it to one. Each session is a card, `SESSION_ROW_HEIGHT` (72)
  in its slot, of three lines in `SPACE_2` padding (8 + 16 + 4 + 18 + 2 + 16 + 8,
  `CARD_LINE`, `CARD_TITLE_LINE` and `CARD_LINE_GAP`): the project's badge, its name and
  how long ago the session was active ("now", "5m", "3h", "2d", "4w", "1y") in
  `TEXT_SMALL` `text_secondary`; the title in medium-weight `text_primary`; then where it
  runs, the branch (or the commit) its project's checkout is on, else its folder, `~` for
  home. The badge is `BADGE_SIZE` (16) with `SPACE_1` corners, the project's initials
  (the first letters of its first two words: arcada.next is AN, oʻquv is OQ) in semibold
  `BADGE_TEXT` (9), in the project's hue, which comes from its folder and so stays across
  launches. The badge and the branch icon share the pages' icon column, and the project's
  name and the branch the pages' title column. On the hover or selection surface the
  supporting lines turn `text_secondary_on_surface`, which keeps their contrast there.
- **Branches and ages** are read off the UI thread: branches from the checkout's files (a
  reftable checkout shows its folder) when projects come and go, when the window comes to
  front and with the ages; at launch the window waits up to 50 ms for them, so its first
  frame already shows them. Ages redraw when the first of them next reads differently,
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
- **An empty list** ("No sessions yet", "No sessions in web yet" under a filter, "No
  sessions match …") is one `text_secondary` line in the cards' title column, `SPACE_2`
  under the header.
- **Titles** show as one line: line breaks, tabs and runs of spaces read as one space,
  and a blank title reads "Untitled session", in the cards and the tabs alike. Search
  matches them ignoring case, е for ё, and a plain apostrophe for ʻ, ʼ and the curly
  ones.
- **Names truncate** where the rest tells most: a title and a project's name at the end,
  a name that is a path's end ("north/web", for two projects named web) and a folder at
  the start, a branch in the middle.
- **Before the first project** the sidebar stays blank under its header; the main area
  says what a project is and offers "Add project", centered, unless a page is on screen.
- **The sidebar footer** holds Usage, Pull requests and Settings at its trailing edge, their
  bottom on the card's bottom edge. They open nothing yet.

## Motion

- **Row surfaces glide.** Each list, the pages and the sessions, draws one selection
  surface and one hover surface. The hover surface follows the pointer from row to row on
  `HOVER_ENTER` (0.06 s, about 80 ms to settle), fades in where it first lands rather
  than sliding in from where it last was (also after a long scroll, or from a row out of
  view), and fades out on `HOVER_EXIT` (0.045 s) when the pointer leaves, the window
  included (GPUI otherwise finds it again at its last place inside). While searching it
  marks the result Enter opens, which arrows and the pointer move. It stays off the
  selected row, which its own surface marks: the two washes together would take the row's
  text under 4.5:1.
- The selection surface glides to a newly chosen row on `SELECTION_ENTER` (0.12 s) and
  fades on `SELECTION_EXIT` (0.09 s), as when a page takes the screen. It lands instead
  of gliding when its list was replaced (Enter on a search result), when the old or the
  new row is out of view, and while the sidebar is closed. A surface whose row moved
  because the list changed, not the choice, takes its place at once, riding its card
  while the cards glide. A session chosen elsewhere, a tab or a shortcut, scrolls into
  view. A surface never slides from one list to the other.
- **Cards glide when the list changes under them**, as when a new session lands on top or
  the sort changes: each moves from where it was drawn to its new place on `ROW_SHIFT`
  (0.14 s), one from more than 12 places away starting just past the view, and a new card
  fades in once the card it takes the place of has mostly moved off. A change mid-glide
  carries every card on from where it is. A search or a project filter replaces the list
  at once, as do Reduce motion and a closed sidebar. A card whose new place is out of
  view leaves at once: the list draws only the rows in view.
