# Keyboard shortcuts

**What it is**

The keys that Loomcoding handles in the composer, the transcript, the global screen and the panels. This page lists only the keys that the code handles today. A key that is not listed here is not documented.

**What it is for**

- Finding a key for an action without opening the help panel.
- Knowing which key works in which area.
- Checking the help panel, which lists the same keys, at the end.

**How**

1. Type in the composer. Tab moves focus between the composer and the transcript.
2. Global keys work in every area, unless a panel or dialog is open.
3. Panels take their own keys while they are open. Esc or the key shown in the panel title closes them.

## Example

```text
Empty composer:  ?        opens the help panel
Composer:        Ctrl+J   new line;  Enter sends the prompt
Transcript:      Tab, then j and k scroll; y copies the active block
```

## Composer

| Key | What it does |
|---|---|
| Enter | Sends the prompt. `/agent` and `@name` are read when you send. |
| Ctrl+J | Inserts a new line. |
| Tab | Completes an agent name if the composer holds one. Otherwise switches focus. |
| Shift+Tab | Switches focus backwards. |
| Shift+Left, Shift+Right, Shift+Up, Shift+Down, Shift+Home, Shift+End | Extends the text selection in the composer. |
| Ctrl+C | With a selection, copies it and stays open. Without a selection, quits. |
| Ctrl+V | With the composer focused, pastes the clipboard. An image is attached; otherwise the text is pasted. |
| Backspace | On an empty composer, removes the last image attachment. |
| Delete | Deletes the character after the cursor. |
| PageUp, PageDown | Scrolls the transcript by 10 lines. |
| ? | On an empty composer with no panel open, opens the help panel. Elsewhere it is a character. |

## Transcript

Press Tab until the transcript has focus.

| Key | What it does |
|---|---|
| Up or k, Down or j | Scrolls one line. With the Crush look (flat chrome), moves between blocks instead. |
| u, d | Half a page up, half a page down. |
| PageUp or b, PageDown or f | One page up, one page down. |
| Space | Folds or unfolds the active long block. When there is none, pages down. |
| Enter | Folds or unfolds the active long block, or opens or closes a subagent node. |
| g or Home, G or End | Jumps to the top, or follows the bottom. |
| [ and ] | Moves to the previous or the next block. |
| y or c | Copies the active block. |
| v | Selects the last line. Then j and k extend the selection, y or c copies it, and v clears it. |
| Left, Right | On a subagent node, folds or unfolds it. |

## Global keys

| Key | What it does |
|---|---|
| F1 | Opens the help panel. Esc or F1 closes it. |
| F2 | Opens the provider settings. |
| F3 | Opens the session picker. Not available while a turn runs. |
| Ctrl+T | Opens the panel of recent tool calls. |
| Ctrl+K | Opens the panel of pending prompts (the queue). |
| Ctrl+B | Shows or hides the sidebar. |
| Ctrl+O | Expands or collapses the latest tool output. |
| Ctrl+G | Cycles the target of the composer: this session, or a live subagent. |
| Ctrl+X | Emergency stop: cancels the turn, the subagents and the jobs. |
| Alt+Left | Goes back to the parent session. |
| Ctrl+Q | Quits. |
| Esc | Clears a text or composer selection. Otherwise closes the queue, returns focus from a side panel to the composer, or cancels the active turn. Press it twice within 350 ms to quit. |

## Panels

| Panel | Keys |
|---|---|
| Queue (Ctrl+K) | Up and Down select. Enter edits the selected prompt. Ctrl+Up and Ctrl+Down move it. Delete removes it. Space pauses or resumes the queue. Ctrl+X pauses the queue and cancels the turn, while a turn runs. Esc closes it. |
| Tool calls (Ctrl+T) | Up and Down select. PageUp and PageDown scroll. Left and Right scroll sideways. Home and End jump. y copies the output. Esc or Ctrl+T closes it. |
| Session picker (F3) | Up and Down, or j and k, move. Enter opens. n starts a new session. Ctrl+F toggles search. Esc leaves search first; then it closes the picker when opened from a chat, and quits otherwise. |
| A2A (`/a2a`) | Tab or Right change focus. Up and Down move. PageUp and PageDown scroll the details. Home goes to the top. Esc or q closes it. |
| Agents (`/agents`) | Up and Down move. Tab and Shift+Tab change focus. PageUp and PageDown jump 5. a approves and r rejects the selected agent. e edits it. / filters by text, t by tag, x clears the filters. Esc or q closes it. |
| Help (F1, `?`, `/help`) | F1, ? or Esc closes it. Tab switches between the shortcuts and the help topics. |

## The help panel

The help panel (F1) and this page agree: Enter sends the prompt, and Ctrl+J inserts a new line. The panel also lists the topics: `/help topics`.
