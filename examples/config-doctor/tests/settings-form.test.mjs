// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import Ajv2020 from '../../../apps/desktop/node_modules/ajv/dist/2020.js';

test('standard JSON Schema fills new defaults without replacing stored format choices', () => {
  const manifest = JSON.parse(readFileSync(new URL('../plugin.json', import.meta.url), 'utf8'));
  const schema = manifest.contributions.settings.form.schema;
  const validate = new Ajv2020({useDefaults:true,strict:true}).compile(schema);
  const firstRun = {};
  assert.equal(validate(firstRun),true);
  assert.deepEqual(firstRun,{format:'auto',warn_duplicate_properties:true});
  const legacy = {format:'toml'};
  assert.equal(validate(legacy),true);
  assert.deepEqual(legacy,{format:'toml',warn_duplicate_properties:true});
  const intentionalDuplicates = {format:'properties',warn_duplicate_properties:false};
  assert.equal(validate(intentionalDuplicates),true);
  assert.equal(intentionalDuplicates.warn_duplicate_properties,false);
  assert.equal(validate({format:'invented',warn_duplicate_properties:true}),false);
  assert.equal(validate({format:'auto',warn_duplicate_properties:'false'}),false);
  assert.equal(validate({format:'auto',warn_duplicate_properties:true,undeclared:true}),false);
});
