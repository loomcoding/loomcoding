# Loomcoding user documentation

This is the user guide. The reference version is in English under [`en/`](en/start/index.md), and the Spanish mirror is in [`es/`](es/start/index.md). Both trees have the same pages, paths and section titles.

## Index

### Start

| Page | English | Español |
|---|---|---|
| Start here | [en](en/start/index.md) | [es](es/start/index.md) |
| Install from source | [en](en/start/install.md) | [es](es/start/install.md) |
| What happens on the first run | [en](en/start/first-run.md) | [es](es/start/first-run.md) |
| Connect a cloud provider | [en](en/start/cloud-provider.md) | [es](es/start/cloud-provider.md) |
| Run a local model server | [en](en/start/local-provider.md) | [es](es/start/local-provider.md) |
| Send your first prompt | [en](en/start/first-prompt.md) | [es](es/start/first-prompt.md) |

### Concepts

| Page | English | Español |
|---|---|---|
| Agent, subagents and jobs | [en](en/concepts/agent-subagents-jobs.md) | [es](es/concepts/agent-subagents-jobs.md) |
| Sessions | [en](en/concepts/sessions.md) | [es](es/concepts/sessions.md) |
| Context and compaction | [en](en/concepts/context-and-compaction.md) | [es](es/concepts/context-and-compaction.md) |
| Permissions | [en](en/concepts/permissions.md) | [es](es/concepts/permissions.md) |
| ODD and flows (`/flow`) | [en](en/concepts/flows-odd.md) | [es](es/concepts/flows-odd.md) |
| A2A | [en](en/concepts/a2a.md) | [es](es/concepts/a2a.md) |

### Coming next

- `guides/`: one task per page.
- `reference/`: commands, shortcuts, `loomcoding.toml`, environment variables, profile folders.
- `problems/`: symptom, cause, fix.

## Style guide

1. **Paragraphs of four lines at most.** If you need more, write a list or a table. Lists, tables, quotes and code blocks do not count as paragraphs.
2. **Schematize first.** Use lists, tables, numbered steps and Mermaid diagrams before prose.
3. **Example first, explanation after.**
4. **Open every page with the three headers, in this order, before the first `##` section:**
   - English: `**What it is**`, `**What it is for**`, `**How**`.
   - Español: `**Qué es**`, `**Para qué**`, `**Cómo**`.
   The opening takes five lines at most.
5. **One idea per page.** When a page grows long, split it.
6. **Titles answer a question or name an action.** Write "Switch model", not "Models".
7. **One image per concept, only when it saves text.** Every image needs alt text. Diagrams are Mermaid; screenshots are generated from real terminal dumps, never drawn by hand.
8. **No personal data.** No Windows user names, no absolute paths from your machine, no emails, no real names. Use placeholders such as `<user>` and `@example.invalid`.
9. **Every setting and every command states its default value and whether it requires a restart.** Use the table in the template.
10. **Neutral tone in both languages.** No regional slang, no jokes that depend on a country.
11. **Parity.** A page exists in `en/` and in `es/` at the same path, with the same number of `##` sections. `en/` is the reference.
12. **Relative links only,** and every link must resolve. Link to technical documents when needed; do not copy them.
13. **Start from the templates:** [`en/_template.md`](en/_template.md) and [`es/_plantilla.md`](es/_plantilla.md). Files whose name starts with `_` are not pages.

## How the rules are checked

The test `docs_usuario_cumple_la_guardia` in `crates/loomcoding-cli/tests/docs_usuario.rs` runs with `cargo test`. It fails, naming file and line, when a page:

- has a paragraph of more than four lines;
- has a relative link that does not resolve (URLs are not checked);
- contains a personal data pattern (a Windows user folder, a home folder with a real name, or an email other than `@example.invalid`);
- lacks the opening headers of its language, or places them after the first `##` section;
- has an image with empty alt text;
- exists in one language and not in the other, or has a different number of `##` sections.

## Build and publish

The website (Docusaurus, `site/`) is not built yet. This section will describe how to build and publish it when that step lands.
