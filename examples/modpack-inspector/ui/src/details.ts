// SPDX-License-Identifier: MIT
import type {Fact,FactStatus,Graph} from './model';
export const factLabel = (status:FactStatus,zh:boolean) => ({present:['ID present','声明 ID 已存在'],disabled:['Disabled provider','提供者已禁用'],missing:['No declared match','无已声明匹配'],runtime:['Runtime unchecked','运行环境未检查'],ambiguous:['Multiple providers','多个提供者']}[status][zh?1:0]!);
export function renderDetails(host:HTMLElement,graph:Graph,id:string|null,zh:boolean,choose:(id:string)=>void) {
  const tr = (en:string,cn:string) => zh?cn:en;
  const el = (tag:string,text='',className='') => {const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
  host.replaceChildren(); const node = id ? graph.nodes.get(id) : null;
  if (!node) {host.append(el('p',tr('Select a Mod.','选择一个模组。'),'empty'));return;}
  host.append(el('h2',node.name));
  if (node.mod) {
    const mod=node.mod;
    host.append(el('p',[mod.version ?? tr('Unknown version','版本未知'),mod.loader,mod.disabled?tr('Disabled','已禁用'):tr('Enabled','已启用')].filter(Boolean).join(' · '),'muted'));
    const ids=mod.declaredIds.filter(value=>value!==node.name);
    if(ids.length||!mod.declaredIds.length)host.append(el('p',mod.declaredIds.length?ids.join(', '):tr('No readable dependency metadata.','没有可读取的依赖元数据。'),'mod-ids'));
  }
  function section(title:string,facts:Fact[],dependents:boolean) {
    const box=el('section','','relations');box.append(el('h3',`${title} (${facts.length})`));
    if (!facts.length) box.append(el('p',tr('None declared.','未声明。'),'muted'));
    for (const fact of facts) {
      const row=el('div','','relation');
      const title=el('strong',dependents ? graph.nodes.get(fact.source)?.name ?? fact.source : fact.id);
      row.append(title,el('span',`${fact.version ?? tr('Unknown requirement','要求未知')} · ${fact.optional?tr('Optional','可选'):tr('Required','必需')}`,'muted'),el('span',factLabel(fact.status,zh),'fact-status'));
      const ids=dependents?[fact.source]:fact.targets;
      for (const target of ids) {
        const candidate=graph.nodes.get(target);if (!candidate || candidate.id===id)continue;
        const button=el('button',candidate.name,'relation-link') as HTMLButtonElement;button.onclick=()=>choose(target);row.append(button);
      }
      box.append(row);
    }
    host.append(box);
  }
  section(tr('Dependencies','依赖'),graph.outgoing.get(node.id) ?? [],false);
  section(tr('Dependents','被依赖'),graph.incoming.get(node.id) ?? [],true);
  const missing=(graph.outgoing.get(node.id) ?? []).filter(fact=>!fact.optional && ['missing','disabled'].includes(fact.status));
  if (missing.length) host.append(el('p',tr(`${missing.length} required declarations have no enabled ID match.`,`${missing.length} 个必需声明没有已启用的 ID 匹配。`),'notice'));
  host.append(el('p',tr('Version ranges and game compatibility are not checked.','未检查版本范围和游戏兼容性。'),'muted'));
}
