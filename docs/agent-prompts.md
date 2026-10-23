# Agent prompts

When a tool needs approval, the agent asks the app and waits. A prompt never times out.
The agent may withdraw one itself, as it does when the turn stops, and a turn that ends
cancels the prompts it left open. The rest of the agent's sessions are in
[agent.md](agent.md).

## What a prompt asks

`PermissionRequest::ask` says what the prompt is about, by the tool asking:

- `Ask::Tool`: a tool wants to run on its input. The prompt names the tool, why the
  agent asks, and the path that made it ask, when there is one. With `default_to_no`,
  approving must take a deliberate choice.
- `Ask::Questions`: the agent's questions for the person, from `AskUserQuestion`. Each
  has its full text, a short header, its options with a label and a description, and
  whether more than one option may be picked.
- `Ask::Plan`: the plan the agent proposes in plan mode, in Markdown, from
  `ExitPlanMode`.

A question or plan whose input doesn't read as one asks as any other tool does.

## Answering

`Permission::answer` turns the person's `Answer` into the command to send:

- `AllowOnce` runs the tool as asked. For a plan it approves the plan, and the agent
  leaves plan mode: claude 2.1.309 then reported the `default` mode.
- `AllowForSession` runs the tool and lets it run without asking for the rest of the
  session. The agent's suggested rules and folders go back with their destination set to
  the session. A suggested change of mode is left out, and nothing is written to a
  settings file, such as the project's `.claude/settings.local.json`. With nothing to
  keep, a rule allowing the tool goes back. Where the agent suppresses such a rule, and
  for questions and plans, it allows once.
- `Deny { feedback }` refuses the tool, and the agent reads what the person said. For a
  plan it keeps the agent planning, the feedback telling it what to change.
- `Cancel` refuses the tool and stops the turn.

`Permission::answer_questions` answers the agent's questions, the i-th list holding the
labels picked for the i-th question, or the person's own words. The answers go back keyed
by each question's text: one label, or a list where the question takes several. Stop
denies every open prompt before it interrupts the turn.

## Planning costs more

In plan mode claude 2.1.309 ran its planning on a larger model than the session's. A
session on haiku made its three model calls before the plan's approval on
claude-sonnet-5-5, and the two after it on haiku. That turn cost $0.155, against about
$0.004 for a short haiku turn.
