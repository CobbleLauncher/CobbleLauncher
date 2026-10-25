// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import {connect,hostContext,type Client,type Context,type DiagnosticDocument,type DiagnosticFile} from '../../../../packages/plugin-web-sdk/src/index';
import {localEndpoint,prepareMessages,requestId,validEndpoint,type ChatMessage} from './model';
import '../../../../packages/plugin-workbench/src/style.css';
import './style.css';
import '../../../../packages/plugin-workbench/src/host-appearance.css';

const copy={
  en:{title:'AI Assistant',settings:'Connection',endpoint:'Chat completions URL',model:'Model',key:'API key',keyLocal:'Optional for a local server.',keyAgain:'Enter the key again to save changes.',save:'Save connection',cancelSetup:'Cancel',clear:'Remove connection',permissions:'AI chat and diagnostic access are reviewed when this extension is installed.',badge:'AI chat',conversation:'Conversation',empty:'Ask about your game or a launch problem.',retry:'Retry',diagnostic:'Attach game diagnostic',game:'Game',file:'Diagnostic file',preview:'Preview redacted file',attach:'Attach to next message',prompt:'Message',placeholder:'Ask a question…',send:'Send',cancel:'Cancel',selectGame:'Choose a game',selectFile:'Choose a file',noGames:'No game is available.',noFiles:'No diagnostic files found.',ready:'The file is not attached until you select the checkbox.',previewTail:'Preview shows the last 32 KiB of redacted text.',waiting:'Waiting for a reply…',cancelling:'Cancelling…',cancelled:'Request cancelled.',omitted:'Earlier messages were omitted to fit the request limit.',configure:'Enter a valid URL and model. A key is required for a remote service.',tooLong:'This message is too long.',failed:'Could not complete the request. Try again.',configFailed:'Could not save the connection.',diagnosticChanged:'This file changed. Preview it again.',confirmCancelled:'Attachment was not sent.',notConfigured:'Set up a connection first.',busy:'Another request is in progress.',noDiagnostic:'Choose a game and file to preview.',previewFailed:'Could not preview this file.',listFailed:'Could not list diagnostic files.',removeFailed:'Could not remove the connection.'},
  'zh-CN':{title:'AI 助手',settings:'连接设置',endpoint:'聊天接口地址',model:'模型',key:'API 密钥',keyLocal:'本地服务可不填密钥。',keyAgain:'更改连接时请重新输入密钥。',save:'保存连接',cancelSetup:'取消',clear:'移除连接',permissions:'安装扩展时会审阅 AI 聊天与诊断读取权限。',badge:'AI 聊天',conversation:'对话',empty:'询问游戏或启动问题。',retry:'重试',diagnostic:'附加游戏诊断',game:'游戏',file:'诊断文件',preview:'预览脱敏文件',attach:'附加到下一条消息',prompt:'消息',placeholder:'输入问题…',send:'发送',cancel:'取消',selectGame:'选择游戏',selectFile:'选择文件',noGames:'没有可用游戏。',noFiles:'未找到诊断文件。',ready:'勾选后才会附加此文件。',previewTail:'预览的是已脱敏文本最后 32 KiB。',waiting:'正在等待回复…',cancelling:'正在取消…',cancelled:'已取消请求。',omitted:'较早的消息未纳入本次请求。',configure:'请输入有效地址和模型。远程服务还需密钥。',tooLong:'消息过长。',failed:'请求未完成，请重试。',configFailed:'无法保存连接。',diagnosticChanged:'文件已变化，请重新预览。',confirmCancelled:'未发送附件。',notConfigured:'请先设置连接。',busy:'另一请求正在进行。',noDiagnostic:'请选择游戏和文件并预览。',previewFailed:'无法预览此文件。',listFailed:'无法列出诊断文件。',removeFailed:'无法移除连接。'}
} as const;
type Locale=keyof typeof copy;type Key=keyof typeof copy.en;
const el=<T extends HTMLElement=HTMLElement>(id:string)=>document.getElementById(id) as T;
const text=(id:string,value:string)=>{el(id).textContent=value;};
let locale:Locale='en',client:Client,context:Context,configured=false,busy=false,setting=false;
let history:ChatMessage[]=[],files:DiagnosticFile[]=[],previewed:DiagnosticDocument|null=null,previewGame='';
let attempt:{messages:ChatMessage[];diagnostic?:{gameId:string;diagnosticId:string;revision:string};omitted:number}|null=null;
let activeRequest='',cancelRequested=false,loadEpoch=0;
const t=(key:Key):string=>copy[locale][key];
const keyOf=(cause:unknown)=>String(cause).match(/AI_[A-Z_]+/)?.[0]??'';
function explain(cause:unknown,fallback:Key):string{
  const key=keyOf(cause);
  return key==='AI_CONFIG_INVALID'?t('configure'):key==='AI_NOT_CONFIGURED'?t('notConfigured'):
    key==='AI_BUSY'?t('busy'):key==='AI_DIAGNOSTIC_CHANGED'?t('diagnosticChanged'):
    key==='AI_CONFIRM_CANCELLED'?t('confirmCancelled'):key==='AI_CANCELLED'?t('cancelled'):t(fallback);
}
function error(value=''){text('error',value);el('error').hidden=!value;}
function status(value=''){text('status',value);}
function keyHint(){const endpoint=el<HTMLInputElement>('endpoint').value.trim();const hint=localEndpoint(endpoint)?t('keyLocal'):configured?t('keyAgain'):'';text('key-hint',hint);el('key-hint').hidden=!hint;}
function controls(){
  el('setup').hidden=!setting;el('chat').hidden=setting||!configured;el('settings-toggle').hidden=!configured||setting;
  el<HTMLButtonElement>('settings-toggle').disabled=busy;
  el<HTMLButtonElement>('send').disabled=busy||!configured;
  el<HTMLButtonElement>('cancel').hidden=!busy;
  el<HTMLButtonElement>('cancel').disabled=cancelRequested;
  el<HTMLTextAreaElement>('prompt').disabled=busy;
  el('retry-row').hidden=!attempt||busy;
  el<HTMLButtonElement>('load-file').disabled=busy||!el<HTMLSelectElement>('game').value||!el<HTMLSelectElement>('file').value;
  el<HTMLInputElement>('attach').disabled=busy||!previewed;
  el<HTMLSelectElement>('game').disabled=busy||!!context?.instanceId;
  el<HTMLSelectElement>('file').disabled=busy||!files.length;
  el('attachment-chip').hidden=!el<HTMLInputElement>('attach').checked||!previewed;
  if(previewed&&el<HTMLInputElement>('attach').checked)text('attachment-chip',previewed.name);
}
function renderMessages(){
  const list=el<HTMLOListElement>('messages');list.replaceChildren();el('empty').hidden=history.length>0;
  for(const message of history){const item=document.createElement('li');item.className=`message message-${message.role}`;
    const body=document.createElement('p');body.textContent=message.content;item.append(body);list.append(item);}
  list.lastElementChild?.scrollIntoView({block:'nearest'});
}
function fillGames(){
  const select=el<HTMLSelectElement>('game');select.replaceChildren();
  const first=new Option(t('selectGame'),'');select.add(first);
  for(const game of context.recentGames??context.games)select.add(new Option(`${game.name} · ${game.version}`,game.id));
  if(context.instanceId)select.value=context.instanceId;
  el('game-field').hidden=!!context.instanceId;
  if(!select.value&&select.options.length===1)text('diagnostic-status',t('noGames'));
}
function clearPreview(){previewed=null;previewGame='';el('preview-wrap').hidden=true;el<HTMLInputElement>('attach').checked=false;text('preview','');text('preview-name','');controls();}
async function listFiles(){
  const epoch=++loadEpoch,gameId=el<HTMLSelectElement>('game').value;clearPreview();files=[];
  const select=el<HTMLSelectElement>('file');select.replaceChildren(new Option(t('selectFile'),''));
  if(!gameId){text('diagnostic-status',t('noGames'));controls();return;}
  text('diagnostic-status','');
  try{const result=await client.diagnostics.list(gameId);if(epoch!==loadEpoch)return;
    files=result;for(const file of files)select.add(new Option(file.name,file.id));
    text('diagnostic-status',files.length?'':t('noFiles'));
  }catch{if(epoch===loadEpoch)text('diagnostic-status',t('listFailed'));}finally{controls();}
}
function tail(textValue:string):{value:string;truncated:boolean}{
  const bytes=new TextEncoder().encode(textValue);
  return {value:new TextDecoder().decode(bytes.subarray(Math.max(0,bytes.length-32768))),truncated:bytes.length>32768};
}
async function previewFile(){
  const gameId=el<HTMLSelectElement>('game').value,id=el<HTMLSelectElement>('file').value;
  const epoch=++loadEpoch;clearPreview();if(!gameId||!id){text('diagnostic-status',t('noDiagnostic'));return;}
  text('diagnostic-status','');el<HTMLButtonElement>('load-file').disabled=true;
  try{const result=await client.diagnostics.read(gameId,id);if(epoch!==loadEpoch)return;
    previewed=result;previewGame=gameId;const view=tail(result.text);
    text('preview-name',result.name);text('preview',view.value);
    el('preview-wrap').hidden=false;text('diagnostic-status',view.truncated?t('previewTail'):t('ready'));
  }catch{if(epoch===loadEpoch)text('diagnostic-status',t('previewFailed'));}finally{controls();}
}
async function loadStatus(){
  const value=await client.ai.status();configured=value.configured;
  setting=!configured;
  el<HTMLInputElement>('endpoint').value=value.endpoint??'';
  el<HTMLInputElement>('model').value=value.model??'';
  el<HTMLInputElement>('key').value='';
  keyHint();
  text('connection-summary',[value.model,value.endpoint].filter(Boolean).join(' · '));
  el('setup-cancel').hidden=!configured;el('clear').hidden=!configured;controls();
}
async function saveConfig(event:SubmitEvent){
  event.preventDefault();error();const endpoint=el<HTMLInputElement>('endpoint').value.trim();
  const model=el<HTMLInputElement>('model').value.trim(),key=el<HTMLInputElement>('key').value;
  if(!validEndpoint(endpoint)||!model||(!localEndpoint(endpoint)&&!key)){error(t('configure'));return;}
  el<HTMLButtonElement>('save').disabled=true;
  try{const value=await client.ai.configure({endpoint,model,key});
    el<HTMLInputElement>('key').value='';configured=value.configured;setting=false;
    text('connection-summary',[value.model,value.endpoint].filter(Boolean).join(' · '));controls();el<HTMLTextAreaElement>('prompt').focus();
  }catch(cause){error(explain(cause,'configFailed'));}
  finally{el<HTMLInputElement>('key').value='';el<HTMLButtonElement>('save').disabled=false;}
}
async function clearConfig(){error();el<HTMLButtonElement>('clear').disabled=true;
  try{await client.ai.clear();history=[];attempt=null;renderMessages();await loadStatus();}
  catch(cause){error(explain(cause,'removeFailed'));}
  finally{el<HTMLButtonElement>('clear').disabled=false;}
}
async function runAttempt(current:NonNullable<typeof attempt>){
  if(busy)return;busy=true;cancelRequested=false;error();status(t('waiting'));controls();
  const id=requestId();activeRequest=id;
  try{const answer=await client.ai.chat({requestId:id,messages:current.messages,...(current.diagnostic?{diagnostic:current.diagnostic}:{})});
    if(cancelRequested){status(t('cancelled'));attempt=current;return;}
    history.push({role:'assistant',content:answer.reply});attempt=null;renderMessages();
    status(current.omitted?t('omitted'):'');
    el<HTMLInputElement>('attach').checked=false;
  }catch(cause){
    if(cancelRequested||keyOf(cause)==='AI_CANCELLED'){status(t('cancelled'));attempt=current;}
    else{error(explain(cause,'failed'));status('');attempt=current;
      if(keyOf(cause)==='AI_DIAGNOSTIC_CHANGED'){
        clearPreview();attempt=null;history.pop();renderMessages();
        el<HTMLTextAreaElement>('prompt').value=current.messages.at(-1)?.content??'';
        el<HTMLDetailsElement>('diagnostic-panel').open=true;
      }}
  }finally{if(activeRequest===id){busy=false;activeRequest='';cancelRequested=false;controls();}}
}
async function send(event:SubmitEvent){
  event.preventDefault();if(busy||!configured)return;
  const input=el<HTMLTextAreaElement>('prompt'),prompt=input.value;
  let prepared:ReturnType<typeof prepareMessages>;
  try{prepared=prepareMessages(history,prompt);}catch{error(t('tooLong'));return;}
  const diagnostic=el<HTMLInputElement>('attach').checked&&previewed&&previewGame===el<HTMLSelectElement>('game').value?
    {gameId:previewGame,diagnosticId:previewed.id,revision:previewed.revision}:undefined;
  history.push({role:'user',content:prompt.trim()});input.value='';renderMessages();
  attempt={messages:prepared.messages,diagnostic,omitted:prepared.omitted};await runAttempt(attempt);
}
async function cancel(){if(!activeRequest||cancelRequested)return;
  cancelRequested=true;status(t('cancelling'));controls();
  try{await client.ai.cancel({requestId:activeRequest});}catch{error(t('failed'));}
}
function translate(){
  for(const [id,key] of Object.entries({title:'title','settings-toggle':'settings','endpoint-label':'endpoint','model-label':'model','key-label':'key',save:'save','setup-cancel':'cancelSetup',clear:'clear','permission-badge':'badge',empty:'empty',retry:'retry','diagnostic-heading':'diagnostic','game-label':'game','file-label':'file','load-file':'preview','attach-label':'attach','prompt-label':'prompt',send:'send',cancel:'cancel'}))text(id,t(key as Key));
  el<HTMLTextAreaElement>('prompt').placeholder=t('placeholder');el('messages').setAttribute('aria-label',t('conversation'));
}
el<HTMLFormElement>('setup-form').onsubmit=event=>void saveConfig(event);
el<HTMLFormElement>('composer').onsubmit=event=>void send(event);
el<HTMLButtonElement>('settings-toggle').onclick=()=>{if(busy)return;setting=true;error();controls();el<HTMLInputElement>('key').value='';keyHint();};
el<HTMLInputElement>('endpoint').oninput=()=>keyHint();
el<HTMLButtonElement>('setup-cancel').onclick=()=>{setting=false;el<HTMLInputElement>('key').value='';error();controls();};
el<HTMLButtonElement>('clear').onclick=()=>void clearConfig();
el<HTMLButtonElement>('retry').onclick=()=>{if(attempt)void runAttempt(attempt);};
el<HTMLButtonElement>('cancel').onclick=()=>void cancel();
el<HTMLButtonElement>('load-file').onclick=()=>void previewFile();
el<HTMLSelectElement>('game').onchange=()=>void listFiles();
el<HTMLSelectElement>('file').onchange=()=>clearPreview();
el<HTMLInputElement>('attach').onchange=()=>controls();
el<HTMLDetailsElement>('diagnostic-panel').ontoggle=()=>{if(el<HTMLDetailsElement>('diagnostic-panel').open&&context)void listFiles();};
el<HTMLTextAreaElement>('prompt').onkeydown=event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();el<HTMLFormElement>('composer').requestSubmit();}};
window.addEventListener('pagehide',()=>{++loadEpoch;if(activeRequest)void client?.ai.cancel({requestId:activeRequest}).catch(()=>{});},{once:true});
async function start(){
  try{client=connect();context=await hostContext(client.context());locale=context.locale==='zh-CN'?'zh-CN':'en';
    document.documentElement.lang=locale;translate();fillGames();await loadStatus();
  }catch{error(t('failed'));}
}
void start();

installPluginPresentation(presentation);
