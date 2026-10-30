import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm, symlink, readdir, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { install, uninstall, enable, disable, status, metadata } from '../install.mjs';
import { tools, validate } from '../agent.mjs';
async function fixture() {
  const home=await mkdtemp(join(tmpdir(),'herdr-titles-install-'));
  await mkdir(join(home,'mods','tabs'),{recursive:true});
  await mkdir(join(home,'layouts'));
  const originals={'mods/tabs/main.js':'stock tabs','layouts/main.json':'{}','layouts/personal.json':'{}','keys.json':'{"global":{"ctrl-p":"custom.action"}}','settings.json':'{"zoom":1.2}'};
  for(const [file,text] of Object.entries(originals)) await writeFile(join(home,file),text);
  let layout='personal', checks=0;
  const rt={home,check:async()=>{checks++;},run:async(args)=>{
    if(args[0]==='status') return {layout};
    assert.deepEqual(args.slice(0,2),['do','layout.switch']);layout=JSON.parse(args[2]).name;return {ok:true};
  }};
  return {home,rt,checks:()=>checks,layout:()=>layout,clean:()=>rm(home,{recursive:true,force:true}),unchanged:async()=>{for(const [file,text] of Object.entries(originals)) assert.equal(await readFile(join(home,file),'utf8'),text,file);}};
}
test('install, idempotent update, activate, disable and uninstall preserve stock files and user layout',async()=>{
 const f=await fixture();try{
  const a=await install({},f.rt); assert.equal(a.installed,true);assert.equal(a.updated,false);assert.equal(f.layout(),'auto-titles-main');assert.equal(f.checks(),1);
  assert.equal((await status(f.rt)).active,true);await f.unchanged();
  const b=await install({},f.rt);assert.equal(b.updated,true);await f.unchanged();
  await enable('terminals',f.rt);assert.equal(f.layout(),'auto-titles-terminals');
  await disable(f.rt);assert.equal(f.layout(),'personal');
  await enable('win98',f.rt);assert.equal(f.layout(),'auto-titles-win98');
  const result=await uninstall(f.rt);assert.equal(result.installed,false);assert.ok(result.removed>30);assert.equal(f.layout(),'personal');
  await f.unchanged();assert.equal((await status(f.rt)).installed,false);
  assert.deepEqual(await readdir(join(f.home,'mods')),['tabs']);
 }finally{await f.clean();}
});
test('preflight refuses unrelated destination and never changes earlier files',async()=>{
 const f=await fixture();try{
  await mkdir(join(f.home,'mods','auto-titles-tabs'));
  await writeFile(join(f.home,'mods','auto-titles-tabs','main.js'),'my independent mod');
  await assert.rejects(install({},f.rt),/not owned/);assert.equal(f.checks(),0);await f.unchanged();
  await assert.rejects(access(join(f.home,'mods','auto-titles-agents')));
 }finally{await f.clean();}
});
test('locally edited plugin files block update and uninstall',async()=>{
 const f=await fixture();try{
  await install({activate:false},f.rt);
  const file=join(f.home,'mods','auto-titles-tabs','main.js');await writeFile(file,'user edit');
  await assert.rejects(install({},f.rt),/Locally edited/);await assert.rejects(uninstall(f.rt),/Locally edited/);
  assert.equal(await readFile(file,'utf8'),'user edit');assert.equal((await status(f.rt)).installed,true);await f.unchanged();
 }finally{await f.clean();}
});
test('failed mod validation leaves no installed files or receipt',async()=>{
 const f=await fixture();try{
  await assert.rejects(install({}, {...f.rt,check:async()=>{throw Error('invalid mod');}}),/invalid mod/);
  assert.deepEqual(await readdir(join(f.home,'mods')),['tabs']);assert.equal((await status(f.rt)).installed,false);await f.unchanged();
 }finally{await f.clean();}
});
test('offline install is explicit about activation and can be uninstalled',async()=>{
 const f=await fixture();try{
  const rt={...f.rt,run:async()=>{throw Error('window closed');}};
  const result=await install({},rt);assert.equal(result.activation.active,false);assert.match(result.activation.error,/window closed/);
  await uninstall(rt);assert.equal((await status(rt)).installed,false);await f.unchanged();
 }finally{await f.clean();}
});
test('uninstall preserves unowned generated or user files inside the plugin folder',async()=>{
 const f=await fixture();try{
  await install({},f.rt);await writeFile(join(f.home,'mods','auto-titles-tabs','user-notes.txt'),'keep');
  await uninstall(f.rt);assert.equal(await readFile(join(f.home,'mods','auto-titles-tabs','user-notes.txt'),'utf8'),'keep');
 }finally{await f.clean();}
});
test('symlinked destinations cannot escape the chosen home',async t=>{
 const f=await fixture(); const outside=await mkdtemp(join(tmpdir(),'herdr-outside-'));
 try{
  try{await symlink(outside,join(f.home,'mods','auto-titles-tabs'),'dir');}catch(e){if(e.code==='EPERM'){t.skip('symlink privilege unavailable');return;}throw e;}
  await assert.rejects(install({},f.rt),/symlink/);assert.deepEqual(await readdir(outside),[]);await f.unchanged();
 }finally{await f.clean();await rm(outside,{recursive:true,force:true});}
});
test('tampered receipt paths cannot delete outside plugin namespaces',async()=>{
 const f=await fixture();try{
  await install({},f.rt);const file=join(f.home,'data','herdr-auto-titles','installation.json');const receipt=JSON.parse(await readFile(file));
  receipt.files['settings.json']='fake';await writeFile(file,JSON.stringify(receipt));
  await assert.rejects(uninstall(f.rt),/Unowned receipt path/);await f.unchanged();
 }finally{await f.clean();}
});
test('all lifecycle actions have validated API, MCP and CLI interfaces',()=>{
 for(const cli of ['install','uninstall','status','enable','disable','list','rename']){
  const tool=tools.find(t=>t.cli===cli);assert.ok(tool?.name && tool.http?.path && tool.uiActions.length,cli);
 }
 const tool=tools.find(t=>t.cli==='enable');
 assert.throws(()=>validate(tool,{layout:'../other'}));
 for(const layout of metadata.layouts) validate(tool,{layout});
});
test('picker shortcuts are scoped to plugin layouts and uninstall preserves later edits',async()=>{
 const f=await fixture();try{
  const file=join(f.home,'keys.json');await writeFile(file,JSON.stringify({global:{'cmd-p':'mod.open:picker'},'layout:auto-titles-win98':{'cmd-p':'my.action'}}));
  await install({},f.rt);const keys=JSON.parse(await readFile(file));
  assert.equal(keys.global['cmd-p'],'mod.open:picker');
  assert.equal(keys['layout:auto-titles-main']['cmd-p'],'mod.open:auto-titles-picker');
  assert.equal(keys['layout:auto-titles-win98']['cmd-p'],'my.action');
  keys['layout:auto-titles-main']['cmd-p']='my.new.action';await writeFile(file,JSON.stringify(keys));
  await uninstall(f.rt);const after=JSON.parse(await readFile(file));
  assert.equal(after['layout:auto-titles-main']['cmd-p'],'my.new.action');
  assert.equal(after['layout:auto-titles-terminals'],undefined);
  assert.equal(after['layout:auto-titles-win98']['cmd-p'],'my.action');
 }finally{await f.clean();}
});
test('normal Win98 pixel-grid scaling survives update/removal but structural edits are protected',async()=>{
 const f=await fixture();try{
  await install({activate:false},f.rt);
  const path=join(f.home,'layouts','auto-titles-win98.json');
  const layout=JSON.parse(await readFile(path));
  layout.font_size=16;
  layout.children[0].children[0].size.px=84;
  layout.children[1].size.px=28;
  const pane=layout.children[0].children[1].content.pane;
  pane.header.height=18;pane.border.width=3;pane.focus.width=3;
  await writeFile(path,JSON.stringify(layout));
  await install({activate:false},f.rt);
  layout.background='#123456';await writeFile(path,JSON.stringify(layout));
  await assert.rejects(uninstall(f.rt),/Locally edited/);
  delete layout.background;layout.background='#008080';await writeFile(path,JSON.stringify(layout));
  await uninstall(f.rt);await f.unchanged();
 }finally{await f.clean();}
});
