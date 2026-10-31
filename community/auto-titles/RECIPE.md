# Auto Titles

Live task titles in your existing Herdr desktop window: tabs, pane headers, agent rows, sub-agent previews and the tab picker follow the task reported by a terminal. Spinner noise disappears, and your manual names stay pinned. This is a mod made of cooperating UI modules; this gallery package contains no layouts. It does not generate AI summaries or read conversation transcripts.

![Auto Titles](preview.png)

## What you see

Keep the user's window arrangement, theme, fonts, terminal contents and side panels. Replace the relevant title-rendering modules in a personal copy of their layout. The Main tab strip uses 29px-high tab chips, 12.8px text, 7px corners and a title truncated at 170px; its block is normally 42px high. Agent rows show the space and task, with children beneath their parent. Pane headers show a program icon and the useful task title. The picker and sub-agent preview repeat the same title policy.

Use Herdr's theme tokens (`chrome`, `raised`, `foreground`, `muted`, `border`, `working`, `blocked`, `done`) so light and dark themes work. The package also includes title-rendering replacements for the Terminals and Win98 styles; install only the layout references appropriate to the user's existing style. It does not ask the user to adopt a new window design.

## How it behaves

Automatic tabs choose the first useful task title from their panes, preferring agent panes in stable pane order. Changing split focus does not change the chosen task. Remove ANSI sequences, control characters and leading spinner glyphs. Reject bare program names, path/host prompts and transient words such as Thinking or Working; fall back to the program and folder. Keep Unicode and meaningful punctuation. A later task title replaces the earlier one automatically.

An explicit tab name always wins, including `Codex` or `1`. Right-click a tab and choose **Use automatic title** to clear that name; **Rename** pins a new one. Never reset all existing names during installation. New tabs launched through the mod's menus start unnamed.

Working agents retain an animated spinner; blocked agents show Needs you, or a compact exclamation mark; done agents show a dot; idle agents show a ring. Status affects the indicator, not the task wording. Tab selection, dragging, closing, pane controls, agent focus, grouping and pinning retain the bundled Herdr behavior. Click a child agent to open its terminal preview with the titled header. Below 160px sidebar width, the inherited compact agent list shows numbers instead of long labels.

## Built from

Herdr desktop 0.1.9 or newer, using its built-in JavaScript mod runtime. No Node runtime is required. All eleven `mods/auto-titles-*` folders are self-contained with their assets and local title helpers. Copy them together because menus, previews and icons reference one another. Do not overwrite any installed folder or built-in module.

In a personal copy of the current layout, change only matching mod references:

| Existing module | Replacement |
|---|---|
| tabs | auto-titles-tabs |
| pane | auto-titles-pane |
| agents | auto-titles-agents |
| workspace-chrome | auto-titles-workspace-chrome |
| terminals-bar | auto-titles-terminals-bar |
| terminals-pane | auto-titles-terminals-pane |
| win98 | auto-titles-win98 |

Keep block IDs, dimensions, props and unrelated modules. The remaining `auto-titles-menu`, `auto-titles-new-menu`, `auto-titles-picker` and `auto-titles-peek` modules are supporting overlays. Route the layout's `terminal_menu` to `auto-titles-menu`. If a global shortcut opens `picker`, add a layout-scoped equivalent opening `auto-titles-picker`; preserve custom shortcuts. Do not supply or replace a global keys file. Carry existing layout size/theme choices over using the documented layout commands; do not edit Herdr's `.state` files.

The title modules read `spaces()`, `panes()` and `terminals()`. They render through `gpui-kit` and call Herdr commands through `run()`. Names use `tab.rename`; selection uses `tab.select` and `pane.focus`; menus use `openOverlay`/`closeOverlay`. The tab strip (or Terminals bar or Win98 taskbar) publishes `auto-titles.snapshot` using `setShared` for machine-readable inspection. Supporting UI uses localStorage for panel choices, pins and status ages.

Read the files before installing. Title calculation makes no network/model calls and reads no transcripts. Inherited controls can run `git` to inspect a checkout, call `hc` for known machines, launch terminals/agents through Herdr, and, in Win98, run `sh` for process/program discovery and `open`/`xdg-open` for selected URLs. They use Herdr's shared state and stock assets outside their own folders. Retain the included Apache-2.0 and font/icon notices.

## Watch out

This folder is the mods-only gallery distribution. The repository's optional v1.1.0 native helper is a separate convenience installer: `install` still adds three preconfigured layouts and selects Main. `install --no-activate` adds those files without switching. Its install/update/uninstall receipts do not track a layout assembled manually from this recipe; do not mix ownership models. Existing releases and users remain supported.

For full CLI/API/MCP inspection with a manually assembled layout, give that layout a unique `auto-titles-` prefix and include a snapshot-producing tab module. The native helper's `list` and `rename` commands work with it; its installation status describes the helper receipt, not a manual installation. Native `enable`/`disable`/`uninstall` manage the helper installation only. Follow `agent-tools.json`, `llms-full.txt` and `SKILL.md` for exact supported operations. Never claim the helper owns manually copied files.

No task reported by a terminal means a program/folder fallback, not an invented summary. Old launcher-assigned names count as manual. Herdr's host-managed terminal-address previews still use the stock header. The UI modules are derived from Herdr 0.1.9; newer host UI features need to be reconciled when updating. Custom modules with different IDs or assumptions need adaptation, not blind replacement.

## Done when

- `herdr-app check` succeeds for every copied mod in a scratch home.
- The mod fits the user's existing layout and unrelated custom files remain unchanged.
- An unnamed agent tab follows a synthetic terminal title change and strips spinner noise.
- Split-focus changes do not rename that tab. Pinning a manual name survives later terminal updates; clearing it resumes automatic naming.
- Agent rows, pane headers, the picker and child previews use the same title policy; working/blocked/done/idle indicators still work.
- The native helper's `list` (MCP `herdr_titles_list`, HTTP `GET /v1/titles`) reports the rendered title and mode for a compatible active layout. `rename ID NAME` (MCP `herdr_tab_title_set`, HTTP `POST /v1/tab-title`) pins/resets the identified tab.
- Existing sessions keep running throughout installation and switching back restores the previous UI.

Copy-paste agent prompt:

> Read https://github.com/mustafaTokmak/herdr-auto-titles/tree/main/community/auto-titles, including RECIPE.md and every mod file. Add Auto Titles to a personal copy of my current Herdr desktop layout, keeping its arrangement, custom controls and manual tab names. Use the namespaced mods, preserve my original files and shortcuts, validate with herdr-app check, and do not restart Herdr or any terminal. Read the included agent tool reference before using the optional native helper; its installer and uninstall receipts apply only to its own installation. Verify rendered titles through its list/MCP/API interface after activation. Only clear manual names for tabs I identify.
