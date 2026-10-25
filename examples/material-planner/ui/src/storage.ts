// SPDX-License-Identifier: MIT
import type { Client, DataScope, JsonValue } from '../../../../packages/plugin-web-sdk/src/index';
import { decodeIndex, MAX_PLANS, validatePlan, type Material, type Plan, type PlanIndex } from './model.ts';
const INDEX_KEY = 'plans-v1';
const CHUNK_BYTES = 60 * 1024;
type Ref = { key: string; revision: string };
type Manifest = { schemaVersion: 1; id: string; name: string; source: string; generation: string; itemCount: number; chunks: Ref[]; garbage: Ref[] };
type Chunk = { schemaVersion: 1; planId: string; generation: string; ordinal: number; items: (Omit<Material, 'label'> & { label?: string })[] };
export function newId(): string { return Array.from(crypto.getRandomValues(new Uint8Array(12)), n => n.toString(16).padStart(2, '0')).join(''); }
export function encodedBytes(value: unknown): number { return new TextEncoder().encode(JSON.stringify(value)).length; }
function planKey(id: string): string { if (!/^[a-f0-9]{24}$/.test(id)) throw Error('PLAN_INVALID'); return `p-${id}`; }
function manifestOf(value: unknown, id: string): Manifest {
    const entry = value as Manifest;
    if (!entry || entry.schemaVersion !== 1 || entry.id !== id || !/^[a-f0-9]{24}$/.test(entry.generation) || !Array.isArray(entry.chunks) || !entry.chunks.length || entry.chunks.length > 2048 || !Array.isArray(entry.garbage) || !Number.isInteger(entry.itemCount) || entry.itemCount < 1 || entry.itemCount > 2048) throw Error('PLAN_INCOMPLETE');
    for (const ref of [...entry.chunks, ...entry.garbage]) if (!ref || !new RegExp(`^c-${id}-[a-f0-9]{12}-\\d{1,4}$`).test(ref.key) || typeof ref.revision !== 'string' || !ref.revision) throw Error('PLAN_INCOMPLETE');
    return entry;
}
export function splitPlan(plan: Plan, generation: string): Chunk[] {
    const chunks: Chunk[] = [];
    let chunk: Chunk = { schemaVersion: 1, planId: plan.id, generation, ordinal: 0, items: [] };
    for (const material of plan.items) {
        const { label, ...rest } = material;
        const item = { ...rest, ...(label === material.id ? {} : { label }) };
        const candidate = { ...chunk, items: [...chunk.items, item] };
        if (encodedBytes(candidate) > CHUNK_BYTES) {
            if (!chunk.items.length) throw Error('PLAN_ITEM_TOO_LARGE');
            chunks.push(chunk);
            chunk = { schemaVersion: 1, planId: plan.id, generation, ordinal: chunks.length, items: [item] };
            if (encodedBytes(chunk) > CHUNK_BYTES) throw Error('PLAN_ITEM_TOO_LARGE');
        } else chunk = candidate;
    }
    if (chunk.items.length) chunks.push(chunk);
    return chunks;
}
export class PlanRepository {
    scope: DataScope = {};
    index: PlanIndex = { schemaVersion: 1, plans: [] };
    indexRevision: string | null = null;
    private active = new Map<string, { manifest: Manifest; revision: string }>();
    private pendingIndex = new Map<string, string>();
    private deleted = new Set<string>();
    private reserved=new Set<string>();
    readonly data:Client['data'];readonly generation:()=>string;
    constructor(data:Client['data'],generation:()=>string=newId){this.data=data;this.generation=generation;}
    async loadIndex(scope: DataScope): Promise<void> {
        const response = await this.data.read({ scope, key: INDEX_KEY });
        const index = decodeIndex(response.value);
        if (JSON.stringify(scope) !== JSON.stringify(this.scope)) { this.active.clear(); this.pendingIndex.clear(); this.deleted.clear(); this.reserved.clear(); }
        this.scope = { ...scope }; this.index = index; this.indexRevision = response.revision;
    }
    async loadPlan(id: string): Promise<Plan> {
        const response = await this.data.read({ scope: this.scope, key: planKey(id) });
        const manifest = manifestOf(response.value, id);
        if (!response.revision) throw Error('PLAN_INCOMPLETE');
        const chunks: Material[][] = [];
        for (const [ordinal, ref] of manifest.chunks.entries()) {
            const document = await this.data.read({ scope: this.scope, key: ref.key });
            const chunk = document.value as unknown as Chunk;
            if (document.revision !== ref.revision || !chunk || chunk.schemaVersion !== 1 || chunk.planId !== id || chunk.generation !== manifest.generation || chunk.ordinal !== ordinal || !Array.isArray(chunk.items)) throw Error('PLAN_INCOMPLETE');
            chunks.push(chunk.items.map(item => ({ ...item, label: item.label ?? item.id })));
        }
        const plan = validatePlan({ schemaVersion: 1, id, name: manifest.name, source: manifest.source, items: chunks.flat() });
        if (plan.items.length !== manifest.itemCount) throw Error('PLAN_INCOMPLETE');
        this.active.set(id, { manifest, revision: response.revision });
        return plan;
    }
    private async updateIndex(id: string, name: string, remove = false): Promise<void> {
        const plans = this.index.plans.filter(item => item.id !== id);
        if (!remove) plans.push({ id, name });
        if (plans.length > MAX_PLANS) throw Error('PLAN_INDEX_LIMIT');
        const index: PlanIndex = { schemaVersion: 1, plans };
        const response = await this.data.write({ scope: this.scope, key: INDEX_KEY, expectedRevision: this.indexRevision, value: index });
        this.index = decodeIndex(response.value); this.indexRevision = response.revision;
    }
    private async cleanup(refs: Ref[], activeKeys: Set<string> = new Set()): Promise<Ref[]> {
        const remaining: Ref[] = [];
        for (const ref of refs) {
            if (activeKeys.has(ref.key)) continue;
            try { await this.data.remove({ scope: this.scope, key: ref.key, expectedRevision: ref.revision }); }
            catch {
                try {
                    const current = await this.data.read({ scope: this.scope, key: ref.key });
                    if (current.value !== null) remaining.push(ref);
                } catch { remaining.push(ref); }
            }
        }
        return remaining;
    }
    async save(planValue: Plan): Promise<Plan> {
        const plan = validatePlan(planValue), fingerprint = JSON.stringify(plan);
        if (this.pendingIndex.get(plan.id) === fingerprint) {
            if(this.index.plans.find(item=>item.id===plan.id)?.name!==plan.name)await this.updateIndex(plan.id, plan.name);
            this.pendingIndex.delete(plan.id);
            return plan;
        }
        const previous = this.active.get(plan.id);
        if (!previous && !this.reserved.has(plan.id) && this.index.plans.some(item => item.id === plan.id)) throw Error('PLAN_RELOAD_REQUIRED');
        if (!this.index.plans.some(item => item.id === plan.id)){await this.updateIndex(plan.id,plan.name);this.reserved.add(plan.id);}
        const garbage = previous ? await this.cleanup(previous.manifest.garbage) : [];
        const generation = this.generation(), chunks = splitPlan(plan, generation), refs: Ref[] = [];
        let publishing = false;
        try {
            for (const chunk of chunks) {
                const key = `c-${plan.id}-${generation.slice(0, 12)}-${chunk.ordinal}`;
                const response = await this.data.write({ scope: this.scope, key, expectedRevision: null, value: chunk as unknown as JsonValue });
                if (!response.revision) throw Error('PLAN_WRITE_FAILED');
                refs.push({ key, revision: response.revision });
            }
            const manifest: Manifest = { schemaVersion: 1, id: plan.id, name: plan.name, source: plan.source, generation, itemCount: plan.items.length, chunks: refs, garbage: [...garbage, ...(previous?.manifest.chunks ?? [])] };
            publishing = true;
            const response = await this.data.write({ scope: this.scope, key: planKey(plan.id), expectedRevision: previous?.revision ?? null, value: manifest });
            if (!response.revision) throw Error('PLAN_WRITE_FAILED');
            this.active.set(plan.id, { manifest, revision: response.revision });
        } catch (cause) {
            let activeKeys = new Set<string>();
            let known = !publishing;
            if (publishing) {
                try {
                    const response = await this.data.read({ scope: this.scope, key: planKey(plan.id) });
                    if (response.value !== null) {
                        const actual = manifestOf(response.value, plan.id);
                        activeKeys = new Set(actual.chunks.map(ref => ref.key));
                        if (actual.generation === generation && response.revision) { this.active.set(plan.id, { manifest: actual, revision: response.revision }); this.pendingIndex.set(plan.id, fingerprint); }
                    }
                    known = true;
                } catch { /* Unknown publication outcome must not delete possibly referenced chunks. */ }
            }
            if (known) await this.cleanup(refs, activeKeys);
            throw cause;
        }
        this.pendingIndex.set(plan.id, fingerprint);
        await this.cleanup(this.active.get(plan.id)!.manifest.garbage);
        try { if(this.index.plans.find(item=>item.id===plan.id)?.name!==plan.name)await this.updateIndex(plan.id, plan.name); }
        catch { throw Error('PLAN_INDEX_PENDING'); }
        this.pendingIndex.delete(plan.id);
        return plan;
    }
    async remove(id: string): Promise<void> {
        const current = this.active.get(id);
        if (!this.deleted.has(id)) {
            if (current) {
                await this.data.remove({ scope: this.scope, key: planKey(id), expectedRevision: current.revision });
                await this.cleanup([...current.manifest.chunks, ...current.manifest.garbage]);
            } else {
                const response = await this.data.read({ scope: this.scope, key: planKey(id) });
                if (response.value !== null) throw Error('PLAN_RELOAD_REQUIRED');
            }
            this.deleted.add(id);
        }
        await this.updateIndex(id, '', true);
        this.deleted.delete(id); this.active.delete(id); this.pendingIndex.delete(id);
    }
}
