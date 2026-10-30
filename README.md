# Herdr Auto Titles

Automatic task titles for the **Herdr desktop app**, packaged as an independent mod bundle. No custom Herdr build or upstream PR is required.

- Task titles across tabs, pane headers, agent rows, sub-agent previews and the tab picker.
- Spinner noise removed; program/folder labels when an agent does not report a task title.
- Manual names stay pinned. **Use automatic title** restores live naming.
- Three layouts: `auto-titles-main`, `auto-titles-terminals`, `auto-titles-win98`.
- Matching CLI, JSON API, MCP tools and generated agent documentation.

This is a desktop mod bundle, **not a plugin for the old Herdr TUI**. It does not generate AI summaries or read transcripts.

![Automatic titles in stock Herdr, using a synthetic agent session](preview.png)

## Install

Requires Herdr desktop **0.1.9+** and Node **22+**. Open Herdr once first.

```sh
git clone https://github.com/mustafaTokmak/herdr-auto-titles
cd herdr-auto-titles
node agent.mjs install
```

The installer checks all 11 mods, installs files under `mods/auto-titles-*` and `layouts/auto-titles-*`, and selects `auto-titles-main`. Your terminals and sessions stay running. It never overwrites built-in mods, layouts, or settings. Standard picker shortcuts are redirected only inside the plugin layouts; custom shortcuts are preserved.

Set `HERDR_APP_BIN` to the Herdr executable if it is not on PATH. `HERDR_APP_HOME` selects a different app home. These variables apply to all commands and agent tools.

To install without switching layouts:

```sh
node agent.mjs install --no-activate
```

Then select an `auto-titles-*` layout in Herdr's layout picker, or:

```sh
node agent.mjs enable main       # or terminals / win98
node agent.mjs status
```

## Update or remove

Keep the checkout for updates and agent tools.

```sh
git pull --ff-only
node agent.mjs install           # update; validates ownership before writing
node agent.mjs disable           # return to the layout used before enabling
node agent.mjs uninstall         # remove only owned files and scoped shortcuts
```

Updates and removal refuse locally edited plugin files rather than discarding changes. Save edits in a separate clone first. Files you add inside a plugin folder are retained. An installation receipt lives at `data/herdr-auto-titles/installation.json`. If Herdr is closed, installation succeeds and reports that activation is pending; open Herdr and run `enable`. Offline removal leaves Herdr to fall back to its stock layout on the next start.

## Names and existing tabs

The plugin follows useful titles reported by programs. Agents without a task title get a program/folder fallback. Focusing another split does not change which pane supplies the tab title.

A manual name always wins—even `Codex`, `1`, or an old program name assigned by Herdr's launcher. Right-click that tab and choose **Use automatic title** to opt it in. New tabs created through the plugin's agent menus start with automatic names.

Custom layouts are not rewritten. The bundle uses its own copies of Herdr 0.1.9's UI mods, so it receives UI updates when this project updates. Sub-agent previews opened from its agent list use the new titles; terminal-address links handled by the host still use the stock preview. Global shortcuts and other stock layouts retain their normal behavior.

## For agents

All plugin workflows have equivalent JSON API endpoints and MCP tools, including install, update, activation and uninstall. The source of truth is [agent-tools.json](agent-tools.json); [llms.txt](llms.txt), [llms-full.txt](llms-full.txt) and [SKILL.md](SKILL.md) are generated from it.

```sh
node agent.mjs                   # MCP stdio server
node agent.mjs --http 0          # loopback API; prints the URL
node agent.mjs list              # rendered titles and automatic/manual modes
node agent.mjs rename TAB_ID "Fix login"  # pin a name
node agent.mjs rename TAB_ID ""           # automatic again
```

Configure an MCP client with `command: "node"` and `args: ["/absolute/path/to/herdr-auto-titles/agent.mjs"]`. Keep stdout dedicated to MCP when running without arguments. The HTTP interface is for local agent clients and rejects browser origins and non-loopback hosts.

Copy-paste prompt:

> Install the independent Herdr Auto Titles mod from github.com/mustafaTokmak/herdr-auto-titles. Preserve my names and custom files, and do not restart any app or terminal. Verify the active layout with herdr_auto_titles_status and inspect titles with herdr_titles_list. Only rename or reset the tabs I identify.

## Development

```sh
npm test
npm run docs
npm run docs:check
```

Tests cover titles, all agent transports, lifecycle ownership, local-edit protection, scoped shortcuts, symlink escapes, cleanup, generated documentation and API/MCP coverage. The mod files are self-contained so they can be cloned independently.

## License and upstream

Apache-2.0. Based on Herdr 0.1.9's mods and layouts with the changes described in [NOTICE](NOTICE). Bundled fonts and icons retain their upstream notices. This is an independent project, not an official Herdr release.
