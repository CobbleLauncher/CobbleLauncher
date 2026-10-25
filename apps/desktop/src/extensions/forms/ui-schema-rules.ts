// SPDX-License-Identifier: MIT
import type { UISchemaElement } from '@jsonforms/core';
/** Convert validated standard const rules to JSON Forms' own no-eval leaf condition. */
export function schemaUiRules(ui?: UISchemaElement): UISchemaElement | undefined {
  if (!ui) return undefined;
  function adapt(element: UISchemaElement): UISchemaElement {
    const result = { ...element } as UISchemaElement & { elements?: UISchemaElement[] };
    if (element.rule) {
      const condition = element.rule.condition as { scope?: unknown; schema?: { const?: unknown } };
      if (!['SHOW','HIDE'].includes(element.rule.effect) || typeof condition.scope !== 'string'
        || !condition.scope.startsWith('#/properties/') || Object.keys(condition).some(key => !['scope','schema'].includes(key))
        || !condition.schema || Object.keys(condition.schema).length !== 1 || !Object.hasOwn(condition.schema,'const')
        || !['string','number','boolean'].includes(typeof condition.schema.const)) throw new Error('Unsupported form rule');
      result.rule = { effect: element.rule.effect, condition: { type:'LEAF',scope:condition.scope,expectedValue:condition.schema.const } };
    }
    if (Array.isArray(result.elements)) result.elements = result.elements.map(adapt);
    return result;
  }
  return adapt(ui);
}
