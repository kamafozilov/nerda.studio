# The composer

Where a message to a session's agent is written, in the main area's card whenever a
session's tab is on screen (`crates/studio/src/composer/`). The materials, colors and
controls it uses are in [design.md](design.md). What sending does, Stop, and the
conversation over a docked composer are in [session.md](session.md).

- **Placement.** One column, at most `COMPOSER_MAX_WIDTH` (720) wide, centered in the
  card and `SPACE_3` clear of its sides when the card is narrower: the context row over
  the box. A draft session, one with no message yet, sets the column a little above the
  card's middle: the space above it is `DRAFT_RISE` (45%) of the height left. A
  session that has seen work docks the column to the card's bottom, `SPACE_3` above its
  edge, under its conversation. Pages and the welcome are as they were. A draft's card
  shows a pattern behind its column, chosen from the card's corner: [backdrop.md](backdrop.md).
- **The box** reads as a text field: `composer_background`, opaque (`202021` dark, the
  selected tab's gray; white light), a `border` hairline that turns
  `composer_focus` while the text has focus, and `COMPOSER_RADIUS` (12) corners, rounder
  than a field's since it holds a toolbar too. `composer_focus` is text primary at 22%,
  quieter than a field's `focus_ring`: the box is large, and its cursor already shows
  where typing goes. Its content stands `SPACE_3` in, less the hairline. No shadow and
  no material of its own. A click anywhere in it puts the cursor in the text.
- **The text** shows "Ask anything, or describe a change" in `text_secondary` while
  empty. It starts `COMPOSER_MIN_LINES` (3) lines tall, so a draft's composer and a
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
- **The toolbar**, `CONTROL_SIZE` high, `SPACE_2` under the text, pulled left by half of
  what Attach's button adds around its icon, so the `+` stands in the text's column: Attach, a `border`
  hairline `ICON_SIZE` tall that sets it apart from the choices, then two pills, the
  model (Claude's logo, "Opus 5.5") and the effort (a gauge, "High"), then Send at the
  trailing edge.
  - Attach, "Attach files", opens the system's picker for files and folders, several at
    once, with an Attach button; cancelling it changes nothing, and a second click while
    it is open opens no other.
  - A pill is `CONTROL_SIZE` high with `CONTROL_RADIUS`: its mark at `ICON_SIZE`, the
    agent's logo in its brand's color (`claude`, `d97757` dark, `c6613f` light) or the
    gauge in `text_secondary`, its value in `TEXT_BODY` `text_secondary` and a chevron
    at `ICON_SIZE_SMALL`, unfilled at rest and on `hover`
    under the pointer and while its menu is open, as the sidebar's menu button is. Its
    tooltip names it: "Model", "Effort". A long value truncates.
  - Send is a `CONTROL_SIZE` circle of `text_primary` with an up arrow in
    `panel_background`: at 40% with "Type a message to send" while there is no
    attachment and the text is empty or only whitespace, full with "Send (↩)" otherwise.
    While the agent works and there is nothing to send, it is Stop, "Stop" its tooltip.
- **Attachments** are files and folders held as their paths, never their bytes, each
  once, in the order added, at most 20 (`crates/studio/src/composer/attachments.rs`).
  Each is an image (by its extension: png, jpg, jpeg, gif, webp), a folder or a file,
  decided when it is attached. Sending gives the agent an image as an image and anything
  else as its path in the text. They come in three ways:
  - **The picker**, from Attach.
  - **A drop** of files or folders from the Finder on the box. While a drag with paths
    hovers the box, its hairline turns `focus_ring` and "Drop to attach" in
    `text_secondary` takes the text's place; a drag that leaves or ends puts both back.
  - **A paste** (`Cmd-V` or Edit > Paste) of files copied in the Finder attaches them
    instead of pasting their names. An image with no text on the clipboard, such as a
    screenshot, is saved in the data folder's `attachments/` as `pasted-<hash>` and
    attached as an image: a PNG, JPEG, GIF or WebP keeps its bytes, any other format
    GPUI reads (TIFF, BMP) becomes a PNG, and pasting the same image again finds the
    same file. Text pastes as before.
  - Past 20, the paths with no room are left out and a line under the box says so ("A
    message takes up to 20 attachments, so 3 weren't added."), in `TEXT_SMALL`
    `text_primary` in line with the text, until the next edit or removal. A pasted image
    that can't be saved or read says why there the same way.
- **Chips.** Above the text, inside the box, one row of chips that wraps, `SPACE_1`
  apart; past two rows and half a third (`CHIPS_MAX_HEIGHT`, 78) it scrolls, so a
  draft's composer with ten lines still fits the card at the window's minimum size. A
  chip is `CONTROL_SIZE` high with `CONTROL_RADIUS` on `chip_background`: its kind's
  icon at `ICON_SIZE` in `text_secondary` (image, folder or file, from the same outline
  set; a thumbnail would mean GPUI decoding each image whole), then its name in
  `TEXT_SMALL` `text_primary`, cut in the middle so the extension stays, the chip at
  most `CHIP_MAX_WIDTH` (200). Under the pointer it shows a remove `×`, `ICON_SIZE_SMALL`
  in an `ICON_SIZE` square that takes clicks over `FIELD_TARGET_SIZE` and turns
  `text_primary` on `hover` under the pointer, "Remove" its tooltip; its space is kept while hidden, so nothing shifts. The name's
  tooltip is the whole path, `~` for home, which tells two `notes.txt` apart. Removing
  the last chip removes the row.
- **Pickers.** A pill's click opens its menu ([design.md](design.md) has the menu
  rules) upward from the pill's leading edge, draft or docked: the composer sits low in
  the card, and a menu that never covers its pill can't take the next click on the pill
  as a pick. The model menu lists each agent's name ("Claude Code") over its models,
  Fable 5.1, Opus 5.5, Sonnet 5.5, Haiku 5.5, each after the agent's logo; the effort
  menu Low, Medium, High, Extra high, Max. A check marks the current value. Focus goes
  back to the text when a picker closes.
- **Choices are saved** with the session at once, and the last one made becomes what a
  new session, and a draft opened again, start from. A file from before choices starts
  on Opus 5.5, High. A model or agent this build doesn't know (a newer
  build's, or a retired model) shows as written, checked under its agent, and stays in
  the file until another is picked; an effort or access it doesn't know reads as the
  default. The agents and their options are a table in `crates/workspace/src/agents.rs`,
  which turns a choice into the agent's arguments: `--model`, `--effort` and
  `--permission-mode`. A choice made while the agent runs goes to it at once.
- **Access is not a choice.** Every session runs in Full access
  (`--permission-mode bypassPermissions`): the agent edits and runs commands without
  asking. A session saved with another access by an older build still starts in Full
  access; the saved value stays in the file.
- **The problem line** under the box, in `TEXT_SMALL` `text_primary` in line with the
  text, also says why a message couldn't be sent or a choice was refused, a command in
  backticks in the monospaced font, until the next edit.
- **The context row**, over the box and `SPACE_1` above it, `CONTROL_SIZE` tall, in
  `TEXT_BODY` `text_secondary`, its icons at `ICON_SIZE` in the column of the box's text:
  - On the box's leading edge, the project's pill: a folder, or a computer for No
    project, the project's name as the sidebar shows it and a chevron at
    `ICON_SIZE_SMALL`, `SPACE_2` apart, on
    `composer_background` under a `border` hairline with `CONTROL_RADIUS`, padded as the
    box is, `hover` washed over it under the pointer and while its menu is open; no
    tooltip. A long name truncates before the branch does.
  - At the trailing edge, the branch its checkout is on, or the commit, cut in the
    middle, at most 40% of the row, the whole of it in the tooltip. Outside git, and in
    No project, there is no branch. It is read as [sidebar.md](sidebar.md) says.
- **The project menu**, from the pill's leading edge, so the pill, the menu and the box
  stand on one edge, never two side by side: downward from a draft, upward from a docked
  composer, 256 wide at least. It leads with a "Search projects" field, 36 tall over a
  hairline, its text starting in the items' icon column, which takes the focus and
  starts empty; then "No project" after a computer, for a session that works on the
  computer itself rather than in a folder, and a hairline; then the folder projects,
  most recently active first, each after a folder. Each shows while its name contains
  the search, folding case and apostrophes as the session search does (`o'z` finds
  `oʻzbek`), the session's own checked at the row's end; "No matching projects" when
  none does; then a hairline and "New project", which opens Add project. Its hairlines
  run edge to edge. Arrows move the highlight, Enter picks it, else the first item.
  Picking another project shows its draft, reopened if it has one, as New session there
  would; picking the session's own changes nothing.
- **No project** is a project of its own kind, made the first time it is picked, at
  most one: its sessions run in the home folder, and no branch is read for it. Adding
  the home folder with Add project opens it, as adding any folder already there opens
  that project.
- **One composer per open session.** A message half written, its attachments included,
  stays with its session while another tab is shown, and goes when its tab closes; it
  isn't saved. A pasted image's file stays in `attachments/`. Showing a
  session's tab, at launch too, puts the cursor in its composer, unless a palette, a
  menu, the quit dialog or the search holds focus. Focus given back to a composer whose
  session is off screen, as when a palette closes, moves on to the one on screen, and so
  does focus left nowhere by a closed tab. A page on screen gives focus to the window.

## Contrast

WCAG 2 contrast on the composer's box (dark / light). The box is opaque, so its text
reads the same over any desktop; the rows not recomputed below are from the
translucent field it had before:

| Against the field | Contrast |
| --- | --- |
| Placeholder, `text_secondary` | 7.58 / 7.72 |
| Text, `text_primary` | 14.52 / 17.24 |
| `composer_focus` hairline | 1.96 sampled, dark |
| Send's arrow, against its fill | 6.84 / 9.26 |

The focused hairline is under the 3:1 a field's `focus_ring` keeps, on purpose: the
box's cursor shows focus, and the owner asked for a quieter edge. Over the card it
reads 2.28.

Sampled on screen over the desktop behind the window at the time, the placeholder read
6.58 / 6.50 and the text 12.62 / 14.55, between the arithmetic's worst and best.

`chip_background` is text primary at 6% (`eff2f80f` / `191b1f0f`), a new role: a chip
at rest, which no other fill does. On a chip, over the field, the card and the window
(glass, dark / light; solid within these):

| On a chip | Contrast |
| --- | --- |
| Name, `text_primary` | 10.08 / 11.79 |
| Icon and `×`, `text_secondary` | 5.26 / 5.28 |
| `×` on its `hover`, `text_primary` | 8.53 / 10.57 |

Sampled on screen, the name read 10.72 / 12.90 and the icon 5.59 / 5.76.
