# A session on screen

What a session's tab shows once it has seen work: its conversation over the docked
composer, and the agent behind both. The composer itself is in [composer.md](composer.md),
the agent's process and transcript in [agent.md](agent.md). The materials and colors are
in [design.md](design.md).

## The agent

`crates/studio/src/agents/` keeps each session's agent for as long as the app runs,
whether or not its tab is open.

- **Starting.** A session's first message starts its agent, in the project folder, on
  the session's model, effort and access. The app picks the conversation's id and saves
  it with the session before the agent says anything. A message after the agent quit,
  as after a restart of the app, starts it again: resuming the conversation when it has
  a turn on record, anew under the same id when it hasn't. `agent::prewarm()` reads the
  login shell's PATH at launch, so the first start doesn't wait for it.
- **Sending.** Send, or Enter, sends the text with its trailing whitespace trimmed, and
  the attachments: an image by its bytes, anything else by its path. The composer clears
  both. The session gets its first activity, so it joins the sidebar's list, and a
  session still called "New session" takes its first line as its title. A message sent
  while a turn runs waits in the agent's queue.
- **A start that fails** sends nothing: the text and the attachments stay, the session
  stays a draft, and the problem line under the box says why, such as "Couldn't find
  claude. Install it with `curl -fsSL https://claude.ai/install.sh | bash` in a
  terminal, then try again." A command in backticks shows in the monospaced font.
- **Stop.** While a turn runs or a message waits, and the text is empty, Send is Stop:
  the same circle with a filled square, drawn from a quad since the icon set's stop is
  an outline. It interrupts the turn and drops what waits. With text, Send stays Send
  and queues the message. Escape does nothing new.
- **Choices mid-session.** A model, effort or access picked while the agent runs goes to
  it at once. Switching to or from Full access while a turn runs is refused: the problem
  line gives the reason, and the pill goes back to the access the agent runs in.
- **Tabs.** Closing a session's tab leaves its agent running; opening the session again
  shows the conversation as it went on. Each open tab keeps its own conversation view,
  so where it was scrolled survives switching tabs.
- **Quitting** ends every agent and waits up to 6 s for each to exit, its last events
  written: an idle agent quits at once (0.2 s measured, 0.8 s for claude), one mid-turn
  in about 2.3 s, when the crate's SIGTERM ends it.
- **A stand-in.** `NERDA_AGENT_PROGRAM=<path>`, read at launch, runs that program in
  place of the agent's own command, such as the agent crate's `fake-agent`, which plays
  `fake-agent.jsonl` from the project folder.

## The journal

Each session's events go to `sessions/<conversation id>.jsonl` in the data folder, one
JSON line each, written in order on a thread of their own. The file is named by the
conversation, which no other workspace shares: a workspace started afresh, after its
file was set aside, hands out session ids again.

- Opening a session reads its journal off the UI thread and replays it into the
  transcript. A message sent before it has read says so in the problem line and stays.
- No journal is an empty conversation. A conversation saved with the session but
  without a journal is resumed on the next message, since one most likely ran.
- A line that doesn't read, as one a crash cut short or a newer build wrote, is left
  out, and a notice under the sidebar's header says how many: "Part of “Plan the
  sidebar” couldn't be read: 2 events of its conversation are left out." The file is
  only ever appended to; a last line cut short gets a line break first, so the next
  line reads.
- A journal that ends without the agent's exit, as after a crash, ends what it left
  open as the agent quitting, in memory only.
- A journal that can't be written says so in a notice, once; the conversation goes on.
- While the workspace isn't saved (its file is from a newer build, or there is no data
  folder), no journal is written either.

## The conversation

`crates/studio/src/conversation/` draws the transcript in the main area over the docked
composer, in the composer's column: at most `COMPOSER_MAX_WIDTH`, centered, `SPACE_3`
clear of the card's sides. A draft shows its composer alone, as before.

- **Rows.** Each turn is its message, then what came back, item by item, then how it
  ended. A row whose look changed is measured again where it is; from where rows came or
  went, the list lays out anew, so streaming text grows only its own row.
- **Sticking to the bottom.** Newest is at the bottom, and the list keeps to it while
  output streams. Scrolled up, it stays where it is, however much streams below, until
  scrolled back to the bottom, or until "Scroll to the end" is pressed: a `CONTROL_SIZE`
  circle centered over the composer, `SPACE_2` above the list's bottom, on
  `popover_background` under a `popover_border` hairline with `popover_shadow`, its
  down arrow `ICON_SIZE` in `text_secondary`, `text_primary` under the pointer. It shows
  only while the list is scrolled up from its end. While more lies below, the rows fade
  into the card over their last `SCROLL_EDGE_FADE`, thinning out as the end comes. The
  list keeps `SCROLL_EDGE_FADE` and `SPACE_2` (40) of room under its last row, inside
  the scroll: at the end the last row stands that far clear of the composer and out of
  the fade, and rows scrolled up still pass under the fade into the composer.
- **The person's message** sits at the column's trailing edge, at most
  `MESSAGE_MAX_WIDTH` (80%) of it, on `field_background` with `COMPOSER_RADIUS` corners,
  `SPACE_3` by `SPACE_2` in, its text in `TEXT_BODY` `text_primary` with its line breaks.
  Its attachments are the composer's chips without the remove button, over the text: a
  file or folder by its name, the whole path in the tooltip; an image, sent by its
  bytes, by its format ("PNG image"). One that waits for the turn before shows "Queued"
  under it, in `TEXT_SMALL` `text_secondary`.
- **The agent's text** fills the column in `text_primary`, as Markdown (CommonMark with
  GitHub's tables, task lists and strikethrough, by `pulldown-cmark`): paragraphs
  `SPACE_2` apart; headings in semibold at `TEXT_HEADING` (18), `TEXT_HEADING_2` (16),
  `TEXT_HEADING_3` (14), then `TEXT_BODY`, `SPACE_3` more clear of what comes before;
  bold, italic and struck text. Uzbek's ʻ and ʼ show as ‘ and ’, which look the same,
  so a line wraps between words, not inside oʻ or gʻ.
  - **Inline code** is a chip: `MONOSPACE_FONT` on `field_background` under a `border`
    hairline, 4 pt corners, two narrow spaces of room each side that don't break, so a
    short chip wraps to the next line whole; a long one wraps where its text does.
  - **A file or folder the text names** is a chip too: inline code that is a path (a
    folder in it and an extension, a trailing `/`, or a line, as `src/app.rs:12` or
    `#L12`), or a link to one. It shows a file or folder icon in `mention`, the name in
    medium weight, its folder after a `·` when another path of the same name is named in
    the text, and its place (`L12`, `L12:C3`) in `text_secondary`, on `mention_background`
    under a `mention_border` hairline. A path reads inside the project's folder unless
    absolute or from `~/`. A click opens it in its app; the tooltip gives its path from
    the home folder and its size, or "Not found".
  A line break stays one, as in the terminal the agent writes for. Text renders as it
  streams; a code fence not closed yet runs to the end, a table shows the rows that came.
  - **Links**, and web addresses written out bare, are in `link`; a click opens a web or
    mail address in the browser. A link to a path is a file chip, above.
  - **Lists** keep their markers in a 20 pt column in `text_secondary`, widened a digit
    at a time past 99; bullets go disc, ring, square by depth. A task list's item has a
    `ICON_SIZE_ROW` box with a `text_secondary` hairline instead, checked when done.
  - **Quotes** stand behind a two-hairline `border` rule.
  - **Code blocks** sit on `field_background` with `MENU_RADIUS` corners: a header with
    the fence's language in `MONOSPACE_FONT` `TEXT_CAPTION` `text_secondary`, and Copy, a
    `FIELD_TARGET_SIZE` button whose icon turns a check, its tooltip "Copied", for
    1.2 s; then the code at `TEXT_SMALL`, scrolling sideways when a line is longer than
    the column, while a vertical wheel over it still scrolls the conversation. A
    language a grammar knows (`syntect` with `two-face`'s, by name or extension, and
    names such as `shell` or `jsonc`) colors its tokens in `syntax`'s keyword, string,
    constant, function and type colors, comments in italic `text_secondary`, a diff's
    lines in `diff_added` and `danger`; any other shows plain. A block that streams is
    parsed a line at a time: each finished line once, only the line still coming again.
    The grammars load on a thread of their own at launch.
  - **Tables** sit in a frame: a `border` hairline with `MENU_RADIUS` corners, the head
    on `field_background` in semibold, a hairline between rows, no lines between
    columns, cells `SPACE_3` by `SPACE_2` in, at `TEXT_BODY`. Each column is as wide as
    its widest cell (code and chips measured in their own fonts), up to 320, and shares
    out the room left over; past that cells wrap, down to 120 or their longest word, a
    head never. A table that can't fit scrolls sideways inside its frame. Columns keep
    GitHub's alignment.

- **Selecting the agent's text.** A press on a reply's text starts a selection there,
  Shift-press extends it, dragging draws it out across the reply's paragraphs, cells
  and code blocks; a double press takes a word, a triple the whole paragraph or block.
  The selection is `text_selection` under the text, inside one reply; a press anywhere
  else, or Escape, lets it go. The press gives the conversation key focus, so ⌘C copies
  the selection and ⌘A selects the whole reply. Copied, paragraphs stand a line apart and
  a table's cells a tab; a chip copies what it stands for, its code or the path as
  written, and Uzbek's ‘ is its ʻ again.
- **The text's menu**, on a right press: over a link, Open Link and Copy Link; over a
  file, Open, Show in Finder and Copy Path; then Copy (⌘C, only with a selection),
  Select All (⌘A), and Copy Reply, the reply's Markdown. A right press outside the
  selection lets it go.
- **Under a turn that ended**, a Copy button leads its "Worked for" or "Stopped" line
  when the agent wrote something in it: it copies all the turn's text as Markdown, a
  blank line apart, and shows a check, its tooltip "Copied", for 1.2 s.
- **Text as it streams** fades in where it ends: each part that arrives goes from clear
  to full color over 320 ms, quick at first, in place, so nothing moves.
- **Thinking** is a row led by a brain icon: "Thinking" with its clock while it streams,
  and its latest line in `text_tertiary` after it; then "Thought for 4s" ("Thought" when
  the app didn't see it stream). A click opens it to the summary, in `text_secondary`, in
  line with the label; a chevron shows under the pointer.
- **A tool call** is one line: its icon (`ICON_SIZE_ROW`, `text_secondary`), a verb in
  the past tense once done and the present while it runs ("Read", "Reading"), what it
  acted on in `text_primary` (a file by its name, the whole path in its tooltip; a
  command or pattern in `MONOSPACE_FONT` at `TEXT_CAPTION`), then quieter detail in
  `text_tertiary` (the file's folder inside the project, "lines 40–119", "3 files"). A
  change counts its lines, "+2 −1" in `diff_added` and `danger`. A failed call turns
  its icon into an alert in `danger` and says "failed". Opened by a click, a command
  shows itself over its output, an edit its diff (removed and added lines on their
  washes, lines kept between them plain), anything else its output: 16 lines at most,
  40 for a diff, then how many more. The plan (TodoWrite) shows its steps always: done
  ones struck through, the one in hand in medium weight. A prompt that waits says "The
  agent is waiting for an answer to run Bash." (or that it asked a question, or
  proposed a plan), then "allowed", "denied" or "not answered". Stop denies it.
- **Groups.** Two or more tool calls and thinking blocks in a row, one of them a call,
  gather under one line: a chevron where their icons stand and what they did, the kinds
  that change things first, three at most and the rest counted ("Edited motion.rs, ran
  2 commands, read 2 files and 3 more"), the lines changed and how many failed. The
  group is open while the agent adds to it and closes once it moves on; a click opens or
  closes it for good. Its rows hang from a `border` rail down from the chevron.
- **Motion while it works.** A running call's icon breathes and its verb glows: a band
  of `text_primary` sweeps through `text_secondary` every 2.2 s; a live group's line
  glows too. A row that comes while the agent works fades in and rises 4 pt on a
  critically damped spring of 0.3 s response. The conversation redraws 30 times a
  second while anything moves, once a second for the clocks under Reduce motion, and not
  at all while idle. Reduce motion holds every glow, wave and fade still.
- **How a turn ended**, in `TEXT_SMALL` under it: "Worked for 1m 12s" (whole seconds,
  at least one; minutes and hours past them) in `text_secondary`, a `border` hairline
  running on to the column's edge; "Stopped"; or a failure as the problem line says it,
  `text_primary`: what went wrong, then what to do, such as "The agent isn't signed
  in." over "Run `claude auth login` in a terminal, then try again."
- **Working.** While a turn runs or a message waits, the conversation's last row,
  `SPACE_3` under the newest, so it stays where the eye is, shows a square of nine dots
  in the rows' icon slot, three rows in `activity`'s blue, amber and
  pink, a wave rising through them every 1.2 s; what the agent is doing, glowing
  ("Thinking", "Writing", "Reading motion.rs", "Running cargo test", "Waiting for your
  answer", "Starting", "Stopping"); and for how long it has worked, in `TEXT_SMALL`
  `text_tertiary`. The row goes once the turn ends.

The conversation's own colors are in `crates/theme/src/palette.rs`, computed in OKLCH:
`danger` (h 27; 6.99:1 dark, 5.68:1 light on the card over the worst desktop),
`diff_added` (h 150; 9.84 and 5.29), their washes at 12% (dark) or 8% (light), and the
`activity` dots, on which nothing is read.

Every text here keeps a contrast measured before: `text_primary` and `text_secondary` on
the card (design.md), `text_primary` on `field_background` (composer.md).

## Not yet

Answering prompts, questions and plans, a subagent's own steps, failure cards with more
than words, the sessions' status in the sidebar, opening links, selecting
text and image thumbnails come later. The rows are the seam: a kind of row draws one item
of the transcript, so a richer one takes its place without touching the rest.

GPUI wraps lines at characters it doesn't know as part of a word, and the Uzbek ʻ
(U+02BB) is one of them: a long line can break between "o" and "ʻ".
