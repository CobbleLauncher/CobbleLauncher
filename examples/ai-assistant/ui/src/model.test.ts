// SPDX-License-Identifier: MIT
import {test} from 'node:test';
import {strict as assert} from 'node:assert';
import {localEndpoint,prepareMessages,validEndpoint} from './model.ts';

test('chat request bounds messages and UTF-8 text',()=>{
  const old=Array.from({length:24},(_,index)=>({role:index%2?'assistant':'user',content:`turn ${index}`} as const));
  const result=prepareMessages(old,'  next  ');
  assert.equal(result.messages.length,20);
  assert.equal(result.messages.at(-1)?.content,'next');
  assert.equal(result.omitted,5);
  assert.throws(()=>prepareMessages([],'🙂'.repeat(5000)),/AI_MESSAGE_INVALID/);
  const oversizedReply=prepareMessages([{role:'user',content:'old'},{role:'assistant',content:'a'.repeat(16385)}],'next');
  assert.deepEqual(oversizedReply.messages,[{role:'user',content:'next'}]);
  assert.equal(oversizedReply.omitted,2);
});
test('endpoint accepts a custom absolute chat path without embedded credentials',()=>{
  assert.equal(validEndpoint('https://api.example.com/v1/chat/completions'),true);
  assert.equal(validEndpoint('https://api.example.com/chat'),true);
  assert.equal(validEndpoint('http://127.0.0.1:8080/v1/chat/completions'),true);
  assert.equal(localEndpoint('http://127.0.0.1:8080/v1/chat/completions'),true);
  assert.equal(validEndpoint('http://example.com/v1/chat/completions'),false);
  assert.equal(validEndpoint('https://example.com/'),false);
  assert.equal(validEndpoint('https://secret@example.com/v1/chat/completions'),false);
  assert.equal(validEndpoint('https://example.com/v1/chat/completions#key'),false);
});
