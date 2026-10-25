// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { redactLog } from './redaction.ts';
test('redacts credential headers, quoted JSON, queries and Minecraft CLI forms', () => {
  const input = ['Authorization: Bearer synthetic-bearer', 'Proxy-Authorization: Basic synthetic-basic', 'Cookie: sid=synthetic-cookie; other=secret', '{"access_token":"synthetic-json","refreshToken": "synthetic-refresh"}', '--accessToken synthetic-cli --username SyntheticPlayer --uuid 0123456789abcdef0123456789abcdef', 'https://example.invalid/?api_key=synthetic-query&other=ok', "password='synthetic spaces'"].join('\r\n');
  const output = redactLog(input);
  for (const secret of ['synthetic-bearer', 'synthetic-basic', 'synthetic-cookie', 'synthetic-json', 'synthetic-refresh', 'synthetic-cli', 'SyntheticPlayer', '0123456789abcdef0123456789abcdef', 'synthetic-query', 'synthetic spaces']) assert(!output.text.includes(secret), secret);
  assert.equal(output.text.split('\r\n').length, 7); assert(output.counts.credential >= 7);
});
test('masks account markers, emails and UUIDs without masking unrelated hashes or class names', () => {
  const input = 'Setting user: SyntheticPlayer\nUUID of player OtherPlayer is 12345678-abcd-1234-abcd-123456789abc\naccountId=Account123 xuid=987654321 email=person@example.invalid\nsha256=0123456789abcdef0123456789abcdef\nat com.example.SessionManager.load(SessionManager.java:19)';
  const output = redactLog(input).text;
  for (const secret of ['SyntheticPlayer', 'OtherPlayer', '12345678-abcd', 'Account123', '987654321', 'person@example.invalid']) assert(!output.includes(secret), secret);
  assert(output.includes('sha256=0123456789abcdef0123456789abcdef')); assert(output.includes('SessionManager.java:19'));
});
test('masks Windows, UNC and POSIX paths and addresses while retaining useful diagnostics', () => {
  const input = '[12:00:01] Loaded "C:\\Users\\Synthetic Person\\Game\\latest.log"\npath=C:\\Games\\Private\\mods\\example.jar\n\\\\private-host\\share\\world\n/home/synthetic/.minecraft/logs/latest.log\nJoining 203.0.113.10:25565 via [2001:db8::1]:25565 and 127.0.0.1\nMinecraft version 1.21.1.1\nat java.base/java.lang.Thread.run(Thread.java:840)\nCaused by: java.lang.NoClassDefFoundError: com/example/Thing\nexample-mod-1.2.3.jar';
  const output = redactLog(input).text;
  for (const secret of ['Synthetic Person', 'C:\\Games', 'private-host', '/home/synthetic', '203.0.113.10', '2001:db8::1', '127.0.0.1']) assert(!output.includes(secret), secret);
  for (const useful of ['[12:00:01]', '25565', 'Minecraft version 1.21.1.1', 'Thread.java:840', 'com/example/Thing', 'example-mod-1.2.3.jar']) assert(output.includes(useful), useful);
});
test('preserves input endings, output spans and bounded change details', () => {
  const output = redactLog('userId=one\r\nuserId=two\nunchanged');
  assert.equal(output.text, 'userId=[ACCOUNT]\r\nuserId=[ACCOUNT]\nunchanged'); assert.deepEqual(output.changes.map(change => change.line), [1, 2]);
  for (const change of output.changes) assert.equal(output.text.slice(change.start, change.end), '[ACCOUNT]');
  const dense = redactLog('userId=x\n'.repeat(3000)); assert.equal(dense.changes.length, 2048); assert.equal(dense.totalChanges, 3000); assert.equal(dense.changesTruncated, true);
  assert.equal(redactLog(output.text).text, output.text);
  assert.throws(() => redactLog('界'.repeat(350_000)), /LOG_REDACTION_INPUT_INVALID/);
});
test('covers unquoted profile directories, URL userinfo and standalone JWT shapes', () => {
  const output = redactLog('Path C:\\Users\\Synthetic Person\\.minecraft\\logs\\latest.log failed\nPath /home/Synthetic Person/.minecraft/logs/latest.log failed\nhttps://SyntheticUser:SyntheticPassword@example.invalid/file\neyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJwcml2YXRlIn0.syntheticSignature').text;
  for (const privateValue of ['Synthetic Person', 'SyntheticUser', 'SyntheticPassword', 'eyJhbGci', 'eyJzdWIi', 'syntheticSignature']) assert(!output.includes(privateValue), privateValue);
  assert(output.includes('failed')); assert(output.includes('example.invalid/file'));
});
test('masks common server player identity messages without dropping connection evidence', () => {
  const output = redactLog('[Server thread/INFO]: SyntheticPlayer[/203.0.113.10:25565] logged in with entity id 12\nSyntheticPlayer joined the game\nSyntheticPlayer lost connection: Disconnected').text;
  assert(!output.includes('SyntheticPlayer')); assert(!output.includes('203.0.113.10')); assert(output.includes('entity id 12')); assert(output.includes('Disconnected'));
});
