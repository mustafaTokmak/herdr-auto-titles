// Load every bundled mod with the unmodified, checksum-pinned Herdr 0.1.9.
import { mkdtemp, writeFile, readdir, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
const exec=promisify(execFile);
const targets={
 'linux-x64':['linux-x86_64','da6f7b40f6c416c5b4def8dbcf8b76677d02be4f4b45d06a4d658766850e0b45'],
 'darwin-arm64':['macos-aarch64','7cd783d6cd98e050d7cb43bc43d7facc387d300b5b5e93c7376596b5d8727231'],
 'win32-x64':['windows-x86_64','d8878e51114349af455405255906998a967df73ac1cc6bf00252a52af29dea62'],
};
const target=targets[process.platform+'-'+process.arch];
if(!target) throw Error('Unsupported smoke-test platform: '+process.platform+'-'+process.arch);
const dir=await mkdtemp(join(tmpdir(),'herdr-stock-mods-'));
async function binaries(folder){
 const found=[];
 for(const item of await readdir(folder,{withFileTypes:true})){
  const path=join(folder,item.name);
  if(item.isDirectory()) found.push(...await binaries(path));
  else if(['herdr-app','Herdr','herdr-app.exe','Herdr.exe'].includes(item.name)) found.push(path);
 }
 return found;
}
try{
 const response=await fetch(`https://releases.herdr.dev/app/0.1.9/herdr-app-${target[0]}.tar.gz`);
 if(!response.ok) throw Error('Download failed: '+response.status);
 const bytes=Buffer.from(await response.arrayBuffer());
 if(createHash('sha256').update(bytes).digest('hex')!==target[1]) throw Error('Herdr archive checksum mismatch');
 const archive=join(dir,'herdr.tar.gz');await writeFile(archive,bytes);
 await exec('tar',['-xzf',archive,'-C',dir]);
 const candidates=await binaries(dir);if(candidates.length!==1) throw Error('Expected one app binary: '+JSON.stringify(candidates));
 const home=join(dir,'home');await mkdir(join(home,'mods'),{recursive:true});await mkdir(join(home,'layouts'));
 const binary=process.env.HERDR_AUTO_TITLES_BIN || fileURLToPath(new URL('../target/release/herdr-auto-titles'+(process.platform==='win32'?'.exe':''),import.meta.url));
 const {stdout}=await exec(binary,['install','--no-activate'],{
  env:{...process.env,HERDR_APP_BIN:candidates[0],HERDR_APP_HOME:home},timeout:180000,maxBuffer:8*1024*1024,
 });
 console.log(stdout.trim());
 console.log('All 11 namespaced mods passed stock Herdr 0.1.9 checks on '+target[0]);
}finally{await rm(dir,{recursive:true,force:true});}
