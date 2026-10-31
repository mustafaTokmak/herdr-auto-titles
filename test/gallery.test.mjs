import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, cp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
const exec = promisify(execFile);
const root = new URL('../', import.meta.url);
test('published mod package rejects stale code, missing recipe and bundled layouts', async () => {
  await exec(process.execPath, [fileURLToPath(new URL('scripts/gallery.mjs', root)), '--check']);
  const dir = await mkdtemp(join(tmpdir(), 'herdr-gallery-'));
  try {
    for (const item of ['scripts', 'payload', 'community', 'licenses', 'preview.png', 'LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES', 'agent-tools.json', 'llms.txt', 'llms-full.txt', 'SKILL.md'])
      await cp(new URL(item, root), join(dir, item), { recursive: true });
    const script = join(dir, 'scripts/gallery.mjs');
    const check = () => exec(process.execPath, [script, '--check']);
    await check();
    const layout = join(dir, 'community/auto-titles/layouts');
    await mkdir(layout);
    await writeFile(join(layout, 'unwanted.json'), '{}');
    await assert.rejects(check(), /Gallery file list differs/);
    await rm(layout, { recursive: true });
    await writeFile(join(dir, 'community/auto-titles/mods/auto-titles-tabs/titles.js'), '// stale');
    await assert.rejects(check(), /Stale gallery file/);
    await exec(process.execPath, [script]);
    await check();
    await rm(join(dir, 'community/auto-titles/RECIPE.md'));
    await assert.rejects(check(), /RECIPE.md/);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
