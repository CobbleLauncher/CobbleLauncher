// SPDX-License-Identifier: MIT
import type { Client, DataScope } from '../../../../packages/plugin-web-sdk/src/index';
export const NOTES_KEY = 'world-notes-v1';
export type WorldNote = { id: string; title: string; body: string; locationId: string; completed: boolean };
export type Notes = { schemaVersion: 1; notes: WorldNote[] };
export const emptyNote = (): WorldNote => ({ id: '', title: '', body: '', locationId: '', completed: false });
export function validNote(value: unknown): value is WorldNote {
    if (!value || typeof value !== 'object') return false;
    const n = value as WorldNote;
    return typeof n.id === 'string' && n.id.length <= 80 && typeof n.title === 'string' && !!n.title.trim() && n.title.length <= 100 && typeof n.body === 'string' && n.body.length <= 2000 && typeof n.locationId === 'string' && n.locationId.length <= 80 && typeof n.completed === 'boolean';
}
export function decodeNotes(value: unknown): Notes {
    if (value === null) return { schemaVersion: 1, notes: [] };
    const doc = value as Notes;
    if (!doc || doc.schemaVersion !== 1 || !Array.isArray(doc.notes) || doc.notes.length > 128 || doc.notes.some(n => !validNote(n) || !n.id) || new Set(doc.notes.map(n => n.id)).size !== doc.notes.length || new TextEncoder().encode(JSON.stringify(doc)).length > 64 * 1024) throw Error('NOTES_DOCUMENT_INVALID');
    return { schemaVersion: 1, notes: doc.notes.map(n => ({...n})) };
}
export class NotesEditor {
    document: Notes = { schemaVersion: 1, notes: [] };
    draft = emptyNote(); baseline = emptyNote(); scope: DataScope = {}; revision: string | null = null; loaded = false;
    private readonly data: Client['data'];
    constructor(data: Client['data']) { this.data = data; }
    get dirty() { return JSON.stringify(this.draft) !== JSON.stringify(this.baseline); }
    select(id: string | null) { this.draft = {...(this.document.notes.find(n => n.id === id) ?? emptyNote())}; this.baseline = {...this.draft}; }
    async load(scope: DataScope, preserve = false) {
        if (JSON.stringify(scope) !== JSON.stringify(this.scope)) this.loaded = false;
        const result = await this.data.read({ scope, key: NOTES_KEY });
        const document = decodeNotes(result.value);
        this.document = document; this.revision = result.revision; this.scope = {...scope}; this.loaded = true;
        if (!preserve) this.select(document.notes.find(n => !n.completed)?.id ?? document.notes[0]?.id ?? null);
    }
    async save() {
        if (!this.loaded || !validNote(this.draft)) throw Error('NOTES_DRAFT_INVALID');
        const note = {...this.draft, title: this.draft.title.trim(), id: this.draft.id || crypto.randomUUID()};
        const notes = this.document.notes.some(n => n.id === note.id) ? this.document.notes.map(n => n.id === note.id ? note : n) : [...this.document.notes, note];
        const next = {schemaVersion: 1 as const, notes};
        if (notes.length > 128 || new TextEncoder().encode(JSON.stringify(next)).length > 64 * 1024) throw Error('NOTES_DOCUMENT_LIMIT');
        await this.write(decodeNotes(next)); this.select(note.id);
    }
    async remove() {
        if (!this.loaded || !this.draft.id) throw Error('NOTES_DRAFT_INVALID');
        const next = {schemaVersion: 1 as const, notes: this.document.notes.filter(n => n.id !== this.draft.id)};
        await this.write(next); this.select(next.notes[0]?.id ?? null);
    }
    private async write(value: Notes) {
        const response = await this.data.write({ scope: this.scope, key: NOTES_KEY, value, expectedRevision: this.revision });
        this.document = decodeNotes(response.value); this.revision = response.revision;
    }
}
