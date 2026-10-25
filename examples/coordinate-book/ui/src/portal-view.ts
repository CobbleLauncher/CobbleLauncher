// SPDX-License-Identifier: MIT
import { coordinates, type Book, type Location } from './model';
import { portalCandidates, portalOriginLocations, type PortalDirection } from './portal-planner';
import { button, el, option } from './view';

type Actions = {
  direction: (value: PortalDirection) => void;
  origin: (value: string) => void;
  search: (value: string) => void;
  select: (value: string) => void;
  open: (location: Location) => void;
};
const text = (en: string, zh: string, isZh: boolean) => isZh ? zh : en;

export function createPortalView(isZh: boolean, actions: Actions) {
  const root = el('section', '', 'portal-planner');
  const heading = el('h2', text('Portal planner', '传送门规划', isZh));
  const controls = el('div', '', 'portal-controls');
  const direction = el('select');
  direction.setAttribute('aria-label', text('Direction', '方向', isZh));
  option(direction, 'overworld-to-nether', text('Overworld → Nether', '主世界 → 下界', isZh));
  option(direction, 'nether-to-overworld', text('Nether → Overworld', '下界 → 主世界', isZh));
  direction.onchange = () => actions.direction(direction.value as PortalDirection);
  const origin = el('select');
  origin.setAttribute('aria-label', text('Origin location', '起点地点', isZh));
  origin.onchange = () => actions.origin(origin.value);
  const search = el('input');
  search.type = 'search';
  search.placeholder = text('Find candidate locations', '查找候选地点', isZh);
  search.setAttribute('aria-label', search.placeholder);
  search.oninput = () => actions.search(search.value);
  controls.append(direction, origin, search);
  const assumptions = el('p', text('X/Z geometry only; actual links depend on game version and nearby portals.', '仅比较 X/Z 几何位置；实际连接受游戏版本和附近传送门影响。', isZh), 'portal-assumption');
  const body = el('div', '', 'portal-body');
  const candidates = el('div', '', 'portal-candidates');
  const map = el('div', '', 'portal-map');
  body.append(candidates, map);
  root.append(heading, controls, assumptions, body);

  function render(book: Book, selectedOrigin: string, selectedTarget: string, query: string, unsaved: boolean) {
    const dir = direction.value as PortalDirection;
    const from = portalOriginLocations(book.locations, dir);
    const retainedOrigin = from.some(item => item.id === selectedOrigin) ? selectedOrigin : from[0]?.id ?? '';
    direction.value = dir;
    origin.replaceChildren();
    if (!from.length) option(origin, '', text('No saved origin locations', '没有已保存的起点地点', isZh));
    for (const location of from) option(origin, location.id, `${location.name} · ${coordinates(location)}`);
    origin.value = retainedOrigin;
    origin.disabled = from.length === 0;
    search.disabled = from.length === 0;
    const ranked = portalCandidates(book.locations, retainedOrigin, dir);
    const needle = query.trim().toLocaleLowerCase();
    const rows = ranked.filter(({ location }) => `${location.name} ${location.note} ${coordinates(location)}`.toLocaleLowerCase().includes(needle));
    const active = rows.find(item => item.location.id === selectedTarget) ?? rows[0];
    candidates.replaceChildren();
    map.replaceChildren();
    map.hidden = true;
    if (unsaved) candidates.append(el('p', text('Save edited locations to include them here.', '保存地点修改后，规划器才会使用新坐标。', isZh), 'portal-unsaved'));
    if (!from.length) {
      candidates.append(el('p', text('Save an Overworld or Nether location in the book first.', '请先在坐标簿中保存主世界或下界地点。', isZh), 'empty'));
      return;
    }
    if (!ranked.length) {
      candidates.append(el('p', text('No saved locations in the other dimension.', '另一维度还没有已保存地点。', isZh), 'empty'));
      return;
    }
    if (!rows.length) {
      candidates.append(el('p', text('No matching locations.', '没有匹配的地点。', isZh), 'empty'));
      return;
    }
    for (const item of rows) {
      const row = button('', () => actions.select(item.location.id), 'portal-candidate', 'portal-candidate');
      row.setAttribute('aria-pressed', String(item.location.id === active?.location.id));
      const title = el('strong', item.location.name);
      const values = el('span', `${coordinates(item.location)} · ${text('offset', '偏差', isZh)} ${item.horizontalOffset.toFixed(1)}`);
      row.append(title, values);
      candidates.append(row);
    }
    if (active) {
      map.hidden = false;
      map.append(drawPlot(ranked, active.location.id, text('Projected location and saved candidates', '换算位置与已保存候选地点', isZh), isZh));
      const summary = el('div', '', 'portal-summary');
      const distance = el('p', `${text('Horizontal X/Z offset', '水平 X/Z 偏差', isZh)}: ${active.horizontalOffset.toFixed(1)} ${text('blocks', '格', isZh)}`);
      const deltaY = el('p', active.yDelta === null
        ? text('Y comparison unavailable; one location has no saved height.', '无法比较 Y；至少一个地点未保存高度。', isZh)
        : `${text('Manual Y difference', '手动 Y 高度差', isZh)}: ${active.yDelta > 0 ? '+' : ''}${active.yDelta} ${text('(not used for ranking)', '（不参与排序）', isZh)}`);
      const open = button(text('Open saved location', '打开已保存地点', isZh), () => actions.open(active.location), 'open-candidate', 'secondary');
      summary.append(distance, deltaY, open);
      map.append(summary);
    }
  }
  return { root, direction, origin, search, render };
}

function drawPlot(ranked: ReturnType<typeof portalCandidates>, selectedId: string, label: string, isZh: boolean): SVGSVGElement {
  const namespace = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(namespace, 'svg');
  svg.setAttribute('viewBox', '0 0 420 220');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', label);
  const points = ranked.map(item => ({ x: item.location.x, z: item.location.z }));
  const projected = ranked[0]?.projected;
  if (!projected) return svg;
  points.push(projected);
  const xs = points.map(point => point.x), zs = points.map(point => point.z);
  const minX = Math.min(...xs), maxX = Math.max(...xs), minZ = Math.min(...zs), maxZ = Math.max(...zs);
  const px = (x: number) => 24 + (x - minX) / Math.max(1, maxX - minX) * 372;
  const py = (z: number) => 24 + (maxZ - z) / Math.max(1, maxZ - minZ) * 152;
  const line = document.createElementNS(namespace, 'line');
  line.setAttribute('x1', String(px(projected.x)));
  line.setAttribute('y1', String(py(projected.z)));
  const selected = ranked.find(item => item.location.id === selectedId);
  line.setAttribute('x2', String(px(selected?.location.x ?? projected.x)));
  line.setAttribute('y2', String(py(selected?.location.z ?? projected.z)));
  line.setAttribute('class', 'portal-link-line');
  svg.append(line);
  const projectedDot = document.createElementNS(namespace, 'polygon');
  const centerX = px(projected.x), centerY = py(projected.z);
  projectedDot.setAttribute('points', `${centerX},${centerY - 7} ${centerX + 7},${centerY} ${centerX},${centerY + 7} ${centerX - 7},${centerY}`);
  projectedDot.setAttribute('class', 'portal-projected-dot');
  svg.append(projectedDot);
  for (const item of ranked) {
    const dot = document.createElementNS(namespace, 'circle');
    dot.setAttribute('cx', String(px(item.location.x)));
    dot.setAttribute('cy', String(py(item.location.z)));
    dot.setAttribute('r', item.location.id === selectedId ? '6' : '4');
    dot.setAttribute('class', item.location.id === selectedId ? 'portal-selected-dot' : 'portal-candidate-dot');
    svg.append(dot);
  }
  const legend = document.createElementNS(namespace, 'text');
  legend.setAttribute('x', '20'); legend.setAttribute('y', '204'); legend.setAttribute('class', 'portal-plot-legend');
  legend.textContent = isZh ? '● 已保存地点    ◆ 换算 X/Z' : '● saved location    ◆ projected X/Z';
  svg.append(legend);
  return svg;
}
