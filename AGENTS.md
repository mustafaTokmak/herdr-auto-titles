# Herdr Auto Titles

Independent desktop mod bundle. Do not reopen upstream Herdr PR #101 or require a custom Herdr build.

Keep payload files namespaced under auto-titles-*. Never overwrite built-in app mods, layouts, settings or unrelated keys. Installation/update/uninstall must preserve local edits and unrelated files. Test with a scratch HERDR_APP_HOME and isolated core; never stop or restart the user's app or core.

Every plugin workflow needs matching JSON API, MCP and CLI interfaces in agent-tools.json. Generate llms.txt, llms-full.txt and SKILL.md with npm run docs. Run npm test and npm run docs:check before publishing. Validate changed mods with the stock herdr-app check command in a scratch copy so generated type files do not enter the payload.

The payload includes modified Apache-2.0 Herdr UI code; retain NOTICE and bundled font/icon licenses. Title logic is copied per mod to keep clones independent; tests check those copies.
