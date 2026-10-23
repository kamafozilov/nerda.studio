# Agent sessions

`crates/agent` runs a session's coding agent as a process and turns what the agent writes
into a transcript the screen can draw. It has no UI; the studio runs it as
[session.md](session.md) says. The
agents a session can run, and the choice of model, effort and access, are the table in
`crates/workspace/src/agents.rs`. Prompts, the agent's questions and plans are in
[agent-prompts.md](agent-prompts.md).

## One process at a time

- **Finding the program.** `Session::start` runs the agent's command, `claude`, unless the
  `Launch` names another program. A bare name is looked up as a terminal would: on the
  app's PATH, then on the PATH the person's login shell sets up, then in the folders
  installers use: `~/.local/bin`, `~/.claude/local`, the newest of
  `~/.local/share/claude/versions/*`, `/opt/homebrew/bin`, `/usr/local/bin` and
  `~/.npm-global/bin`. An app opened from Finder or the Dock has only a bare PATH, so the
  login shell's matters. The session reads it once per run of the app, from
  `$SHELL -lic` with `-lc` as the fallback, and gives up on an attempt after 5 s. The
  first start waits for it; `agent::prewarm()` reads it ahead on a thread. The agent
  runs with its program's folder, the app's PATH and the login shell's PATH, each folder
  once, so the tools it runs find what the person's terminal finds.
- **Starting.** The agent starts in the project folder, in a process group of its own.
  Its arguments make it read and write one JSON object per line, stream its reply, echo
  each message it takes up, write a summary of its thinking, and ask the app before a
  tool runs. Then come the session's choice as `--model`, `--effort` and
  `--permission-mode`, then `--session-id <id>` for a new conversation or
  `--resume <id>` for one that ran before. The app picks the id, a `ConversationId`, so
  it can save the id before the agent says anything. Every child the crate starts,
  agents, the login shell and `ps` alike, starts one at a time: on macOS a child started
  while another's pipes are being made can keep those pipes open, and an agent whose
  input another child holds open never sees that input close.
- **Ready.** The first line the session writes is the `initialize` request. The session is
  starting until the agent answers it, about 0.75 s after launch, and then ready.
- **Messages.** Each message is one line: its images first, then its text, with any other
  attached files by absolute path under "Attached files:". The agent expands a
  `/command` only from the last text block, and only when the command leads it. The
  session writes each message under a new id, which its `Event::Sent` carries, since
  claude 2.1.309 answers a repeated id as completed and never runs it. Text of whitespace
  alone sends no text block. A message sent while a turn runs waits in the agent's queue
  and runs after it.
- **Attachments.** `Attachment::open` reads a PNG, JPEG, GIF or WebP of at most 5 MiB in
  as an image, known by its first bytes whatever its name. Any other file goes by its
  absolute path, and so does an image that is larger or can't be read, so attaching
  never fails. A 4.6 MiB PNG went through claude 2.1.309 on haiku as it was.
- **Changes mid-session.** A new model takes effect from the agent's next model call, a
  new effort from the next turn, and a new access at once. Ask first goes as `manual`,
  which the agent reads back as `default`. An effort or access the agent doesn't offer
  sends nothing.
- **Full access.** A running agent can't change into or out of Full access, so switching
  to or from it ends the agent and starts it again in the new mode, on the current
  model and effort. The new process resumes the conversation once a turn is on record,
  and starts it anew under the same id before then: claude 2.1.309 finds nothing to
  resume for a conversation without a turn. The events show the old process's exit,
  then `Started`. While a turn runs or a message waits, the switch is refused with
  `Event::Refused` and its reason in words, and the agent goes on as it was. Only Full
  access starts the agent with permission checks off.
- **Stop.** `Command::Interrupt` sends `stop_task` for each task the agent runs in the
  background, denies each prompt it waits on, then ends the running turn and drops the
  messages queued behind it. Each of these is reported as sent. A message the agent drops
  comes back whole as `Event::Dropped`, so it can be sent again. The process stays up,
  and the next message runs as usual.
- **Requests the app doesn't handle.** The agent may ask the app for other things, such
  as an elicitation from an MCP server. The session declines each one at once, so the
  agent never waits on it.
- **Ending.** Dropping the `Session` ends the agent and everything it started. The session
  notes every process the agent has started, closes the agent's input, sends SIGTERM to
  the agent's group if the agent hasn't quit 2 s later, and SIGKILL 2 s after that.
  Before closing the input and before each signal it waits, at most 40 s, while the
  agent's sign-in refresh lock is fresh, in `~/.claude` unless the environment moves it:
  a refresh token works once, and an agent killed mid-refresh would sign the person out.
  Once the agent has quit, what is left of its group and of the processes it started
  gets SIGTERM, then SIGKILL after 1 s. Those processes are found by walking the process
  table, so a dev server that Bash started in a group of its own goes too. An idle agent quits at once with code 0. The exit comes as the
  last event, after all of that, so an app that quits drops its sessions and waits for
  each one's `Exited`. On Windows the agent gets a new process group, and `taskkill /T`
  ends its tree.
- **Watching the process.** The session looks at the process itself every 50 ms to see
  that it has ended, rather than waiting for its output to close.

## When something goes wrong

- **No program.** `Session::start` returns a `StartError` in words to show, such as
  "Couldn't find claude. Install it with `curl -fsSL https://claude.ai/install.sh | bash`
  in a terminal, then try again." It does the same for a project folder that isn't there
  and for an agent this build doesn't know. The caller applies `Event::Failed` with those
  words, and the session shows as failed. A start that fails when Full access is switched
  comes as `Event::Failed` as well.
- **A failed turn.** A turn that fails ends with one `Failure`, each with a sentence to
  show from `message()` and, where there is one, what to do from `suggestion()`:

  | Failure | When |
  | --- | --- |
  | `UsageLimit { resets_at }` | A rate limit rejected the turn, the agent's error code is `rate_limit`, or its words say the limit is reached. The reset time is seconds since the epoch, read from seconds, milliseconds or a date. |
  | `SignedOut { message }` | The code is `authentication_failed`, the HTTP status is 401, or the words say the sign-in is gone. The suggestion is `claude auth login`. |
  | `Overloaded` | The code is `overloaded`, or the status is 529. |
  | `Provider { status, message }` | The code is `server_error`, or the status is 500 or more. |
  | `Crashed { code, stderr }` | The agent quit before the turn ended. `stderr` holds its last 8 KB, credentials removed. |
  | `NotSent` | The message was sent after the agent had quit. |
  | `NeverStarted` | The agent completed the message without starting it. |
  | `Other(words)` | Anything else, in the agent's words, or in words for the result's reason when it gave none. |

  The agent reports a failed model request in an assistant message of its own, with an
  error code and HTTP status. That message shows as the failure, not as a reply. A
  result marked success fails its turn when its reason or HTTP status says so, or when
  the turn ran into an error and has no reply. Its errors read as strings or objects. A
  rate limit doesn't count when extra usage covers it, and a subagent's never does. The
  agent's own diagnostics, `[ede_diagnostic] …` among errors and `[claude-code:…] …` on
  stderr, are left out of what is shown.
- **A line that isn't an event.** It arrives as `Event::Unreadable` with the line and why,
  and the session goes on. Lines of a kind the crate doesn't model, such as hook progress,
  a subagent's own messages, or a kind a newer agent adds, arrive as `Incoming::Other`
  and change nothing.
- **A message after the agent quit.** Every command `Session::send` takes is reported as
  sent, written or not, so no message goes missing without a word.

## Events

A session's events arrive on one channel, `Events`, which an async task drains. They come
in the order they happened.

- `Started` comes first, for a new process and for one resuming its conversation.
- `Sent` carries a command as it is written, before anything the agent says about it.
- `Refused` carries a command the session didn't carry out, and why.
- `Received` carries a line the agent wrote, parsed: a turn's start, streamed text, thinking
  and tool calls, finished blocks, tool output, a turn's result with its cost and usage,
  prompts and their withdrawal, answers to the app's requests, rate limits, status, the
  agent's tasks, and the queue's moves.
- `Dropped` carries a waiting message the agent dropped on stop.
- `Unreadable` and `Stderr` are diagnostics.
- `Exited` comes last for a process, with the exit code, or none when a signal ended it.

Every event, and all it carries, serializes to JSON and reads back as it was, so the
studio can keep each session's events in a journal of its own, one line each. An image
goes as its bytes in base64 and reads back only as an image. `Transcript::replay`
rebuilds the transcript from a journal's events, and `workspace::Session` saves the
conversation id, so a restarted app shows a session as it was and resumes it with
`--resume`.

## The transcript

`Transcript::apply` reduces events into a list of turns. A turn holds the person's message,
then what came back, in the order it came, then how the turn ended.

- **A turn's state** is queued until the agent takes the message up, then running, then
  ended. It ends finished, with what it cost, its tokens and its duration; interrupted,
  before or while it ran; or failed, with its `Failure`.
- **Items** are text, thinking, tool calls and prompts. A tool call shows its name and
  input, and runs until its output comes back, done or failed.
- **Streaming.** Text and thinking grow as the reply streams, and the finished block takes
  the streamed one's place, so nothing shows twice. Without streaming, the finished
  blocks show alone.
- **One turn per message.** The agent echoes a message when it takes it up, and only that
  moves the queued turn to running. A turn's result ends the messages it names. A result
  naming none ends the agent's own running turn, else the running one, but never a
  message still waiting: on resume the agent settles its old background tasks that way
  before it starts on the next message.
- **A turn the agent begins on its own**, as when a background task of its finishes, has
  no message. One that begins while messages wait goes before them.
- **Cost.** The agent reports what the conversation has cost so far by its count, and a
  turn's cost is what the turn added. A resumed agent goes on from the total it saved, as
  claude 2.1.309 did; one that counts from zero shows by a total below the last.
- **An ended turn** has nothing left streaming, running or waiting: a tool still running
  fails with no output, and a pending prompt is cancelled. A new process ends, as
  interrupted, whatever the last one left open, so a journal cut short by a crash never
  offers a prompt whose process is gone.
- **Models.** `Transcript::models` gives the models the agent offers, with their efforts,
  once it has answered `initialize`, and the catalog's list before then. The agent's
  "default" and models it marks hidden, disabled or needing an update are left out. The
  session's chosen model is always among them, at the end when no list names it.

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
app wrote. Among them are a refused API key, a model that doesn't exist, a turn whose
thinking streams a summary, and a plan-mode turn that asks a question, proposes a plan
and writes a file once approved. The transcript tests play them line by line. The
session tests run `fake-agent`, a binary of this crate in `tests/support/fake_agent.rs`,
which plays a fixture as the agent, records what the app wrote to it, pauses between
lines, and can start processes in its group or outside it. `tests/sign_in.rs` is its own
binary, since it sets the sign-in folder through the environment.

The agent's task lines are read in the shapes its documented types give; no recorded run
has them yet.

One test runs the installed `claude` on haiku, which cost $0.0035 on each of its runs.
It is ignored, so run it on purpose:

```sh
cargo test -p agent --test real -- --ignored
```
