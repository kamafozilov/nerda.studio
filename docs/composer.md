# The composer

Where a message to a session's agent is written, in the main area's card whenever a
session's tab is on screen (`crates/studio/src/composer/`). The materials, colors and
controls it uses are in [design.md](design.md). Nothing is sent yet: the composer is its
look, its editing and its choices, ahead of the agents.

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
  pills, the model ("Opus 5.5"), the effort ("High") and the access ("Ask first"), then
  Send at the trailing edge.
  - Attach can't act yet: 40%, its tooltip "Attachments come with agents".
  - A pill is `CONTROL_SIZE` high with `CONTROL_RADIUS`, its value in `TEXT_BODY`
    `text_secondary` and a chevron at `ICON_SIZE_SMALL`, unfilled at rest and on `hover`
    under the pointer and while its menu is open, as the sidebar's menu button is. Its
    tooltip names it: "Model", "Effort", "Access". A long value truncates.
  - Send is a `CONTROL_SIZE` circle of `text_primary` with an up arrow in
    `panel_background`: at 40% with "Type a message to send" while the text is empty or
    only whitespace, full with "Send (↩)" otherwise.
- **Pickers.** A pill's click opens its menu ([design.md](design.md) has the menu
  rules) upward from the pill's leading edge, draft or docked: the composer sits low in
  the card, and a menu that never covers its pill can't take the next click on the pill
  as a pick. The model menu lists each agent's name ("Claude Code") over its models,
  Fable 5.1, Opus 5.5, Sonnet 5.5, Haiku 5.5; the effort menu Low, Medium, High, Extra
  high, Max; the access menu Ask first, Accept edits, Plan, Auto and Full access, each
  over a `TEXT_SMALL` line on what it allows. A check marks the current value. Focus goes
  back to the text when a picker closes.
- **Choices are saved** with the session at once, and the last one made becomes what a
  new session, and a draft opened again, start from. A file from before choices starts
  on Opus 5.5, High, Ask first. A model or agent this build doesn't know (a newer
  build's, or a retired model) shows as written, checked under its agent, and stays in
  the file until another is picked; an effort or access it doesn't know reads as the
  default. The agents and their options are a table in `crates/workspace/src/agents.rs`,
  which turns a choice into the agent's arguments: `--model`, `--effort` and
  `--permission-mode` (Ask first is `manual`, Full access `bypassPermissions`).
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
