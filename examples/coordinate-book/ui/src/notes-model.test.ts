// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import {test} from 'node:test';
import {NotesEditor, decodeNotes, NOTES_KEY} from './notes-model.ts';
const note = {id:'n1', title:'Finish bridge', body:'Bring spruce', locationId:'home', completed:false};
test('bounded notes reject invalid documents and preserve linked location IDs',()=>{
 assert.deepEqual(decodeNotes(null).notes,[]);
 assert.equal(decodeNotes({schemaVersion:1,notes:[note]}).notes[0]?.locationId,'home');
 for(const value of [{schemaVersion:2,notes:[]},{schemaVersion:1,notes:[note,note]},{schemaVersion:1,notes:[{...note,completed:'yes'}]},{schemaVersion:1,notes:[{...note,body:'x'.repeat(2001)}]}])assert.throws(()=>decodeNotes(value));
});
test('private notes use game/world scope and optimistic revisions; conflicts retain draft',async()=>{
 const docs=new Map(),calls=[]; let conflict=false;
 const data={read:async(args)=>{calls.push(args);return docs.get(JSON.stringify(args.scope))??{value:null,revision:null};},write:async(args)=>{calls.push(args);if(conflict)throw Error('PLUGIN_DATA_CONFLICT');const result={value:args.value,revision:'r1'};docs.set(JSON.stringify(args.scope),result);return result;}};
 const editor=new NotesEditor(data as never);await editor.load({gameId:'a',worldId:'one'});editor.draft={...note,id:''};await editor.save();assert.equal(editor.dirty,false);assert.equal(calls.at(-1).key,NOTES_KEY);assert.deepEqual(calls.at(-1).scope,{gameId:'a',worldId:'one'});
 editor.draft.body='Unsaved';conflict=true;await assert.rejects(editor.save(),/CONFLICT/);assert.equal(editor.dirty,true);assert.equal(editor.draft.body,'Unsaved');await editor.load({gameId:'a',worldId:'one'},true);assert.equal(editor.draft.body,'Unsaved');conflict=false;editor.draft.completed=true;await editor.save();assert.equal(editor.document.notes[0]?.completed,true);
 await editor.load({gameId:'a',worldId:'two'});assert.equal(editor.document.notes.length,0);await editor.load({gameId:'a',worldId:'one'});await editor.remove();assert.equal(editor.document.notes.length,0);
});
