// SPDX-License-Identifier: MIT
import type { Client, Context, ConfigWorkspaceDocument, ConfigWorkspaceBackupDocument } from '../../../../packages/plugin-web-sdk/src/index';
import { jsonFieldChanges, restoreJsonFields } from './json-recovery.ts';
import { lineChanges } from './text-recovery.ts';

type Workspace = Client['configWorkspace'];
export async function restoreSelection(client:Workspace,gameId:string,current:ConfigWorkspaceDocument,saved:ConfigWorkspaceBackupDocument,selectedPaths:string[]|null){
  if(selectedPaths?.length){
    const changes=jsonFieldChanges(current.text,saved.text);
    if(!changes)throw new Error('JSON_RECOVERY_UNSUPPORTED');
    return client.write(gameId,current.id,current.revision,restoreJsonFields(current.text,saved.text,selectedPaths));
  }
  return client.restore(gameId,current.id,saved.id,current.revision);
}
type Copy = {
  chooseFile:string; chooseBackup:string; config:string; current:string; saved:string; field:string; restoreField:string; noFiles:string; noBackups:string;
  restoreFields:string; restoreFile:string; undo:string;
  loading:string; changed:string; failed:string; cancelled:string; removed:string; added:string;
  unsupported:string; missing:string;
};
const copy:Record<'en'|'zh-CN',Copy>={
  en:{chooseFile:'Choose a config',chooseBackup:'Choose a saved version',config:'Config',current:'Current version',saved:'Saved version',field:'Field',restoreField:'Restore field',restoreFields:'Restore selected fields',restoreFile:'Restore this version',undo:'Undo last restore',noFiles:'No configs available.',noBackups:'No saved versions for this config.',loading:'Loading…',changed:'This config changed outside Config Doctor. Refresh before restoring.',failed:'Could not load or restore this version.',cancelled:'Restore cancelled.',removed:'Removed',added:'Added',unsupported:'This format can only be restored as a whole file.',missing:'Missing'},
  'zh-CN':{chooseFile:'选择配置',chooseBackup:'选择保存版本',config:'配置',current:'当前版本',saved:'保存版本',field:'字段',restoreField:'恢复字段',restoreFields:'恢复所选字段',restoreFile:'恢复此版本',undo:'撤销上次恢复',noFiles:'没有可用配置。',noBackups:'此配置没有保存版本。',loading:'正在加载…',changed:'配置已在其他位置更改。请刷新后再恢复。',failed:'无法加载或恢复此版本。',cancelled:'已取消恢复。',removed:'已删除',added:'已新增',unsupported:'此格式仅支持整文件恢复。',missing:'缺失'},
};
const elem=<K extends keyof HTMLElementTagNameMap>(tag:K,value='',className=''):HTMLElementTagNameMap[K]=>{const n=document.createElement(tag);n.textContent=value;n.className=className;return n;};
function option(select:HTMLSelectElement,value:string,label:string){const item=new Option(label,value);select.add(item);}

export function mountRecovery(client:Workspace, context:Context, getGame:()=>string, host:HTMLElement) {
  const t=copy[context.locale==='zh-CN'?'zh-CN':'en'];
  const controls=elem('div','','recovery-controls');
  const fileLabel=elem('label','','recovery-choice');fileLabel.append(elem('span',t.config));
  const fileSelect=elem('select','');fileSelect.setAttribute('aria-label',t.chooseFile);fileLabel.append(fileSelect);
  const backupLabel=elem('label','','recovery-choice');backupLabel.append(elem('span',t.saved));
  const backupSelect=elem('select','');backupSelect.setAttribute('aria-label',t.chooseBackup);backupLabel.append(backupSelect);
  controls.append(fileLabel,backupLabel);
  const status=elem('p','','recovery-status');status.setAttribute('role','status');
  const diff=elem('div','','recovery-diff');
  const actions=elem('div','','actions recovery-actions');
  const restoreFields=elem('button',t.restoreFields);restoreFields.type='button';restoreFields.className='primary';
  const restoreFile=elem('button',t.restoreFile);restoreFile.type='button';restoreFile.className='primary';
  const undo=elem('button',t.undo);undo.type='button';actions.append(restoreFields,restoreFile,undo);
  host.replaceChildren(controls,status,diff,actions);
  let files:Awaited<ReturnType<Workspace['list']>>=[];
  let current:Awaited<ReturnType<Workspace['read']>>|null=null;
  let saved:Awaited<ReturnType<Workspace['readBackup']>>|null=null;
  let fields:ReturnType<typeof jsonFieldChanges>=null;
  let busy=false;let undoConfigId:string|null=null;let operation=0;let comparison=0;let disposed=false;
  const fail=(message:string)=>{status.textContent=message;status.dataset.error='true';};
  const clearStatus=()=>{status.textContent='';delete status.dataset.error;};
  const updateActions=()=>{
    const selected=[...diff.querySelectorAll<HTMLInputElement>('input[type=checkbox]:checked')].map(input=>input.value);
    restoreFields.hidden=!fields?.length;restoreFields.disabled=busy||!current||!saved||!selected.length;
    restoreFile.hidden=!!fields?.length;restoreFile.disabled=busy||!current||!saved;
    undo.hidden=!undoConfigId||current?.id!==undoConfigId||!current?.backups.length;undo.disabled=busy||!current?.backups.length;
  };
  const renderDiff=()=>{
    diff.replaceChildren();fields=current&&saved?jsonFieldChanges(current.text,saved.text):null;
    if(!current||!saved){updateActions();return;}
    if(fields?.length){
      const columns=elem('div','','recovery-columns');columns.append(elem('span',t.field),elem('span',t.saved),elem('span',t.current));diff.append(columns);
      for(const change of fields){
        const row=elem('label','','recovery-field');const check=elem('input','');check.type='checkbox';check.value=change.path;check.setAttribute('aria-label',`${t.restoreField}: ${change.path}`);
        const field=elem('span',change.path,'recovery-field-name');
        const from=elem('span',change.beforeExists?JSON.stringify(change.before):t.missing,'recovery-value recovery-saved');from.dataset.state=change.beforeExists?(change.currentExists?'changed':'removed'):'missing';from.dataset.label=t.saved;
        const to=elem('span',change.currentExists?JSON.stringify(change.current):t.missing,'recovery-value recovery-current');to.dataset.state=change.currentExists?(change.beforeExists?'changed':'added'):'missing';to.dataset.label=t.current;
        row.append(check,field,from,to);diff.append(row);check.onchange=updateActions;
      }
    } else {
      diff.append(elem('p',t.unsupported,'muted'));
      const pair=elem('div','','recovery-pair');
      for(const [label,value] of [[t.saved,saved.text],[t.current,current.text]]){const section=elem('section');section.append(elem('h3',label));const pre=elem('pre',value);pre.tabIndex=0;section.append(pre);pair.append(section);}
      diff.append(pair);
      const changes=lineChanges(saved.text,current.text);
      if(changes?.length){
        const list=elem('ul','','recovery-lines');
        for(const change of changes){const row=elem('li',`${change.kind==='added'?t.added:t.removed} · ${change.line}: ${change.text}`);row.dataset.kind=change.kind;list.append(row);}
        diff.append(list);
      }
    }
    updateActions();
  };
  const loadComparison=async()=>{
    const epoch=++comparison;
    clearStatus();current=null;saved=null;diff.replaceChildren();updateActions();
    const configId=fileSelect.value,historyId=backupSelect.value,gameId=getGame();
    if(!configId||!historyId||!gameId)return;
    busy=true;status.textContent=t.loading;updateActions();
    try{const [nextCurrent,nextSaved]=await Promise.all([client.read(gameId,configId),client.readBackup(gameId,configId,historyId)]);if(disposed||epoch!==comparison||fileSelect.value!==configId||backupSelect.value!==historyId||getGame()!==gameId)return;current=nextCurrent;saved=nextSaved;renderDiff();}
    catch{if(!disposed&&epoch===comparison)fail(t.failed);}
    finally{if(!disposed&&epoch===comparison){busy=false;if(!status.dataset.error)clearStatus();updateActions();}}
  };
  const loadFiles=async(target?:{configId:string;backupId:string}|null)=>{
    const epoch=++operation;
    busy=true;clearStatus();status.textContent=t.loading;updateActions();
    try{
      const values=await client.list(getGame());if(disposed||epoch!==operation)return;files=values;fileSelect.replaceChildren();
      if(!files.length)option(fileSelect,'',t.noFiles);
      for(const file of files)option(fileSelect,file.id,file.name);
      fileSelect.disabled=!files.length;
      const requested=target&&files.some(file=>file.id===target.configId)?target.configId:'';
      if(requested)fileSelect.value=requested;
      await loadBackups(target?.backupId);
      if(!files.length)status.textContent=t.noFiles;
    }catch{if(!disposed&&epoch===operation)fail(t.failed);}finally{if(!disposed&&epoch===operation){busy=false;if(!status.dataset.error&&status.textContent===t.loading)clearStatus();updateActions();}}
  };
  const loadBackups=async(preselect?:string)=>{
    const epoch=++operation;
    const fileId=fileSelect.value;backupSelect.replaceChildren();saved=null;current=null;
    if(!fileId){option(backupSelect,'',t.noBackups);backupSelect.disabled=true;renderDiff();return;}
    const gameId=getGame();let doc:Awaited<ReturnType<Workspace['read']>>;
    try{doc=await client.read(gameId,fileId);}catch{if(!disposed&&epoch===operation)fail(t.failed);return;}
    if(disposed||epoch!==operation||fileSelect.value!==fileId||getGame()!==gameId)return;current=doc;
    if(!doc.backups.length){option(backupSelect,'',t.noBackups);backupSelect.disabled=true;renderDiff();return;}
    backupSelect.disabled=false;
    for(const backup of doc.backups)option(backupSelect,backup.id,new Date(backup.createdAt).toLocaleString(context.locale==='zh-CN'?'zh-CN':'en'));
    if(preselect&&doc.backups.some(backup=>backup.id===preselect))backupSelect.value=preselect;
    await loadComparison();
  };
  const apply=async(fieldsToRestore:string[]|null)=>{
    if(!current||!saved||busy)return;
    busy=true;clearStatus();updateActions();
    try{
      const result=await restoreSelection(client,getGame(),current,saved,fieldsToRestore&&fields?.length?fieldsToRestore:null);
      current=result;undoConfigId=current.id;
      await loadBackups();
    }catch(error){
      fail(error instanceof Error&&error.message==='CONFIG_CHANGED'?t.changed:error instanceof Error&&error.message==='PLUGIN_ACTION_CANCELLED'?t.cancelled:t.failed);
    }finally{busy=false;updateActions();}
  };
  fileSelect.onchange=()=>void loadBackups();backupSelect.onchange=()=>void loadComparison();
  restoreFields.onclick=()=>void apply([...diff.querySelectorAll<HTMLInputElement>('input[type=checkbox]:checked')].map(input=>input.value));
  restoreFile.onclick=()=>void apply(null);
  undo.onclick=async()=>{if(!current?.backups[0]||busy)return;busy=true;updateActions();try{saved=await client.readBackup(getGame(),current.id,current.backups[0].id);busy=false;await apply(null);}catch{fail(t.failed);}finally{busy=false;updateActions();}};
  const target=(context as Context&{configRestoreTarget?:{configId:string;backupId:string}|null}).configRestoreTarget;
  if(getGame())void loadFiles(target);else{status.textContent=t.noFiles;fileSelect.disabled=true;backupSelect.disabled=true;updateActions();}
  return { refresh:()=>void loadFiles(), dispose:()=>{disposed=true;operation++;comparison++;host.replaceChildren();} };
}
