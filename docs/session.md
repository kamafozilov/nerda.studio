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
  scrolled back to the bottom.
- **The person's message** sits at the column's trailing edge, at most
  `MESSAGE_MAX_WIDTH` (80%) of it, on `field_background` with `COMPOSER_RADIUS` corners,
  `SPACE_3` by `SPACE_2` in, its text in `TEXT_BODY` `text_primary` with its line breaks.
  Its attachments are the composer's chips without the remove button, over the text: a
  file or folder by its name, the whole path in the tooltip; an image, sent by its
  bytes, by its format ("PNG image"). One that waits for the turn before shows "Queued"
  under it, in `TEXT_SMALL` `text_secondary`.
- **The agent's text** fills the column in `text_primary`, as Markdown (CommonMark, by
  `pulldown-cmark`): paragraphs `SPACE_2` apart; headings by weight (semibold), not
  size; bold, italic and struck text; inline code and code blocks in `MONOSPACE_FONT`,
  the platform's (SF Mono on macOS); a block on `field_background` with
  `CONTROL_RADIUS`, scrolling sideways when a line is longer than the column, while a
  vertical wheel over it still scrolls the conversation; bullet and numbered lists, their
  markers in a 20 pt column; quotes behind a `border` rule. Links show as their text.
  A line break stays one, as in the terminal the agent writes for. Text renders as it
  streams; a code fence not closed yet runs to the end.
- **Thinking** is one `text_secondary` row with a chevron: "Thinking… 3s" while it
  streams, "Thinking" after. A click opens it to the summary, in `text_secondary`, in
  line with the label.
- **Tool calls and prompts** are one `TEXT_SMALL` `text_secondary` line each for now:
  "Bash · running", "Read · done"; a prompt that waits says "The agent is waiting for
  an answer to run Bash." (or that it asked a question, or proposed a plan), then
  "allowed", "denied" or "not answered". Stop denies a waiting prompt.
- **How a turn ended**, in `TEXT_SMALL` under it: "Worked for 1m 12s" (whole seconds,
  at least one; minutes and hours past them) in `text_secondary`; "Stopped"; or a failure
  as the problem line says it, `text_primary`: what went wrong, then what to do, such as
  "The agent isn't signed in." over "Run `claude auth login` in a terminal, then try
  again."
- **Working.** While a turn runs or a message waits, "Working… 12s" in `TEXT_SMALL`
  `text_secondary` stands over the composer, in line with its text. Its `CONTROL_SIZE`
  row is kept while idle, so the composer never moves. The clocks redraw once a second
  while the agent works.

Every text here keeps a contrast measured before: `text_primary` and `text_secondary` on
the card (design.md), `text_primary` on `field_background` (composer.md).

## Not yet

Answering prompts, questions and plans, tool calls with their input and output, failure
cards with more than words, the sessions' status in the sidebar, opening links, selecting
text and image thumbnails come later. The rows are the seam: a kind of row draws one item
of the transcript, so a richer one takes its place without touching the rest.

GPUI wraps lines at characters it doesn't know as part of a word, and the Uzbek ʻ
(U+02BB) is one of them: a long line can break between "o" and "ʻ".
