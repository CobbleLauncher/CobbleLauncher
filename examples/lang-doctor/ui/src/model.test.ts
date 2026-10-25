// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultLocales, issuesOf, metadataOf, sourceLines, visibleIssues, reportText } from './model.ts';
const report = {schema_version:1,findings:[],tables:[{columns:['reference_keys','target_keys','issue_count'],rows:[['3','2','1']]},{columns:['file','line','kind','key','reference','translation','message'],rows:[['file-0','3','missing_key','missing','Text','','Missing']]}]};
test('machine columns retain real baseline source IDs and line numbers',()=>{
 const issues=issuesOf(report);assert.equal(issues[0]!.file,'file-0');assert.equal(issues[0]!.line,3);assert.equal(metadataOf(report).reference_keys,'3');
 assert.equal(visibleIssues(issues,'text','missing_key').length,1);assert.equal(visibleIssues(issues,'','duplicate_key').length,0);
 assert.match(reportText(report,'fixture.zip'),/file-0/);
});
test('single-file syntax and explicit comparison locales never use same reference/target',()=>{
 assert.deepEqual(defaultLocales(['zh_cn']),{baseline:'',target:'zh_cn'});
 assert.deepEqual(defaultLocales(['fr_fr','en_us','zh_cn']),{baseline:'en_us',target:'zh_cn'});
 assert.deepEqual(defaultLocales(['en_us','de_de']),{baseline:'en_us',target:'de_de'});
});
test('source snippet highlights exact source line without inventing missing lines',()=>{
 assert.deepEqual(sourceLines('one\ntwo\nthree',2).filter(line=>line.active),[{number:2,text:'two',active:true}]);
 assert.equal(sourceLines('one',99).some(line=>line.active),false);
});
