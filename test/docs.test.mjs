import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile,mkdtemp,mkdir,writeFile,rm,cp} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';
const exec=promisify(execFile), root=fileURLToPath(new URL('../',import.meta.url));
test('agent docs are current, tolerate CRLF, and reject drift',async()=>{
 await exec(process.execPath,[join(root,'scripts/docs.mjs'),'--check']);
 const dir=await mkdtemp(join(tmpdir(),'herdr-title-docs-'));
 try{
  await mkdir(join(dir,'scripts'));
  for(const file of ['scripts/docs.mjs','agent.mjs','agent-tools.json','install.mjs','plugin.json','llms.txt','llms-full.txt','SKILL.md']){
   const text=await readFile(join(root,file),'utf8');await writeFile(join(dir,file),text.replace(/\r?\n/g,'\r\n'));
  }
  await exec(process.execPath,[join(dir,'scripts/docs.mjs'),'--check']);
  await writeFile(join(dir,'llms.txt'),'outdated');
  await assert.rejects(exec(process.execPath,[join(dir,'scripts/docs.mjs'),'--check']));
 }finally{await rm(dir,{recursive:true,force:true});}
});
