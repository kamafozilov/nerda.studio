# The composer

Where a message to a session's agent is written, in the main area's card whenever a
session's tab is on screen (`crates/studio/src/composer/`). The materials, colors and
controls it uses are in [design.md](design.md). Nothing is sent yet: the composer is its
look and its editing, ahead of the agents.

- **Placement.** One column, at most `COMPOSER_MAX_WIDTH` (720) wide, centered in the
  card and `SPACE_3` clear of its sides when the card is narrower. A draft session, one
  with no message yet, centers the column in the card under a question naming its
  project, "What should we work on in web?", in semibold `TEXT_HEADING` (18)
  `text_primary`, 24 above the box; a long name truncates and the question mark stays.
  A session that has seen work docks the column to the card's bottom, `SPACE_3` above
  its edge, leaving the space above for its messages. Pages and the welcome are as they
  were.
- **The box** reads as a text field: `field_background`, a `border` hairline that turns
  `focus_ring` while the text has focus, and `COMPOSER_RADIUS` (12) corners, rounder than
  a field's since it holds a toolbar too. Its content stands `SPACE_3` in, less the
  hairline. No shadow and no material of its own. A click anywhere in it puts the cursor
  in the text.
- **The text** shows "Ask anything, or describe a change" in `text_secondary` while
  empty. It starts `COMPOSER_MIN_LINES` (2) lines tall, so a draft's composer and a
  docked one stand the same height, and grows a line at a time as it wraps or breaks, to
  `COMPOSER_MAX_LINES` (10): docked, it grows upward and the toolbar stays put. Past ten
  lines it scrolls inside, keeping the cursor in view; the wheel scrolls it too, until
  the cursor next moves. Height changes are instant.
- **Keys.** Enter sends and Shift-Enter breaks the line. Up and Down move a row, the
  line as wrapped, aiming at the column the first of them started from; Up on the first
  row goes to the start, Down on the last to the end. Home, End and Cmd-Left and
  Cmd-Right keep to the row, and End stops before the space where a row wraps, so the
  cursor never shows on the next row. Cmd-Up and Cmd-Down go to the start and the end of
  the text (Ctrl-Home and Ctrl-End elsewhere). Pasted text keeps its line breaks, each
  `\r\n` and `\r` read as one. The sidebar's search and the Add project palette hold one
  line, as before: a break pasted there is a space.
- **The toolbar**, `CONTROL_SIZE` high, `SPACE_2` under the text: Attach, then three
  pills, the agent ("Claude Code"), the effort ("High") and the access ("Ask first"),
  then Send at the trailing edge.
  - Attach can't act yet: 40%, its tooltip "Attachments come with agents".
  - A pill is `CONTROL_SIZE` high with `CONTROL_RADIUS`, its value in `TEXT_BODY`
    `text_secondary` and a chevron at `ICON_SIZE_SMALL`, unfilled at rest and on `hover`
    under the pointer, as an icon button is. It shows a current value, so it isn't
    faded; its tooltip says "Choosing comes with agents".
  - Send is a `CONTROL_SIZE` circle of `text_primary` with an up arrow in
    `panel_background`: at 40% with "Type a message to send" while the text is empty or
    only whitespace, full with "Send (↩)" otherwise.
- **The context row**, `SPACE_2` under the box and in line with its text: the project's
  folder, `~` for home and cut at its start, then the branch its checkout is on, or the
  commit, cut in the middle, each after its `ICON_SIZE_SMALL` icon, in `TEXT_SMALL`
  `text_secondary`. Outside git there is no branch. It is the branch the sidebar's cards
  show, read as [sidebar.md](sidebar.md) says.
- **One composer per open session.** A message half written stays with its session
  while another tab is shown, and goes when its tab closes; it isn't saved. Showing a
  session's tab, at launch too, puts the cursor in its composer, unless a palette, a
  menu, the quit dialog or the search holds focus. Focus given back to a composer whose
  session is off screen, as when a palette closes, moves on to the one on screen, and so
  does focus left nowhere by a closed tab. A page on screen gives focus to the window.

## Contrast

WCAG 2 contrast on the composer's field over the card, the lower over a white and a
black desktop, glass (dark / light):

| Against the field | Contrast |
| --- | --- |
| Placeholder, `text_secondary` | 6.24 / 5.91 |
| Text, `text_primary` | 11.95 / 13.19 |
| `focus_ring` hairline | 3.71 / 3.14 |
| Send's arrow, against its fill | 6.84 / 9.26 |

Sampled on screen over the desktop behind the window at the time, the placeholder read
6.58 / 6.50 and the text 12.62 / 14.55, between the arithmetic's worst and best.
