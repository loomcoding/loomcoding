# Commands reference

**What it is**

Every slash command that Loomcoding runs locally in the composer. Commands are never sent to the model. This page lists all 32 entries of the command table in the code, with what each one does, an example, and when it works.

**What it is for**

- Finding the command that changes a setting or opens a panel.
- Checking why a command was refused.
- Knowing which commands are hidden from the `/` menu.

**How**

1. Type `/` at the start of the composer to open the menu, or type the full name.
2. Press Enter to run the command. The text is cleared when the command runs.
3. If a command is refused, the reason is shown and the text you typed stays in the composer.

## Example

```text
> /model nvidia/nemotron-3.5-super-vl:free
> /effort high
> /flow odd off
```

The first two commands apply at once when no turn is running. The third saves the flow setting; it applies when Loom restarts.

## All commands

Entries follow the order of the code table. "When idle" means no turn is running.

| Command | What it does | Example | Available when |
|---|---|---|---|
| `/help` | Opens the help panel. Alone, it shows the keyboard shortcuts (same as F1). `/help topics` lists the topics. `/help <command>` shows one command's usage and example. `/help <topic>` opens a topic page. Tab switches between the shortcuts and the topics. | `/help model`, `/help topics` | Always. An unknown name is refused: the text stays in the composer, with a suggestion when one is close. |
| `/new` | Starts a new session. | `/new` | When idle. Otherwise: "Wait for the active turn before switching sessions". |
| `/sessions` | Opens the session picker. Same as F3. | `/sessions` | When idle, with the same message as `/new`. |
| `/settings` | Opens the provider and model settings panel. Same as F2. | `/settings` | When provider settings are loaded. |
| `/model` | Changes the model. Without an id, opens the model picker. | `/model <id>` | When idle and no save is running. |
| `/effort` | Changes the reasoning effort. Without a level, opens the picker. | `/effort high`, `/effort default` | When idle, and the model offers effort levels. |
| `/provider` | Switches the provider. Without an id, opens the picker. | `/provider llama` | When idle and no save is running. |
| `/queue` | Opens the panel of pending prompts. Same as Ctrl+K. | `/queue` | Always. |
| `/tools` | Opens the panel of recent tool calls. Same as Ctrl+T. | `/tools` | Always. |
| `/theme` | Cycles the terminal colour theme and shows its name. | `/theme` | Always. Hidden alias. |
| `/layout` | Cycles the screen layout: rail, minimal, cards. | `/layout` | Always. Hidden alias. |
| `/temas` | Opens the interface panel. Same as `/ui` without arguments. | `/temas` | Always. Hidden alias. |
| `/ui` | Opens the interface panel, or sets one row: `/ui <row> [value]`. | `/ui mouse off` | Always. Unknown rows are refused. |
| `/crush` | Switches to the Crush look. | `/crush` | Always. Hidden alias. |
| `/classic` | Switches back to the Cloth look. | `/classic` | Always. Hidden alias. |
| `/cancel` | Cancels the active turn and pauses the queue. | `/cancel` | Only while a turn runs. Otherwise: "No active turn to cancel". |
| `/pause` | Pauses the pending prompts. | `/pause` | Always. |
| `/resume` | Resumes the pending prompts. | `/resume` | Always. |
| `/parent` | Goes back to the parent session. Same as Alt+Left. | `/parent` | When idle and a parent session exists. |
| `/sidebar` | Shows or hides the workspace sidebar. Same as Ctrl+B. | `/sidebar` | Always. Hidden alias. |
| `/mouse` | Turns mouse capture on or off. When off, you can select text with the mouse. | `/mouse` | Always. Hidden alias. It toggles; any argument is ignored. |
| `/mascot` | Shows or hides the status mascot. When off, the status is one text line. | `/mascot` | Always. Hidden alias. |
| `/logofx` | Sets the logo effects: `on`, `off`, or a number of seconds between bursts. | `/logofx 30` | Always. |
| `/verbose` | Shows or hides the provider request notices in the transcript. Without an argument, it toggles. | `/verbose on` | Always. Changes this session only; it is not saved. |
| `/flow` | Lists the flow modes, or saves one as on or off. | `/flow odd off` | Always. Saving needs an open project. Applies on restart. |
| `/agent` | Runs a named agent on a task. You can also write `@<name>`. | `/agent <name> [--skills a,b] <task>` | When the agent is loaded and usable. |
| `/focus` | Sends what you type next to a subagent. Without an id, returns to this session. | `/focus <id>` | The id must belong to a live subagent. Ctrl+G cycles them. |
| `/agents` | Lists the loaded agents. | `/agents` | Always. |
| `/a2a` | Opens the panel for A2A cards, tasks and exchanges. | `/a2a` | Always. |
| `/refresh-agents` | Downloads the cards of the remote agents again. | `/refresh-agents` | When `[a2a] role` is `consume` or `both`. Otherwise: "Remote agents are not enabled". |
| `/compact` | Summarizes the older history now. | `/compact` | When a session exists and idle. Otherwise a reason is shown. |
| `/quit` | Exits Loomcoding. Same as Ctrl+Q. | `/quit` | Always. |

## Hidden aliases

Eight commands still run when you type their full name, but the `/` menu does not list them. The menu has room only for the entries of `/ui`, which covers the same settings.

| Hidden command | Covered in the menu by |
|---|---|
| `/theme`, `/layout`, `/crush`, `/classic` | `/ui` (theme, style, layout) |
| `/temas` | `/ui` (interface panel) |
| `/mascot`, `/sidebar`, `/mouse` | `/ui` (mascot, sidebar, mouse) |

## Refused commands

A refused command keeps its text in the composer and shows one of these messages.

| Message | Commands |
|---|---|
| Wait for the active turn before switching sessions | `/new`, `/sessions` |
| Wait for the active turn before applying settings | `/model`, `/provider`, `/effort` |
| Provider settings are unavailable | `/settings`, `/model`, `/provider`, `/effort` |
| No active turn to cancel | `/cancel` |
| No session to compact; Wait for the active turn before compacting; Already compacting | `/compact` |
| Remote agents are not enabled ([a2a] role) | `/refresh-agents` |
| A usage line that shows the accepted arguments | `/verbose`, `/ui`, `/flow` |
| Unknown row: <name>. Rows: theme, style, layout, carril, effects, spinner, mascot, sidebar, mouse | `/ui` |
| Unknown command · /help for commands · Esc to send as text | Any unknown word that starts with a slash |
| No help for '<name>' · did you mean /<name>? · /help lists the topics | `/help`, with a name that is not a command or a topic |

## Next steps

- [Keyboard shortcuts](shortcuts.md)
- [Configuration (`loomcoding.toml`)](configuration.md)
- [Sessions](../concepts/sessions.md)
- [ODD and flows (`/flow`)](../concepts/flows-odd.md)
