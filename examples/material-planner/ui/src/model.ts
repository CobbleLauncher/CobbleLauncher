// SPDX-License-Identifier: MIT
export const MAX_ITEMS = 2048;
export const MAX_PLANS = 64;
export type Material = { id: string; label: string; required: number; collected: number; stackSize: number | null };
export type Plan = { schemaVersion: 1; id: string; name: string; source: string; items: Material[] };
export type PlanEntry = { id: string; name: string };
export type PlanIndex = { schemaVersion: 1; plans: PlanEntry[] };
export type SortOrder = 'missing' | 'required' | 'name' | 'collected';
export function integer(value: string, maximum = 1_000_000_000): number | null {
    if (!/^\d{1,10}$/.test(value.trim())) return null;
    const count = Number(value);
    return Number.isSafeInteger(count) && count >= 0 && count <= maximum ? count : null;
}
export function stackSize(value: string): number | null {
    if (!value.trim()) return null;
    const count = integer(value, 999);
    return count !== null && count > 0 ? count : null;
}
export function missing(item: Material): number { return Math.max(0, item.required - item.collected); }
export function groups(count: number, stack: number | null): { stacks: number; loose: number } | null {
    return stack === null ? null : { stacks: Math.floor(count / stack), loose: count % stack };
}
export function totals(plan: Plan) {
    return plan.items.reduce((sum, item) => ({ required: sum.required + item.required, missing: sum.missing + missing(item), complete: sum.complete + Number(missing(item) === 0), unknown: sum.unknown + Number(item.stackSize === null) }), { required: 0, missing: 0, complete: 0, unknown: 0 });
}
export function sortedItems(plan: Plan, query: string, order: SortOrder): Material[] {
    const needle = query.trim().toLocaleLowerCase();
    return plan.items.filter(item => `${item.label} ${item.id}`.toLocaleLowerCase().includes(needle)).sort((a, b) => {
        const delta = order === 'missing' ? missing(b) - missing(a) : order === 'required' ? b.required - a.required : order === 'collected' ? b.collected - a.collected : 0;
        return delta || a.label.localeCompare(b.label) || a.id.localeCompare(b.id);
    });
}
export function importedPlan(payload: { name: string; source?: string; materials: { id: string; name?: string; required: number; stackSize?: number | null }[] }): Plan {
    return validatePlan({ schemaVersion: 1, id: '', name: payload.name, source: payload.source ?? '', items: payload.materials.map(item => ({ id: item.id, label: item.name ?? item.id, required: item.required, collected: 0, stackSize: item.stackSize ?? null })) }, true);
}
// Bounded text rejects NUL/control characters from imported counts.
// eslint-disable-next-line no-control-regex
function text(value: unknown, max: number): value is string { return typeof value === 'string' && [...value].length <= max && !/[\u0000-\u001f\u007f]/u.test(value); }
export function validatePlan(value: unknown, unsaved = false): Plan {
    if (!value || typeof value !== 'object') throw Error('PLAN_INVALID');
    const plan = value as Plan;
    if (plan.schemaVersion !== 1 || typeof plan.id !== 'string' || (!unsaved && !/^[a-f0-9]{24}$/.test(plan.id)) || !text(plan.name, 80) || !plan.name.trim() || !text(plan.source, 120) || !Array.isArray(plan.items) || plan.items.length < 1 || plan.items.length > MAX_ITEMS) throw Error('PLAN_INVALID');
    const ids = new Set<string>();
    for (const item of plan.items) {
        // eslint-disable-next-line no-control-regex
        if (!item || typeof item.id !== 'string' || !item.id || new TextEncoder().encode(item.id).length > 1024 || /[\u0000-\u001f\u007f]/u.test(item.id) || !text(item.label, 1024) || !Number.isSafeInteger(item.required) || item.required < 1 || item.required > 1_000_000_000 || !Number.isSafeInteger(item.collected) || item.collected < 0 || item.collected > 1_000_000_000 || (item.stackSize !== null && (!Number.isInteger(item.stackSize) || item.stackSize < 1 || item.stackSize > 999)) || ids.has(item.id)) throw Error('PLAN_INVALID');
        ids.add(item.id);
    }
    return structuredClone(plan);
}
export function decodeIndex(value: unknown): PlanIndex {
    if (value === null) return { schemaVersion: 1, plans: [] };
    const index = value as PlanIndex;
    if (!index || index.schemaVersion !== 1 || !Array.isArray(index.plans) || index.plans.length > MAX_PLANS) throw Error('PLAN_INDEX_INVALID');
    const ids = new Set<string>();
    for (const item of index.plans) {
        if (!item || !/^[a-f0-9]{24}$/.test(item.id) || !text(item.name, 80) || !item.name.trim() || ids.has(item.id)) throw Error('PLAN_INDEX_INVALID');
        ids.add(item.id);
    }
    return structuredClone(index);
}
export function planText(plan: Plan, zh = false): string {
    const lines = [plan.name, ...(plan.source ? [plan.source] : []), zh ? '方块需求；已收集数量由玩家填写。' : 'Block requirements; collected quantities are entered by the player.', '', zh ? '材料\t需求\t已收集\t缺口\t每组数量\t缺口组数\t散件' : 'Material\tRequired\tCollected\tMissing\tStack size\tStacks missing\tLoose'];
    for (const item of plan.items) {
        const count = missing(item), grouped = groups(count, item.stackSize);
        lines.push([item.label, item.required, item.collected, count, item.stackSize ?? '?', grouped?.stacks ?? '?', grouped?.loose ?? '?'].join('\t'));
    }
    return lines.join('\n');
}
// eslint-disable-next-line no-control-regex
export function exportName(plan: Plan): string { return `${plan.name.replace(/[\\/:*?"<>|\u0000-\u001f]/gu, '-').slice(0, 70).replace(/^[. ]+|[. ]+$/gu, '') || 'material-plan'}.txt`; }
