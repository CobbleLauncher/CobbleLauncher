// SPDX-License-Identifier: MIT
import type { Client, DataScope } from '../../../../packages/plugin-web-sdk/src/index';
import { DOCUMENT_KEY, decodeBook, draftOf, emptyDraft, draftDirty, locationOf, upsert, without, type Book, type Draft } from './model.ts';
function newId() {
    return Array.from(crypto.getRandomValues(new Uint8Array(16)), value => value.toString(16).padStart(2, '0')).join('');
}
export class BookEditor {
    book: Book = { schemaVersion: 1, locations: [] };
    revision: string | null = null;
    scope: DataScope = {};
    draft: Draft = emptyDraft();
    baseline: Draft = emptyDraft();
    loaded = false;
    private epoch = 0;
    private readonly data: Client['data'];
    private readonly createId: () => string;
    constructor(data: Client['data'], createId: () => string = newId) {
        this.data = data;
        this.createId = createId;
    }
    get dirty() {
        return draftDirty(this.draft, this.baseline);
    }
    select(id: string | null) {
        const location = this.book.locations.find(item => item.id === id);
        this.draft = location ? draftOf(location) : emptyDraft();
        this.baseline = { ...this.draft };
    }
    edit(patch: Partial<Draft>) {
        this.draft = { ...this.draft, ...patch };
    }
    prefill(draft: Draft) {
        this.draft = { ...draft, id: '' };
        this.baseline = emptyDraft();
    }
    async load(scope: DataScope, preserveDraft = false) {
        const current = ++this.epoch;
        if (JSON.stringify(scope) !== JSON.stringify(this.scope))
            this.loaded = false;
        const response = await this.data.read({ scope, key: DOCUMENT_KEY });
        const book = decodeBook(response.value);
        if (current !== this.epoch)
            return false;
        this.scope = { ...scope };
        this.book = book;
        this.revision = response.revision;
        this.loaded = true;
        if (!preserveDraft)
            this.select(book.locations[0]?.id ?? null);
        return true;
    }
    async save() {
        if (!this.loaded)
            throw Error('BOOK_NOT_LOADED');
        const location = locationOf(this.draft, this.draft.id || this.createId());
        if (!location)
            throw Error('BOOK_DRAFT_INVALID');
        const response = await this.data.write({ scope: this.scope, key: DOCUMENT_KEY, value: upsert(this.book, location), expectedRevision: this.revision });
        const book = decodeBook(response.value);
        this.book = book;
        this.revision = response.revision;
        this.select(location.id);
        return location;
    }
    async remove() {
        if (!this.loaded || !this.draft.id)
            throw Error('BOOK_NOT_LOADED');
        const next = without(this.book, this.draft.id);
        if (next.locations.length === 0) {
            await this.data.remove({ scope: this.scope, key: DOCUMENT_KEY, expectedRevision: this.revision });
            this.book = next;
            this.revision = null;
        }
        else {
            const response = await this.data.write({ scope: this.scope, key: DOCUMENT_KEY, value: next, expectedRevision: this.revision });
            this.book = decodeBook(response.value);
            this.revision = response.revision;
        }
        this.select(this.book.locations[0]?.id ?? null);
    }
}
