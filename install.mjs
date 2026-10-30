import { readFile, writeFile, readdir, mkdir, lstat, realpath, rename, unlink, rmdir, mkdtemp, cp, rm } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec = promisify(execFile);
export const root = dirname(fileURLToPath(import.meta.url));
export const metadata = JSON.parse(await readFile(join(root, 'plugin.json'), 'utf8'));
export const layouts = metadata.layouts;
const statePath = 'data/herdr-auto-titles/installation.json';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const exists = async path => { try { return await lstat(path); } catch (e) { if (e.code === 'ENOENT') return null; throw e; } };
export function appHome() { return process.env.HERDR_APP_HOME || join(process.env.XDG_CONFIG_HOME || join(homedir(), '.config'), 'herdr-app'); }
export async function runApp(args, json = true, env = process.env) {
  const { stdout } = await exec(process.env.HERDR_APP_BIN || 'herdr-app', args, { env, timeout: 15000, maxBuffer: 8 * 1024 * 1024 });
  if (!json) return stdout;
  const value = JSON.parse(stdout);
  if (value.error) throw new Error(typeof value.error === 'string' ? value.error : JSON.stringify(value.error));
  return value;
}
function runtime(overrides = {}) { return { home: appHome(), run: runApp, check: validateMods, ...overrides }; }
async function safePath(home, rel) {
  if (rel.split(/[\\/]/).some(s => s === '..' || !s) || rel.startsWith('/') || rel.includes('\\') || rel.includes(':')) throw new Error(`Unsafe managed path: ${rel}`);
  let path = home;
  for (const part of rel.split('/')) {
    path = join(path, part);
    if ((await exists(path))?.isSymbolicLink()) throw new Error(`Refusing symlink: ${path}`);
  }
  return path;
}
async function state(home) {
  const path = await safePath(home, statePath);
  if (!await exists(path)) return null;
  const value = JSON.parse(await readFile(path, 'utf8'));
  if (value.name !== metadata.name || !value.files || typeof value.files !== 'object' || Array.isArray(value.files)) throw new Error('Invalid installation receipt');
  for (const name of Object.keys(value.files)) {
    if (!/^(?:mods\/auto-titles-[a-z0-9-]+\/|layouts\/auto-titles-[a-z0-9-]+\.(?:json|svg)$)/.test(name)) throw new Error(`Unowned receipt path: ${name}`);
    await safePath(home, name);
  }
  return value;
}
async function walk(dir, prefix = '') {
  const files = new Map();
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const rel = prefix + e.name;
    if (e.isSymbolicLink()) throw new Error(`Payload symlink: ${rel}`);
    if (e.isDirectory()) for (const [path, bytes] of await walk(join(dir, e.name), rel + '/')) files.set(path, bytes);
    else if (!['gpui-kit.d.ts', 'jsconfig.json'].includes(e.name) && !e.name.endsWith('.error')) files.set(rel, await readFile(join(dir, e.name)));
  }
  return files;
}
async function atomic(path, bytes) {
  await mkdir(dirname(path), { recursive: true });
  const temp = path + '.' + randomUUID() + '.tmp';
  try { await writeFile(temp, bytes); await rename(temp, path); }
  finally { if (await exists(temp)) await unlink(temp); }
}
async function transaction(home, changes) {
  const before = new Map();
  for (const [rel] of changes) {
    const path = await safePath(home, rel);
    const info = await exists(path);
    if (info && !info.isFile()) throw new Error(`Not a regular file: ${path}`);
    before.set(rel, info ? await readFile(path) : null);
  }
  const applied = [];
  try {
    for (const [rel, bytes] of changes) {
      const path = await safePath(home, rel);
      applied.push(rel);
      if (bytes === null) { if (await exists(path)) await unlink(path); }
      else await atomic(path, bytes);
    }
  } catch (error) {
    for (const rel of applied.reverse()) {
      const path = join(home, rel), bytes = before.get(rel);
      if (bytes === null) { if (await exists(path)) await unlink(path); }
      else await atomic(path, bytes);
    }
    throw error;
  }
}
async function validateMods() {
  const version = await runApp(['--version'], false);
  const match = version.match(/(\d+)\.(\d+)\.(\d+)/);
  if (!match || (Number(match[1]) === 0 && Number(match[2]) === 1 && Number(match[3]) < 9) || (Number(match[1]) === 0 && Number(match[2]) < 1)) throw new Error('Herdr desktop 0.1.9 or newer is required');
  const dir = await mkdtemp(join(tmpdir(), 'herdr-auto-titles-check-'));
  try {
    await cp(join(root, 'payload'), dir, { recursive: true });
    await mkdir(join(dir, 'check-home'));
    for (const mod of await readdir(join(dir, 'mods'))) await runApp(['check', join(dir, 'mods', mod)], false, { ...process.env, HERDR_APP_HOME: join(dir, 'check-home') });
  } finally { await rm(dir, { recursive: true, force: true }); }
}
// Win98's fitLayout() writes its pixel-grid sizes into personal layout JSON.
// Accept only that correlated auto-fit transformation, never arbitrary edits.
export function isAutoFitLayout(actual, expected) {
  const find = (node, id) => node?.id === id ? node : (node?.children ?? []).map(n => find(n, id)).find(Boolean);
  const canonical = value => Array.isArray(value) ? value.map(canonical) : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])])) : value;
  try {
    const now = JSON.parse(actual), base = JSON.parse(expected);
    const k = find(now, 'taskbar').size.px / 28;
    if (!Number.isFinite(k) || k <= 0) return false;
    const want = n => Math.round(n * k * 1000) / 1000;
    const near = (a, b) => typeof a === 'number' && Math.abs(a-b) < 0.002;
    const pane = find(now, 'main').content.pane;
    if (!near(find(now, 'desktop').size.px, want(84)) || !near(pane.header.height, want(18)) || !near(pane.border.width, want(3)) || !near(pane.focus.width, want(3)) || !(near(now.font_size,16) || near(now.font_size,want(16)))) return false;
    for (const layout of [now, base]) {
      delete layout.font_size;
      delete find(layout, 'desktop').size.px;
      delete find(layout, 'taskbar').size.px;
      const p = find(layout, 'main').content.pane;
      delete p.header.height; delete p.border.width; delete p.focus.width;
    }
    return JSON.stringify(canonical(now)) === JSON.stringify(canonical(base));
  } catch { return false; }
}
async function verifyOwned(home, receipt) {
  for (const [rel, expected] of Object.entries(receipt?.files ?? {})) {
    const path = await safePath(home, rel), info = await exists(path);
    if (!info) continue;
    if (info.isFile()) {
      const bytes = await readFile(path);
      if (hash(bytes) === expected) continue;
      if (rel === 'layouts/auto-titles-win98.json') {
        let baseline = receipt.runtime_layouts?.[rel];
        if (!baseline) {
          const bundled = await readFile(join(root, 'payload', rel));
          if (hash(bundled) === expected) baseline = bundled.toString();
        }
        if (baseline && isAutoFitLayout(bytes.toString(), baseline)) continue;
      }
    }
    throw new Error(`Locally edited plugin file: ${path}. Save your edits elsewhere before updating or uninstalling.`);
  }
}
export async function status(overrides) {
  const rt = runtime(overrides), receipt = await state(rt.home);
  let app = null, error = null;
  try { app = await rt.run(['status']); } catch (e) { error = e.message; }
  return { installed: !!receipt, version: receipt?.version ?? null, home: rt.home, active: !!app?.layout?.startsWith('auto-titles-'), layout: app?.layout ?? null, app_error: error };
}
export async function enable(layout = 'main', overrides) {
  if (!layouts.includes(layout)) throw new Error(`layout must be one of ${layouts.join(', ')}`);
  const rt = runtime(overrides), receipt = await state(rt.home);
  if (!receipt) throw new Error('Install herdr-auto-titles first');
  const current = await rt.run(['status']);
  if (!current.layout?.startsWith('auto-titles-')) {
    receipt.previous_layout = current.layout || 'main';
    await transaction(rt.home, new Map([[statePath, Buffer.from(JSON.stringify(receipt, null, 2) + '\n')]]));
  }
  await rt.run(['do', 'layout.switch', JSON.stringify({ name: 'auto-titles-' + layout })]);
  return { active: true, layout: 'auto-titles-' + layout };
}
export async function disable(overrides) {
  const rt = runtime(overrides), receipt = await state(rt.home);
  const current = await rt.run(['status']);
  if (!current.layout?.startsWith('auto-titles-')) return { active: false, layout: current.layout };
  const target = receipt?.previous_layout || 'main';
  const layout = await exists(await safePath(rt.home, 'layouts/' + target + '.json')) ? target : 'main';
  await rt.run(['do', 'layout.switch', JSON.stringify({ name: layout })]);
  return { active: false, layout };
}
async function pickerKeys(home, old, removing = false) {
  const path = await safePath(home, 'keys.json');
  if (!await exists(path)) return { keys: null, bindings: old?.key_bindings ?? {} };
  const keys = JSON.parse(await readFile(path, 'utf8'));
  const bindings = removing ? old?.key_bindings ?? {} : structuredClone(old?.key_bindings ?? {});
  if (!removing) {
    // Only shadow shortcuts that still target the stock picker. Custom global
    // and existing layout-specific bindings remain the user's choice.
    for (const layout of layouts) {
      const section = 'layout:auto-titles-' + layout;
      for (const [key, action] of Object.entries(keys.global ?? {})) {
        if (action !== 'mod.open:picker' || keys[section]?.[key] !== undefined) continue;
        keys[section] ??= {}; bindings[section] ??= {};
        keys[section][key] = bindings[section][key] = 'mod.open:auto-titles-picker';
      }
    }
  } else {
    for (const [section, entries] of Object.entries(bindings)) {
      if (!section.startsWith('layout:auto-titles-')) throw new Error('Unowned key section in receipt');
      for (const [key, action] of Object.entries(entries)) if (keys[section]?.[key] === action) delete keys[section][key];
      if (keys[section] && !Object.keys(keys[section]).length) delete keys[section];
    }
  }
  return { keys, bindings };
}
export async function install(options = {}, overrides) {
  const rt = runtime(overrides);
  if (options.layout && !layouts.includes(options.layout)) throw new Error('Unknown layout');
  if (!await exists(join(rt.home, 'mods')) || !await exists(join(rt.home, 'layouts'))) throw new Error('Open the Herdr desktop app once before installing');
  rt.home = await realpath(rt.home);
  const old = await state(rt.home), files = await walk(join(root, 'payload'));
  await verifyOwned(rt.home, old);
  for (const [rel] of files) {
    const path = await safePath(rt.home, rel);
    if (await exists(path) && !Object.hasOwn(old?.files ?? {}, rel)) throw new Error(`Destination is not owned by this plugin: ${path}`);
  }
  await rt.check();
  const keyState = await pickerKeys(rt.home, old);
  const receipt = { runtime_layouts: { 'layouts/auto-titles-win98.json': files.get('layouts/auto-titles-win98.json').toString() }, key_bindings: keyState.bindings, name: metadata.name, version: metadata.version, previous_layout: old?.previous_layout || 'main', files: Object.fromEntries([...files].map(([rel, bytes]) => [rel, hash(bytes)])) };
  const changes = new Map(files);
  if (keyState.keys && Object.keys(keyState.bindings).length) changes.set('keys.json', Buffer.from(JSON.stringify(keyState.keys, null, 2) + '\n'));
  for (const rel of Object.keys(old?.files ?? {})) if (!files.has(rel)) changes.set(rel, null);
  changes.set(statePath, Buffer.from(JSON.stringify(receipt, null, 2) + '\n'));
  await transaction(rt.home, changes);
  let activation = null;
  if (options.activate !== false) {
    try { activation = await enable(options.layout || 'main', rt); }
    catch (e) { activation = { active: false, error: e.message, hint: 'Files installed. Start Herdr, then run enable.' }; }
  }
  return { installed: true, version: metadata.version, home: rt.home, updated: !!old, files: files.size, activation };
}
export async function uninstall(overrides) {
  const rt = runtime(overrides), receipt = await state(rt.home);
  if (!receipt) return { installed: false, removed: 0 };
  await verifyOwned(rt.home, receipt);
  // If Herdr is running, leave the plugin layout before removing its files.
  // Offline removal is safe: stock main is the app's missing-layout fallback.
  const current = await status(rt);
  if (current.active) await disable(rt);
  const keyState = await pickerKeys(rt.home, receipt, true);
  const changes = new Map(Object.keys(receipt.files).map(rel => [rel, null]));
  if (keyState.keys && Object.keys(keyState.bindings).length) changes.set('keys.json', Buffer.from(JSON.stringify(keyState.keys, null, 2) + '\n'));
  changes.set(statePath, null);
  await transaction(rt.home, changes);
  const parents = new Set();
  for (const rel of changes.keys()) {
    let dir = dirname(rel);
    while (dir !== '.' && !['mods', 'layouts', 'data'].includes(dir)) { parents.add(dir); dir = dirname(dir); }
  }
  for (const dir of [...parents].sort((a,b) => b.length-a.length)) {
    try { await rmdir(join(rt.home, dir)); } catch (e) { if (!['ENOTEMPTY','ENOENT','EEXIST'].includes(e.code)) throw e; }
  }
  return { installed: false, removed: Object.keys(receipt.files).length };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const command = process.argv[2] || 'install';
  const options = process.argv.includes('--no-activate') ? { activate: false } : {};
  const task = command === 'install' ? install(options) : command === 'uninstall' ? uninstall() : command === 'status' ? status() : Promise.reject(new Error('usage: node install.mjs [install|uninstall|status] [--no-activate]'));
  task.then(v => console.log(JSON.stringify(v))).catch(e => { console.error(JSON.stringify({ error: e.message })); process.exitCode = 1; });
}
