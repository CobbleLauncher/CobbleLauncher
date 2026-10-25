// SPDX-License-Identifier: MIT
import type {Report} from '../../../../packages/plugin-web-sdk/src/index';
import {timelineFromReport,seconds} from './timeline-model';
const stageNames:Record<string,string>={'Fabric loader':'Fabric 加载器','Mod list':'模组列表','Mixin initialization':'Mixin 初始化','Graphics backend':'图形后端','Resource reload':'资源重载','Audio backend':'音频后端','Sound engine':'声音引擎'};
export function renderTimeline(host:HTMLElement,report:Report|null,zh:boolean,busy:boolean,reveal?:((first:number,last:number)=>void)) {
  const tr=(en:string,cn:string)=>zh?cn:en;
  const el=<K extends keyof HTMLElementTagNameMap>(tag:K,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
  host.replaceChildren();const timeline=timelineFromReport(report);
  if(!timeline){host.append(el('p',busy?tr('Reading timeline…','正在读取时间线…'):tr('Analyze a log to view its timeline.','分析日志以查看时间线。'),'timeline-empty'));return;}
  const metric=(key:string)=>timeline.metrics.get(key)??null,segments=metric('Segments')??0;
  const stats=el('div','','timeline-stats');
  for(const [value,label] of [[seconds(metric('Observed span ms')),tr('Logged span','日志时间跨度')],[metric('Stage events'),tr('Stage messages','阶段消息')],[metric('Warnings'),tr('Warnings','警告')]]){const cell=el('div');cell.append(el('strong',String(value??'—')),el('span',String(label)));stats.append(cell);}host.append(stats);
  if(!metric('Timestamped lines'))host.append(el('p',tr('No supported timestamps. Stage messages have no measured timing.','没有支持的时间戳，阶段消息没有可测量时间。'),'timeline-notice'));
  else if(segments>1)host.append(el('p',tr(`Clock decreased; timing is split into ${segments} segments. ${metric('Possible midnight rollovers')??0} possible midnight rollover(s).`,`时钟倒退，时间已拆分为 ${segments} 段，其中 ${metric('Possible midnight rollovers')??0} 次可能为跨午夜。`),'timeline-notice'));
  const coverage=el('p',tr(`${metric('Timestamped lines')??0} timestamped lines · ${metric('Untimed lines')??0} untimed lines`,`${metric('Timestamped lines')??0} 行有时间戳 · ${metric('Untimed lines')??0} 行无可用时间戳`),'timeline-coverage');host.append(coverage);
  const layout=el('div','','timeline-layout'),eventSection=el('section'),events=el('ol','','stage-timeline');eventSection.append(el('h2',tr('Stage messages','阶段消息')));
  if(!timeline.events.length)eventSection.append(el('p',tr('No recognized stage messages.','没有识别到阶段消息。'),'timeline-empty'));
  for(const event of timeline.events){const item=el('li'),button=el('button','','stage-event'),clock=el('span',event.clock||tr('Time unknown','时间未知'),'event-clock'),title=el('strong',zh?(stageNames[event.stage]??event.stage):event.stage),offset=el('span',event.elapsed===null?'—':`${segments>1?tr(`Segment ${event.segment} · `,`第 ${event.segment} 段 · `):''}+${seconds(event.elapsed)}`,'event-offset'),line=el('small',tr(`Line ${event.line}`,`第 ${event.line} 行`));button.append(clock,title,offset,line);button.disabled=!reveal;button.onclick=()=>reveal?.(event.line,event.line);item.append(button);const excerpt=el('details','','event-evidence');excerpt.append(el('summary',tr('Evidence','证据')),el('pre',event.excerpt));item.append(excerpt);events.append(item);}eventSection.append(events);layout.append(eventSection);
  const aside=el('div','','timeline-aside'),gaps=el('section'),clusters=el('section');gaps.append(el('h2',tr('Longest log gaps','最长日志间隔')),el('p',tr('Gaps of 5 s or more between logged timestamps. These are not measured stage durations.','相邻日志时间戳之间至少 5 秒的间隔，并非阶段实际耗时。'),'timeline-help'));
  if(!timeline.gaps.length)gaps.append(el('p',!metric('Timestamped lines')?tr('No usable timing.','没有可用时间。'):tr('No gaps of 5 s or more.','没有至少 5 秒的间隔。'),'timeline-empty'));
  for(const gap of timeline.gaps){const card=el('button','','gap-card'),label=el('strong',seconds(gap.ms)),range=el('span',`${gap.from} → ${gap.to}`),lines=el('small',tr(`Lines ${gap.first}–${gap.last}${segments>1?` · Segment ${gap.segment}`:''}`,`第 ${gap.first}–${gap.last} 行${segments>1?` · 第 ${gap.segment} 段`:''}`)),bar=el('span','','gap-bar');bar.style.width=`${100*gap.ms/timeline.gaps[0]!.ms}%`;card.append(label,range,lines,bar);card.disabled=!reveal;card.onclick=()=>reveal?.(gap.first,gap.last);gaps.append(card);}aside.append(gaps);
  clusters.append(el('h2',tr('Warning clusters','警告集中区间')),el('p',tr('Three or more WARN messages within 10 s.','10 秒内至少 3 条 WARN 消息。'),'timeline-help'));
  if(!timeline.clusters.length)clusters.append(el('p',tr('No timed warning clusters.','没有符合条件的警告集中区间。'),'timeline-empty'));
  for(const cluster of timeline.clusters){const card=el('button','','cluster-card');card.append(el('strong',tr(`${cluster.count} warnings`,`${cluster.count} 条警告`)),el('span',`${cluster.from} → ${cluster.to}`),el('small',tr(`Lines ${cluster.first}–${cluster.last}`,`第 ${cluster.first}–${cluster.last} 行`)));card.disabled=!reveal;card.onclick=()=>reveal?.(cluster.first,cluster.last);clusters.append(card);}aside.append(clusters);layout.append(aside);host.append(layout);
  const omitted=['Unshown stage events','Unshown gaps','Unshown clusters'].map(key=>metric(key)??0);
  if(omitted.some(Boolean))host.append(el('p',tr(`Not shown: ${omitted[0]} stage messages, ${omitted[1]} gaps, ${omitted[2]} clusters. Totals include all granted log text.`,`未显示 ${omitted[0]} 条阶段消息、${omitted[1]} 个间隔、${omitted[2]} 个集中区间。统计包含已授权日志全文。`),'timeline-notice'));
  if(metric('Untimed warnings'))host.append(el('p',tr(`${metric('Untimed warnings')} warnings have no usable timestamp.`,`${metric('Untimed warnings')} 条警告没有可用时间戳。`),'timeline-coverage'));
}
