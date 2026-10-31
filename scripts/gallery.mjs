// The gallery package contains only mods; the optional native installer retains
// its v1 layouts for compatibility. Generate copies from the tested payload.
import { readdir, readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const destination = join(root, 'community/auto-titles');
const check = process.argv.includes('--check');
async function files(dir) {
  const result = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isSymbolicLink()) throw Error(`Symlink in gallery input: ${path}`);
    if (entry.isDirectory()) result.push(...await files(path));
    else result.push(path);
  }
  return result.sort();
}
const recipe = await readFile(join(destination, 'RECIPE.md'), 'utf8');
for (const heading of ['# Auto Titles', '## What you see', '## How it behaves', '## Built from', '## Watch out', '## Done when'])
  if (!recipe.includes(heading)) throw Error(`Gallery recipe is missing ${heading}`);
if (!recipe.includes('](preview.png)')) throw Error('Gallery recipe must link its preview');
const source = join(root, 'payload/mods');
const expected = new Map();
for (const file of await files(source)) {
  if (/^(gpui-kit\.d\.ts|jsconfig\.json)$/.test(file.split(/[\\/]/).at(-1)) || file.endsWith('.error')) continue;
  expected.set(join('mods', relative(source, file)), await readFile(file));
}
for (const name of ['preview.png', 'LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES', 'agent-tools.json', 'llms.txt', 'llms-full.txt', 'SKILL.md'])
  expected.set(name, await readFile(join(root, name)));
for (const file of await files(join(root, 'licenses')))
  expected.set(join('licenses', relative(join(root, 'licenses'), file)), await readFile(file));
if (check) {
  const actual = (await files(destination)).map(p => relative(destination, p)).filter(p => p !== 'RECIPE.md');
  if (JSON.stringify(actual.sort()) !== JSON.stringify([...expected.keys()].sort())) throw Error('Gallery file list differs; run npm run gallery');
  for (const [name, bytes] of expected) {
    const current = await readFile(join(destination, name));
    // Git may check text out with CRLF on Windows; binary assets stay exact.
    const text = /\.(?:js|ts|json|md|txt|svg)$/.test(name) || ['LICENSE', 'NOTICE', 'THIRD-PARTY-NOTICES'].includes(name);
    if (text ? current.toString().replace(/\r\n/g, '\n') !== bytes.toString().replace(/\r\n/g, '\n') : !current.equals(bytes))
      throw Error(`Stale gallery file: ${name}; run npm run gallery`);
  }
} else {
  await rm(join(destination, 'mods'), { recursive: true, force: true });
  for (const [name, bytes] of expected) {
    const path = join(destination, name);
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, bytes);
  }
}
console.log(`Auto Titles mod package: ${expected.size} files ${check ? 'verified' : 'generated'}`);
