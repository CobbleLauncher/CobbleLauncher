// SPDX-License-Identifier: MIT
import { describe, expect, it } from 'vitest';
import { Ajv2020 } from 'ajv/dist/2020';
import addFormats from 'ajv-formats';
import schema from '../../../contracts/schemas/types.schema.json';
import instance from '../../../contracts/examples/instance.json';
import lock from '../../../contracts/examples/lock.json';
import plan from '../../../contracts/examples/plan.json';
import plugin from '../../../contracts/examples/plugin.json';
import event from '../../../contracts/examples/event.json';
import type { InstanceSpec, InstanceLock, InstallPlan, PluginManifest, TaskEvent } from '../../../contracts/generated/models';

const validators = new Ajv2020({ strict: false, validateFormats: true });
addFormats(validators);
validators.addSchema(schema);
const examples = { instance, lock, plan, plugin, event };
const typed = [instance as InstanceSpec, lock as InstanceLock, plan as InstallPlan,
  plugin as PluginManifest, event as TaskEvent];
void typed;

describe('canonical persistence contracts in TypeScript', () => {
  for (const [name, example] of Object.entries(examples)) {
    const validate = validators.compile({ $ref: `${schema.$id}#/$defs/${name}` });
    it(`${name} accepts its original fixture and rejects every omitted required field`, () => {
      expect(validate(example), JSON.stringify(validate.errors)).toBe(true);
      for (const key of Object.keys(example)) {
        const missing: Record<string, unknown> = { ...example };
        delete missing[key];
        expect(validate(missing), `missing ${key}`).toBe(false);
      }
    });
    it(`${name} rejects additional secret fields instead of stripping them`, () => {
      const invalid = { ...example, access_token: 'never accepted' };
      expect(validate(invalid)).toBe(false);
      expect(invalid).toHaveProperty('access_token', 'never accepted');
    });
  }

  it('preserves required-null semantics and safe JSON integer limits', () => {
    const validate = validators.compile({ $ref: `${schema.$id}#/$defs/instance` });
    expect(validate({ ...instance, account_id: null })).toBe(true);
    expect(validate({ ...instance, memory_mib: 512.0 })).toBe(true);
    expect(validate({ ...instance, memory_mib: 511 })).toBe(false);
    expect(validate({ ...instance, memory_mib: 512.5 })).toBe(false);
    expect(validate({ ...instance, instance_id: '../outside' })).toBe(false);
    const task = validators.compile({ $ref: `${schema.$id}#/$defs/event` });
    expect(task({ ...event, completed_bytes: Number.MAX_SAFE_INTEGER })).toBe(true);
    expect(task({ ...event, completed_bytes: Number.MAX_SAFE_INTEGER + 1 })).toBe(false);
  });
});
