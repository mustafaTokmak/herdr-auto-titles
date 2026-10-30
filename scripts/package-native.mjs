import { cp, mkdir, rm, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec=promisify(execFile);
const targets={'linux-x64':'linux-x86_64','darwin-arm64':'macos-aarch64','win32-x64':'windows-x86_64'};
const target=targets[process.platform+'-'+process.arch];if(!target)throw Error('Unsupported packaging target');
const {version}=JSON.parse(await readFile(new URL('../plugin.json',import.meta.url),'utf8'));
const dist=new URL('../dist/',import.meta.url), stage=new URL('stage/',dist);
await rm(stage,{recursive:true,force:true});await mkdir(stage,{recursive:true});
const binary='herdr-auto-titles'+(process.platform==='win32'?'.exe':'');
await cp(new URL('../target/release/'+binary,import.meta.url),new URL(binary,stage));
for(const file of ['README.md','LICENSE','NOTICE','THIRD-PARTY-NOTICES','SKILL.md','llms.txt','llms-full.txt','agent-tools.json','install.sh','licenses'])
 await cp(new URL('../'+file,import.meta.url),new URL(file,stage),{recursive:true});
const fonts='payload/mods/auto-titles-win98/fonts/';await mkdir(new URL(fonts,stage),{recursive:true});
for(const file of ['LICENSE-MSSansSerif.txt','LICENSE-PxPlus.txt']) await cp(new URL('../'+fonts+file,import.meta.url),new URL(fonts+file,stage));
// Preserve executable permissions on POSIX; tar is also available on Windows.
const {fileURLToPath}=await import('node:url');
const name=`herdr-auto-titles-${version}-${target}.tar.gz`, archive=new URL(name,dist);
await exec('tar',['-czf',fileURLToPath(archive),'-C',fileURLToPath(stage),'.']);
const digest=createHash('sha256').update(await readFile(archive)).digest('hex');
await writeFile(new URL(name+'.sha256',dist),`${digest}  ${name}\n`);
await rm(stage,{recursive:true,force:true});console.log(fileURLToPath(archive));
