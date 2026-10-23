// SPDX-License-Identifier: MIT
import type { Client, Context, KeybindingPreview, KeybindingResult } from '../../../../packages/plugin-web-sdk/src/index';

export function bindingLabel(value: string, chinese = false): string {
  const labels: Record<string, string[]> = {forward:['Forward','前进'],back:['Backward','后退'],left:['Left','向左'],right:['Right','向右'],jump:['Jump','跳跃'],sneak:['Sneak','潜行'],sprint:['Sprint','疾跑'],attack:['Attack / destroy','攻击 / 破坏'],use:['Use / place','使用 / 放置'],inventory:['Inventory','物品栏'],drop:['Drop item','丢弃物品'],chat:['Chat','聊天'],playerlist:['Player list','玩家列表'],swapOffhand:['Swap hands','交换双手物品'],pickItem:['Pick block','选取方块'],fullscreen:['Fullscreen','全屏'],perspective:['Perspective','切换视角']};
  const name=value.replace(/^key_key\./,'').replace(/^key_/,'');
  return labels[name]?.[chinese ? 1 : 0] ?? name.replace(/[._]/g,' ').replace(/([a-z])([A-Z])/g,'$1 $2');
}
export function keyLabel(value: string): string {
  if(value==='key.keyboard.unknown')return '—';
  if(value.startsWith('key.mouse.'))return ({left:'Mouse 1',right:'Mouse 2',middle:'Mouse 3'} as Record<string,string>)[value.slice(10)] ?? `Mouse ${value.slice(10)}`;
  return value.replace(/^key.keyboard\./,'').replaceAll('.',' ').toUpperCase();
}
export function defaultSource(context: Context): string {
  const games=context.keybindingGames??context.games;
  const target=context.keybindingTargetId;
  const current=games.find(game=>game.id===context.instanceId&&game.id!==target);
  return current?.id??context.recentGames?.filter(game=>game.id!==target&&games.some(item=>item.id===game.id)).sort((a,b)=>(b.lastLaunchedAt??0)-(a.lastLaunchedAt??0))[0]?.id??games.find(game=>game.id!==target)?.id??"";
}
export function mountKeybindings(client: Client, context: Context, host: HTMLElement) {
  const zh=context.locale==='zh-CN'; const tr=(en:string,cn:string)=>zh?cn:en;
  const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='')=>{const item=document.createElement(tag);item.textContent=text;return item;};
  let preview:KeybindingPreview|null=null, backup:KeybindingResult|null=null, busy=false;
  const source=el('select'), target=el('select'); const games=context.keybindingGames??context.games;
  source.setAttribute('aria-label',tr('Copy from','复制来源')); target.setAttribute('aria-label',tr('Copy to','复制目标'));
  for(const game of games.filter(game=>game.id!==context.keybindingTargetId))source.add(new Option(`${game.name} · ${game.version}`,game.id));
  source.value=defaultSource(context);
  const sourceBox=el('label',tr('Copy from','复制来源')),targetBox=el('label',tr('Copy to','复制目标'));
  sourceBox.append(source);targetBox.append(target);
  const arrow=el('span','→');arrow.className='transfer-arrow';arrow.setAttribute('aria-hidden','true');
  const choices=el('div');choices.className='transfer-choices';choices.append(sourceBox,arrow,targetBox);
  const inspect=el('button',tr('Compare keybindings','对比按键')),apply=el('button',tr('Copy keybindings','复制按键')),restore=el('button',tr('Restore backup','恢复备份'));
  apply.className='primary'; const actions=el('div');actions.className='actions';actions.append(inspect,apply,restore);
  const status=el('p'),error=el('p');status.setAttribute('role','status');status.setAttribute('aria-live','polite');error.setAttribute('role','alert');error.className='error';error.hidden=true;
  const comparison=el('div');comparison.className='binding-comparison';host.append(choices,actions,error,status,comparison);
  function update(){source.disabled=busy||!!backup||(!context.keybindingTargetId&&!!context.instanceId);target.disabled=busy||!!backup||!!context.keybindingTargetId;inspect.disabled=busy||!source.value||!target.value;inspect.textContent=backup?tr('New comparison','重新对比'):tr('Compare keybindings','对比按键');apply.hidden=!preview;apply.disabled=busy||!preview?.changes.length;restore.hidden=!backup;restore.disabled=busy;status.classList.toggle('loading',busy);host.setAttribute('aria-busy',String(busy));}
  function clear(){preview=null;backup=null;comparison.replaceChildren();status.textContent='';error.hidden=true;update();}
  function targets(){target.replaceChildren(new Option(tr('Choose target game','选择目标游戏'),''));for(const game of games.filter(game=>game.id!==source.value))target.add(new Option(`${game.name} · ${game.version}`,game.id));target.value=context.keybindingTargetId??"";clear();}
  function render(){comparison.replaceChildren();if(!preview)return;const table=el('table'),head=el('thead'),row=el('tr');for(const text of [tr('Action','操作'),tr('Target now','目标当前按键'),tr('After copy','复制后')]){const cell=el('th',text);cell.scope='col';row.append(cell);}head.append(row);table.append(head);const body=el('tbody');for(const change of preview.changes){const row=el('tr'),name=el('td',bindingLabel(change.key,zh)),details=el('details'),summary=el('summary',tr('Raw binding','原始键名'));details.append(summary,el('code',`${change.key}: ${change.before} → ${change.after}`));name.append(details);row.append(name,el('td',keyLabel(change.before)),el('td',keyLabel(change.after)));body.append(row);}table.append(body);comparison.append(table);for(const skipped of preview.skippedBindings??[]){comparison.append(el('p',bindingLabel(skipped.key,zh)+' · '+(skipped.reason==='missing-target'?tr('Not available in target; skipped','目标没有此按键，已跳过'):tr('Unknown key format; skipped','未知按键格式，已跳过'))));}for(const conflict of preview.conflicts??[]){comparison.append(el('p',tr('Conflict: ','冲突：')+bindingLabel(conflict.key,zh)+' / '+bindingLabel(conflict.otherKey,zh)+' · '+keyLabel(conflict.value)));}status.textContent=preview.changes.length?tr(`${preview.changes.length} changes · ${preview.compatible??preview.changes.length} compatible · ${preview.skipped} unsupported bindings skipped`,`${preview.changes.length} 项更改 · ${preview.compatible??preview.changes.length} 项兼容 · 跳过 ${preview.skipped} 项不支持的按键`):tr('Keybindings already match.','按键已经一致。');}
  async function run(message:string,action:()=>Promise<void>){if(busy)return;busy=true;error.hidden=true;status.textContent=message;update();try{await action();}catch(cause){const code=String(cause);error.textContent=code.includes('KEYBINDINGS_NOT_INITIALIZED')?tr('Start and close each game once to create its keybindings.','请先启动并关闭一次游戏，生成按键配置。'):code.includes('CONFIG_BUSY')?tr('Stop both games before continuing.','请先关闭两个游戏。'):code.includes('CONFIG_CHANGED')?tr('Settings changed. Compare again before copying.','配置已更改，请重新对比。'):code.includes('PERMISSION')?tr('Approve keybinding access in extension permissions.','请在扩展权限中批准按键访问。'):tr('Could not complete this action. Compare again and retry.','操作未完成，请重新对比后重试。');error.hidden=false;status.textContent='';preview=null;}finally{busy=false;update();}}
  inspect.onclick=()=>{if(backup){clear();return;}void run(tr('Comparing…','正在对比…'),async()=>{preview=await client.keybindings.preview(source.value,target.value);render();});};
  apply.onclick=()=>void run(tr('Copying…','正在复制…'),async()=>{const value=preview;if(!value)return;backup=await client.keybindings.apply(source.value,target.value,value.sourceRevision,value.targetRevision);preview=null;comparison.replaceChildren();status.textContent=tr('Keybindings copied. A backup is ready to restore.','按键已复制，可恢复原配置备份。');});
  restore.onclick=()=>void run(tr('Restoring…','正在恢复…'),async()=>{const value=backup;if(!value)return;await client.keybindings.restore(target.value,value.backupId,value.targetRevision);backup=null;status.textContent=tr('Original keybindings restored.','已恢复原按键配置。');});
  source.onchange=targets;target.onchange=clear;targets();
  return ()=>!busy;
}
