# Agent sessions

`crates/agent` runs a session's coding agent as a process and turns what the agent writes
into a transcript the screen can draw. It has no UI, and the studio doesn't use it yet. The
agents a session can run, and the choice of model, effort and access, are the table in
`crates/workspace/src/agents.rs`.

## One process per session

- **Starting.** `Session::start` runs the agent's command, `claude`, looked up on PATH
  unless the `Launch` names another program, with the project folder as its working
  folder. Its arguments make it read and write one JSON object per line, stream its reply,
  echo each message it takes up and ask the app before a tool runs; then the session's
  choice as `--model`, `--effort` and `--permission-mode`; then `--session-id <id>` for a
  new conversation or `--resume <id>` for one that ran before. The app picks the id, a
  `ConversationId`, so it can save the id before the agent says anything. Agents start
  one at a time: on macOS a child started while another's pipes are being made can keep
  those pipes open, and an agent that holds another's input open keeps it from seeing
  that input close.
- **Ready.** The first line the session writes is the `initialize` request. The session is
  starting until the agent answers it, about 0.75 s after launch, and then ready.
- **Messages.** Each message is one line: its text, then any attached files by absolute
  path under "Attached files:", then images inline as base64. The agent reads PNG, JPEG,
  GIF and WebP; `Attachment::open` reads those in and names any other file by its path.
  Text of whitespace alone sends no text block. A message sent while a turn runs waits in
  the agent's queue and runs after it.
- **Changes mid-session.** A new model takes effect from the agent's next model call, a
  new effort from the next turn, and a new access at once. Ask first goes as `manual`,
  which the agent reads back as `default`. The agent refuses a switch to Full access
  mid-session unless the process started in it, and answers with an error naming the flag
  it lacks. An effort or access the agent doesn't offer sends nothing.
- **Permission prompts.** When a tool needs approval, the agent asks and waits. A prompt
  never times out. `Permission::allow` answers with the tool's input as asked, and
  `Permission::deny` with a reason the agent reads. The agent may withdraw a prompt
  itself, as it does when the turn stops.
- **Stop.** `Command::Interrupt` ends the running turn and drops the messages queued
  behind it. The process stays up, and the next message runs as usual.
- **Requests the app doesn't handle.** The agent may ask the app for other things, such
  as an answer to a question from one of its tools. The session declines each one at once,
  so the agent never waits on it.
- **Ending.** Dropping the `Session` closes the agent's input. An idle agent quits at
  once with code 0. An agent that hasn't quit 2 s later is killed. The session looks at
  the process itself every 50 ms to see that it has ended, rather than waiting for its
  output to close.

## When something goes wrong

- **No program.** `Session::start` returns a `StartError` in words to show, such as
  "Couldn't start claude: it isn't installed, or isn't on PATH." It does the same for a
  project folder that isn't there and for an agent this build doesn't know. The caller
  applies `Event::Failed` with those words, and the session shows as failed.
- **The agent quits mid-turn.** Every open turn ends with "The agent quit before it
  finished (exit code 3).", or "The agent was stopped before it finished." when a signal
  ended it. The exit comes after what the agent wrote. Once the agent has quit, the
  session waits at most half a second for the last of its output, since a process the
  agent started may hold its pipes open.
- **A line that isn't an event.** It arrives as `Event::Unreadable` with the line and why,
  and the session goes on. Lines of a kind the crate doesn't model, such as hook progress,
  a subagent's own messages, or a kind a newer agent adds, arrive as `Incoming::Other`
  and change nothing.
- **A message after the agent quit.** Its turn ends at once with "The agent wasn't
  running, so the message wasn't sent." Every command `Session::send` takes is reported
  as sent, written or not, so no message goes missing without a word.

## Events

A session's events arrive on one channel, `Events`, which an async task drains. They come
in the order they happened.

- `Started` comes first, for a new process and for one resuming its conversation.
- `Sent` carries a command as it is written, before anything the agent says about it.
- `Received` carries a line the agent wrote, parsed: a turn's start, streamed text, thinking
  and tool calls, finished blocks, tool output, a turn's result with its cost and usage,
  permission prompts and their withdrawal, answers to the app's requests, rate limits,
  status, and the queue's moves.
- `Unreadable` and `Stderr` are diagnostics.
- `Exited` comes last, with the exit code, or none when a signal ended the process.

## The transcript

`Transcript::apply` reduces events into a list of turns. A turn holds the person's message,
then what came back, in the order it came, then how the turn ended.

- **A turn's state** is queued until the agent takes the message up, then running, then
  ended. It ends finished, with what it cost, its tokens and its duration; interrupted,
  before or while it ran; or errored, with the agent's words or the reason the agent
  quit.
- **Items** are text, thinking, tool calls and permission prompts. A tool call shows its
  name and input, and runs until its output comes back, done or failed. A prompt is
  pending until the person allows or denies it, or until it is cancelled.
- **Streaming.** Text and thinking grow as the reply streams, and the finished block takes
  the streamed one's place, so nothing shows twice. Without streaming, the finished
  blocks show alone.
- **One turn per message.** The agent echoes a message when it takes it up. The echo moves
  the queued turn to running and adds nothing.
- **Cost.** The agent reports what its process has cost so far. A turn's cost is what the
  turn added, and a new process counts from zero.
- **An ended turn** has nothing left streaming, running or waiting: a tool still running
  fails with no output, and a pending prompt is cancelled.
- **A turn the agent begins on its own**, as when a background task of its finishes, has
  no message.

## Status

The session's status is one state machine, in `crates/agent/src/transcript/status.rs`.

| Status | Means | Leaves for |
| --- | --- | --- |
| Starting | The process is up and hasn't answered `initialize` | Ready, or Running if a message waits; Failed if the agent refuses |
| Ready | Idle | Running when a message is sent |
| Running | A turn runs or waits in the queue | Awaiting permission, Stopping, Ready |
| Awaiting permission | A prompt waits for the person | Running once answered or withdrawn |
| Stopping | Stop was sent while a turn ran | Ready when the turns end; Running when a new message is sent |
| Exited | The process ended | Starting when a new process starts |
| Failed | The session couldn't start | Starting when a new process starts |

Stop while idle changes nothing. A failure moves any status to Failed. An exit moves
any status but Failed to Exited, so a failure keeps its words when the process then
exits.

## Tests

The fixtures in `crates/agent/tests/fixtures` hold the wire of real runs of `claude`
2.1.309, trimmed and with private paths replaced. A `{"host": …}` line stands where the
app wrote. The transcript tests play them line by line. The session tests run
`fake-agent`, a binary of this crate in `tests/support/fake_agent.rs`, which plays a
fixture as the agent and records what the app wrote to it.

One test runs the installed `claude` on haiku, which cost $0.0035 on each of its runs.
It is ignored, so run it on purpose:

```sh
cargo test -p agent --test real -- --ignored
```
