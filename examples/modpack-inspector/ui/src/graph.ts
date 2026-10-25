// SPDX-License-Identifier: MIT
import * as dagre from '@dagrejs/dagre';
import type {Fact, GraphNode} from './model';
/** Dagre positions and routes the bounded graph; ordinary buttons provide interaction. */
export function renderGraph(host:HTMLElement,nodes:readonly GraphNode[],facts:readonly Fact[],selected:string|null,zh:boolean,choose:(id:string)=>void) {
  const graph = new dagre.graphlib.Graph({multigraph:true}).setGraph({rankdir:'LR',nodesep:22,ranksep:80,marginx:24,marginy:24});
  graph.setDefaultEdgeLabel(() => ({}));
  for (const node of nodes) graph.setNode(node.id,{width:172,height:64});
  facts.forEach((fact,index) => {for (const target of fact.targets) if (graph.hasNode(target)) graph.setEdge(fact.source,target,{optional:fact.optional},String(index));});
  dagre.layout(graph);
  const width = Math.max(400,graph.graph().width ?? 400), height = Math.max(260,graph.graph().height ?? 260);
  const surface = document.createElement('div'); surface.className = 'graph-surface'; surface.style.width = `${width}px`;surface.style.height = `${height}px`;
  const ns = 'http://www.w3.org/2000/svg', svg = document.createElementNS(ns,'svg');svg.setAttribute('width',String(width));svg.setAttribute('height',String(height));svg.setAttribute('aria-hidden','true');
  const defs = document.createElementNS(ns,'defs'), marker = document.createElementNS(ns,'marker'), arrow = document.createElementNS(ns,'path');
  marker.id='dependency-arrow';marker.setAttribute('viewBox','0 0 10 10');marker.setAttribute('refX','9');marker.setAttribute('refY','5');marker.setAttribute('markerWidth','7');marker.setAttribute('markerHeight','7');marker.setAttribute('orient','auto-start-reverse');arrow.setAttribute('d','M 0 0 L 10 5 L 0 10 z');marker.append(arrow);defs.append(marker);svg.append(defs);
  for (const edge of graph.edges()) {
    const value = graph.edge(edge), line = document.createElementNS(ns,'polyline');
    line.setAttribute('points',value.points.map(point => `${point.x},${point.y}`).join(' '));line.setAttribute('marker-end','url(#dependency-arrow)');
    if (value.optional) line.classList.add('optional');
    if (edge.v === selected || edge.w === selected) line.classList.add('connected');
    svg.append(line);
  }
  surface.append(svg);
  for (const item of nodes) {
    const position = graph.node(item.id), button = document.createElement('button'), name = document.createElement('strong'), detail = document.createElement('span');
    button.className = 'graph-node';button.dataset.kind = item.kind;button.dataset.selected = String(item.id === selected);button.setAttribute('aria-pressed',String(item.id === selected));button.title = item.name;
    button.style.left = `${position.x-86}px`;button.style.top = `${position.y-32}px`;
    name.textContent = item.name;detail.textContent = item.mod ? item.mod.disabled ? (zh?'已禁用':'Disabled') : item.mod.version ?? (zh?'版本未知':'Unknown version') : item.kind === 'runtime' ? (zh?'运行环境 · 未检查':'Runtime · unchecked') : (zh?'无已声明匹配':'No declared match');
    button.append(name,detail);button.onclick = () => choose(item.id);surface.append(button);
  }
  host.replaceChildren(surface);
}
