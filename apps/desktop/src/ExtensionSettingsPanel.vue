<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { localizeFormErrors, type SettingsFormError } from './extensions/forms/settings-form';
import SettingsSection from './ui/settings/SettingsSection.vue';
import SettingsDisclosure from './ui/settings/SettingsDisclosure.vue';
import { computed, defineAsyncComponent, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui';
import { locale, t } from './i18n';
import SettingsActions from './ui/settings/SettingsActions.vue';
import SettingsRow from './ui/settings/SettingsRow.vue';
import type { ExtensionJsonValue, ExtensionSettingField, ExtensionSettingsSnapshot } from './extensions/settings-types';

const SchemaSettingsForm = defineAsyncComponent(() => import('./extensions/forms/SchemaSettingsForm.vue'));

const props = defineProps<{ pluginId: string; instanceId?: string; categoryId?: string; scope?: 'global' | 'instance' }>();
const emit = defineEmits<{ busy: [value: boolean] }>();
const formErrors = ref<SettingsFormError[]>([]);
const localizedFormErrors = computed(() => localizeFormErrors(formErrors.value));
const snapshot = ref<ExtensionSettingsSnapshot | null>(null);
const savedValues = ref<Record<string, ExtensionJsonValue>>({});
const draftValues = ref<Record<string, ExtensionJsonValue>>({});
const loading = ref(false);
const saving = ref(false);
const yamlBusy = ref(false);
const yamlDialog = ref<'import' | 'export' | null>(null);
const yamlText = ref('');
const yamlError = ref('');
const yamlStatus = ref('');
const importedDraft = ref(false);
const immediateFieldId = ref<string | null>(null);
const error = ref('');
const leavePrompt = ref(false);
let contextVersion = 0;
let localeReloadPending = false;
function reloadAfterWrite() {
  if (localeReloadPending && !saving.value && !immediateFieldId.value) {
    localeReloadPending = false;
    void loadSettings(true);
  }
}
let leaveResolver: ((value: boolean) => void) | null = null;

function commandLocale() { return locale.value === 'en-US' ? 'en' : 'zh-CN'; }
function fieldValue(field: ExtensionSettingField, source = savedValues.value): ExtensionJsonValue {
  const value = source[field.id];
  return value === undefined ? field.default : value;
}
function draftValue(field: ExtensionSettingField): ExtensionJsonValue {
  const value = draftValues.value[field.id];
  return value === undefined ? fieldValue(field) : value;
}
function sameValue(left: unknown, right: unknown) { return JSON.stringify(left) === JSON.stringify(right); }
function editable(field: ExtensionSettingField) { return (!props.scope || field.scope === props.scope) && (field.scope === 'global' || Boolean(props.instanceId)); }
const categories = computed(() => (snapshot.value?.categories ?? [])
  .filter(category => !props.categoryId || category.id === props.categoryId)
  .map(category => ({ ...category, fields: category.fields.filter(editable) }))
  .filter(category => category.fields.length > 0));
const fields = computed(() => categories.value.flatMap(category => category.fields));
const draftFields = computed(() => fields.value.filter(field => field.type === 'string' || field.type === 'number'));
const changedDraftFields = computed(() => (importedDraft.value ? fields.value : draftFields.value).filter(field => !sameValue(draftValue(field), fieldValue(field))));
const form = computed(() => {
  const definition = snapshot.value?.form;
  return definition && (!props.scope || definition.scope === props.scope) && (definition.scope === 'global' || props.instanceId) ? definition : undefined;
});
const dirty = computed(() => form.value ? !sameValue(draftValues.value, savedValues.value) : changedDraftFields.value.length > 0);
watch(() => loading.value || saving.value || !!immediateFieldId.value || yamlBusy.value, value => emit('busy', value), { immediate: true });

function fieldError(field: ExtensionSettingField): string | undefined {
  if (!changedDraftFields.value.includes(field)) return undefined;
  const value = draftValue(field);
  if (field.type === 'string' && (typeof value !== 'string' || (field.maxLength !== undefined && value.length > field.maxLength))) return t('extensionSettings.lengthInvalid');
  if (field.type === 'number' && (typeof value !== 'number' || !Number.isFinite(value) || (field.minimum !== undefined && value < field.minimum) || (field.maximum !== undefined && value > field.maximum))) return t('extensionSettings.rangeInvalid');
  return undefined;
}
const invalidDrafts = computed(() => changedDraftFields.value.some(field => fieldError(field)));

function responseMatches(response: ExtensionSettingsSnapshot, pluginId: string, instanceId?: string) {
  return response.pluginId === pluginId && response.instanceId === instanceId;
}
async function loadSettings(preserveDrafts: boolean) {
  const version = ++contextVersion;
  const oldDrafts = preserveDrafts ? { ...draftValues.value } : {};
  const preserveImported = preserveDrafts && importedDraft.value;
  if (!preserveDrafts) {
    snapshot.value = null;
    savedValues.value = {};
    draftValues.value = {};
    importedDraft.value = false;
    yamlStatus.value = '';
    yamlDialog.value = null;
  }
  if (!preserveDrafts) formErrors.value = [];
  loading.value = true;
  saving.value = false;
  yamlBusy.value = false;
  immediateFieldId.value = null;
  error.value = '';
  const pluginId = props.pluginId;
  const instanceId = props.instanceId;
  const args = { pluginId, locale: commandLocale(), ...(instanceId ? { instanceId } : {}) };
  try {
    const response = await invoke<ExtensionSettingsSnapshot>('get_extension_settings', args);
    if (version !== contextVersion) return;
    if (!responseMatches(response, pluginId, instanceId)) throw new Error('invalid extension settings response');
    snapshot.value = response;
    savedValues.value = { ...response.values };
    const editableDrafts = Object.fromEntries(response.categories.flatMap(category => category.fields)
      .filter(field => editable(field) && (preserveImported || field.type === 'string' || field.type === 'number') && Object.prototype.hasOwnProperty.call(oldDrafts, field.id))
      .map(field => [field.id, oldDrafts[field.id]!]));
    draftValues.value = { ...response.values, ...(response.form && preserveDrafts ? oldDrafts : editableDrafts) };
    importedDraft.value = preserveImported;
  } catch {
    if (version === contextVersion) error.value = t('extensionSettings.loadFailed');
  } finally {
    if (version === contextVersion) loading.value = false;
  }
}

watch(
  () => [props.pluginId, props.instanceId, props.categoryId, locale.value] as const,
  ([pluginId, instanceId, categoryId, language], previous) => {
    const preserveDrafts = Boolean(previous && pluginId === previous[0] && instanceId === previous[1] && categoryId === previous[2] && language !== previous[3]);
    if (preserveDrafts && (saving.value || immediateFieldId.value)) { localeReloadPending = true; return; }
    localeReloadPending = false;
    void loadSettings(preserveDrafts);
  },
  { immediate: true, flush: 'sync' },
);
onUnmounted(() => { contextVersion += 1; leaveResolver?.(false); });

function fieldId(field: ExtensionSettingField) { return `extension-${props.pluginId}-${field.id}`; }
function updateDraft(field: ExtensionSettingField, value: ExtensionJsonValue) { draftValues.value = { ...draftValues.value, [field.id]: value }; error.value = ''; }
function inputString(field: ExtensionSettingField, event: Event) { updateDraft(field, (event.target as HTMLInputElement).value); }
function inputNumber(field: ExtensionSettingField, event: Event) {
  const input = event.target as HTMLInputElement;
  updateDraft(field, input.value === '' ? '' : input.valueAsNumber);
}
async function saveField(field: ExtensionSettingField, value: ExtensionJsonValue): Promise<boolean> {
  const version = contextVersion;
  const pluginId = props.pluginId;
  const instanceId = props.instanceId;
  immediateFieldId.value = field.id;
  error.value = '';
  const args = {
    pluginId,
    fieldId: field.id,
    value,
    locale: commandLocale(),
    ...(field.scope === 'instance' && instanceId ? { instanceId } : {}),
  };
  try {
    const response = await invoke<ExtensionSettingsSnapshot>('set_extension_setting', args);
    if (version !== contextVersion) return false;
    if (response.pluginId !== pluginId || (field.scope === 'instance' && response.instanceId !== instanceId)) throw new Error('invalid extension settings response');
    const responseValue = response.values[field.id];
    const persisted = responseValue === undefined ? value : responseValue;
    savedValues.value = { ...savedValues.value, [field.id]: persisted };
    draftValues.value = { ...draftValues.value, [field.id]: persisted };
    return true;
  } catch {
    if (version === contextVersion) error.value = t('extensionSettings.saveFailed');
    return false;
  } finally {
    if (version === contextVersion) { immediateFieldId.value = null; reloadAfterWrite(); }
  }
}
function changeBoolean(field: ExtensionSettingField, event: Event) {
  const value = (event.target as HTMLInputElement).checked;
  if (importedDraft.value) updateDraft(field, value);
  else void saveField(field, value);
}
function decodeOption(event: Event) {
  try { return JSON.parse((event.target as HTMLSelectElement).value) as ExtensionJsonValue; }
  catch { return null; }
}
function changeEnum(field: ExtensionSettingField, event: Event) {
  const value = decodeOption(event);
  if (importedDraft.value) updateDraft(field, value);
  else void saveField(field, value);
}

function updateForm(values: Record<string, unknown>) {
  if (sameValue(values, draftValues.value)) return;
  draftValues.value = values as Record<string, ExtensionJsonValue>;
  formErrors.value = [];
  error.value = '';
}
function cancelDrafts() {
  formErrors.value = [];
  draftValues.value = { ...savedValues.value };
  importedDraft.value = false;
  yamlStatus.value = '';
  error.value = '';
}
function closeYamlDialog() { yamlDialog.value = null; yamlError.value = ''; }
async function applyDrafts(): Promise<boolean> {
  if (!dirty.value) return true;
  if (saving.value || immediateFieldId.value || loading.value) return false;
  if (invalidDrafts.value) return false;
  const version = contextVersion;
  const pluginId = props.pluginId;
  const localeValue = commandLocale();
  saving.value = true;
  error.value = '';
  try {
    if (form.value) {
      const result = await invoke<{ snapshot: ExtensionSettingsSnapshot | null; errors: SettingsFormError[] }>('save_extension_settings_form', {
        pluginId, values: draftValues.value, locale: localeValue,
        ...(form.value.scope === 'instance' && props.instanceId ? { instanceId: props.instanceId } : {}),
      });
      if (version !== contextVersion) return false;
      formErrors.value = result.errors;
      if (!result.snapshot) return false;
      if (!responseMatches(result.snapshot, pluginId, form.value.scope === 'instance' ? props.instanceId : undefined)) throw new Error('invalid form response');
      snapshot.value = result.snapshot;
      savedValues.value = { ...result.snapshot.values };
      draftValues.value = { ...result.snapshot.values };
    }
    for (const field of changedDraftFields.value) {
      const value = draftValue(field);
      const instanceId = props.instanceId;
      const args = {
        pluginId,
        fieldId: field.id,
        value,
        locale: localeValue,
        ...(field.scope === 'instance' && instanceId ? { instanceId } : {}),
      };
      const response = await invoke<ExtensionSettingsSnapshot>('set_extension_setting', args);
      if (version !== contextVersion) return false;
      if (response.pluginId !== pluginId || (field.scope === 'instance' && response.instanceId !== instanceId)) throw new Error('invalid extension settings response');
      const responseValue = response.values[field.id];
      const persisted = responseValue === undefined ? value : responseValue;
      savedValues.value = { ...savedValues.value, [field.id]: persisted };
      draftValues.value = { ...draftValues.value, [field.id]: persisted };
    }
    importedDraft.value = false;
    yamlStatus.value = '';
    return true;
  } catch {
    if (version === contextVersion) error.value = t(form.value ? 'extensionSettings.formApplyFailed' : 'extensionSettings.applyFailed');
    return false;
  } finally {
    if (version === contextVersion) { saving.value = false; reloadAfterWrite(); }
  }
}

function yamlContext() {
  return { pluginId: props.pluginId, locale: commandLocale(), ...(props.instanceId ? { instanceId: props.instanceId } : {}) };
}
function openYamlImport() {
  yamlText.value = '';
  yamlError.value = '';
  yamlDialog.value = 'import';
}
async function openYamlExport() {
  if (loading.value || yamlBusy.value) return;
  const version = contextVersion;
  yamlBusy.value = true;
  yamlError.value = '';
  try {
    const value = await invoke<string>('export_extension_settings_yaml', yamlContext());
    if (version !== contextVersion) return;
    if (typeof value !== 'string' || value.length > 65536) throw new Error('invalid YAML export');
    yamlText.value = value;
    yamlDialog.value = 'export';
  } catch {
    if (version === contextVersion) yamlError.value = t('extensionSettings.yamlExportFailed');
  } finally {
    if (version === contextVersion) yamlBusy.value = false;
  }
}
function validImportedValue(field: ExtensionSettingField, value: unknown): value is ExtensionJsonValue {
  if (field.type === 'boolean') return typeof value === 'boolean';
  if (field.type === 'enum') return field.options.some(option => sameValue(option.value, value as ExtensionJsonValue));
  if (field.type === 'string') return typeof value === 'string' && (field.maxLength === undefined || value.length <= field.maxLength);
  return typeof value === 'number' && Number.isFinite(value) && (field.minimum === undefined || value >= field.minimum) && (field.maximum === undefined || value <= field.maximum);
}
async function previewYamlImport() {
  if (yamlBusy.value || loading.value || saving.value || immediateFieldId.value || !yamlText.value.trim()) return;
  const version = contextVersion;
  yamlBusy.value = true;
  yamlError.value = '';
  try {
    const result = await invoke<{ values: Record<string, unknown> }>('preview_extension_settings_yaml', { ...yamlContext(), yaml: yamlText.value });
    if (version !== contextVersion) return;
    if (!result || typeof result.values !== 'object' || result.values === null || Array.isArray(result.values)) throw new Error('invalid YAML preview');
    const fieldMap = new Map(fields.value.map(field => [field.id, field]));
    const imported = Object.entries(result.values);
    if (!imported.length || imported.some(([id, value]) => form.value
      ? !Object.hasOwn(form.value.schema.properties ?? {}, id) || !['string', 'number', 'boolean'].includes(typeof value)
      : !fieldMap.has(id) || !validImportedValue(fieldMap.get(id)!, value))) throw new Error('invalid YAML values');
    const nextDrafts = { ...draftValues.value };
    for (const [id, value] of imported) nextDrafts[id] = value as ExtensionJsonValue;
    draftValues.value = nextDrafts;
    importedDraft.value = true;
    yamlStatus.value = t('extensionSettings.yamlImported');
    yamlDialog.value = null;
  } catch {
    if (version === contextVersion) yamlError.value = t('extensionSettings.yamlImportFailed');
  } finally {
    if (version === contextVersion) yamlBusy.value = false;
  }
}
async function copyYaml() {
  try {
    await writeText(yamlText.value);
    yamlDialog.value = null;
    yamlStatus.value = t('extensionSettings.yamlCopied');
    yamlError.value = '';
  } catch {
    yamlError.value = t('extensionSettings.yamlCopyFailed');
  }
}

function canLeave() { return !dirty.value && !saving.value && !immediateFieldId.value; }
function confirmLeave(): Promise<boolean> {
  if (saving.value || immediateFieldId.value) return Promise.resolve(false);
  if (!dirty.value) return Promise.resolve(true);
  leavePrompt.value = true;
  return new Promise(resolve => { leaveResolver?.(false); leaveResolver = resolve; });
}
function finishLeave(value: boolean) {
  leavePrompt.value = false;
  leaveResolver?.(value);
  leaveResolver = null;
}
async function applyAndLeave() { if (await applyDrafts() && !dirty.value) finishLeave(true); }
function discardAndLeave() { cancelDrafts(); finishLeave(true); }
defineExpose({ canLeave, confirmLeave });
</script>

<template>
  <section
    class="extension-settings-panel"
    :aria-busy="loading || saving || !!immediateFieldId || yamlBusy"
  >
    <p
      v-if="loading"
      class="setting-hint"
      role="status"
    >
      {{ t('extensionSettings.loading') }}
    </p>
    <p
      v-if="error"
      class="settings-error"
      role="alert"
    >
      {{ error }}
    </p>
    <SchemaSettingsForm
      v-if="form"
      :schema="form.schema"
      :ui-schema="form.ui_schema"
      :translations="form.translations?.[commandLocale()]"
      :data="draftValues"
      :disabled="loading || saving || yamlBusy"
      :errors="localizedFormErrors"
      @change="updateForm"
    />
    <SettingsSection
      v-for="category in form ? [] : categories"
      :key="category.id"
      :title="!categoryId ? category.label : undefined"
    >
      <SettingsRow
        v-for="field in category.fields.filter(item => !item.advanced)"
        :key="field.id"
        :label="field.label"
        :input-id="fieldId(field)"
        :error="fieldError(field)"
      >
        <input
          v-if="field.type === 'boolean'"
          :id="fieldId(field)"
          type="checkbox"
          :checked="draftValue(field) === true"
          :disabled="loading || saving || !!immediateFieldId"
          @change="changeBoolean(field, $event)"
        >
        <select
          v-else-if="field.type === 'enum'"
          :id="fieldId(field)"
          :value="JSON.stringify(draftValue(field))"
          :disabled="loading || saving || !!immediateFieldId"
          @change="changeEnum(field, $event)"
        >
          <option
            v-for="option in field.options"
            :key="JSON.stringify(option.value)"
            :value="JSON.stringify(option.value)"
          >
            {{ option.label }}
          </option>
        </select>
        <input
          v-else-if="field.type === 'string'"
          :id="fieldId(field)"
          type="text"
          :value="draftValue(field)"
          :maxlength="field.maxLength"
          :disabled="loading || saving || !!immediateFieldId"
          :aria-invalid="!!fieldError(field)"
          @input="inputString(field, $event)"
        >
        <input
          v-else
          :id="fieldId(field)"
          type="number"
          step="any"
          :value="typeof draftValue(field) === 'number' ? draftValue(field) : ''"
          :min="field.minimum"
          :max="field.maximum"
          :disabled="loading || saving || !!immediateFieldId"
          :aria-invalid="!!fieldError(field)"
          @input="inputNumber(field, $event)"
        >
        <p
          v-if="immediateFieldId === field.id"
          class="setting-hint"
          role="status"
        >
          {{ t('extensionSettings.saving') }}
        </p>
      </SettingsRow>
      <SettingsDisclosure
        v-if="category.fields.some(field => field.advanced)"
        :title="t('extensionSettings.advanced')"
        class="extension-settings-advanced"
      >
        <SettingsRow
          v-for="field in category.fields.filter(item => item.advanced)"
          :key="field.id"
          :label="field.label"
          :input-id="fieldId(field)"
          :error="fieldError(field)"
        >
          <input
            v-if="field.type === 'boolean'"
            :id="fieldId(field)"
            type="checkbox"
            :checked="draftValue(field) === true"
            :disabled="loading || saving || !!immediateFieldId"
            @change="changeBoolean(field, $event)"
          >
          <select
            v-else-if="field.type === 'enum'"
            :id="fieldId(field)"
            :value="JSON.stringify(draftValue(field))"
            :disabled="loading || saving || !!immediateFieldId"
            @change="changeEnum(field, $event)"
          >
            <option
              v-for="option in field.options"
              :key="JSON.stringify(option.value)"
              :value="JSON.stringify(option.value)"
            >
              {{ option.label }}
            </option>
          </select>
          <input
            v-else-if="field.type === 'string'"
            :id="fieldId(field)"
            type="text"
            :value="draftValue(field)"
            :maxlength="field.maxLength"
            :disabled="loading || saving || !!immediateFieldId"
            :aria-invalid="!!fieldError(field)"
            @input="inputString(field, $event)"
          >
          <input
            v-else
            :id="fieldId(field)"
            type="number"
            step="any"
            :value="typeof draftValue(field) === 'number' ? draftValue(field) : ''"
            :min="field.minimum"
            :max="field.maximum"
            :disabled="loading || saving || !!immediateFieldId"
            :aria-invalid="!!fieldError(field)"
            @input="inputNumber(field, $event)"
          >
          <p
            v-if="immediateFieldId === field.id"
            class="setting-hint"
            role="status"
          >
            {{ t('extensionSettings.saving') }}
          </p>
        </SettingsRow>
      </SettingsDisclosure>
    </SettingsSection>
    <div
      v-if="snapshot && (fields.length || form) && !categoryId"
      class="extension-settings-transfer"
    >
      <button
        class="text-button"
        type="button"
        :disabled="loading || saving || !!immediateFieldId || yamlBusy"
        @click="openYamlImport"
      >
        {{ t('extensionSettings.yamlImport') }}
      </button>
      <button
        class="text-button"
        type="button"
        :disabled="loading || saving || !!immediateFieldId || yamlBusy"
        @click="openYamlExport"
      >
        {{ yamlBusy ? t('extensionSettings.yamlWorking') : t('extensionSettings.yamlExport') }}
      </button>
    </div>
    <p
      v-if="yamlStatus"
      class="setting-hint"
      role="status"
    >
      {{ yamlStatus }}
    </p>
    <p
      v-if="yamlError && !yamlDialog"
      class="settings-error"
      role="alert"
    >
      {{ yamlError }}
    </p>
    <p
      v-if="snapshot && fields.length === 0 && !form && !loading"
      class="setting-hint"
    >
      {{ t('extensionSettings.noSettings') }}
    </p>
    <SettingsActions
      :dirty="dirty"
      :busy="saving"
      :disabled="loading || !!immediateFieldId || invalidDrafts || yamlBusy"
      @apply="applyDrafts"
      @cancel="cancelDrafts"
    />
    <DialogRoot
      :open="yamlDialog !== null"
      @update:open="!$event && closeYamlDialog()"
    >
      <DialogPortal>
        <DialogOverlay class="settings-dialog-overlay" />
        <DialogContent class="settings-yaml-dialog">
          <DialogTitle>{{ t(yamlDialog === 'import' ? 'extensionSettings.yamlImport' : 'extensionSettings.yamlExport') }}</DialogTitle>
          <DialogDescription class="setting-hint">
            {{ t(yamlDialog === 'import' ? 'extensionSettings.yamlImportDescription' : 'extensionSettings.yamlExportDescription') }}
          </DialogDescription>
          <textarea
            v-model="yamlText"
            :readonly="yamlDialog === 'export'"
            :maxlength="65536"
            :aria-label="t('extensionSettings.yamlTextLabel')"
            spellcheck="false"
            autocomplete="off"
          />
          <p
            v-if="yamlError"
            class="settings-error"
            role="alert"
          >
            {{ yamlError }}
          </p>
          <div class="settings-inline-actions">
            <button
              v-if="yamlDialog === 'import'"
              class="secondary-button"
              type="button"
              :disabled="yamlBusy || !yamlText.trim()"
              @click="previewYamlImport"
            >
              {{ yamlBusy ? t('extensionSettings.yamlWorking') : t('extensionSettings.yamlPreview') }}
            </button>
            <button
              v-else
              class="secondary-button"
              type="button"
              :disabled="yamlBusy || !yamlText"
              @click="copyYaml"
            >
              {{ t('extensionSettings.yamlCopy') }}
            </button>
            <button
              class="text-button"
              type="button"
              :disabled="yamlBusy"
              @click="closeYamlDialog"
            >
              {{ t('settingsLayout.cancel') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
    <DialogRoot
      :open="leavePrompt"
      @update:open="!$event && finishLeave(false)"
    >
      <DialogPortal>
        <DialogOverlay class="settings-dialog-overlay" />
        <DialogContent class="settings-leave-dialog">
          <DialogTitle>{{ t('settingsLayout.discardTitle') }}</DialogTitle>
          <p
            v-if="error"
            class="settings-error"
            role="alert"
          >
            {{ error }}
          </p>
          <div class="settings-inline-actions">
            <button
              class="secondary-button"
              :disabled="saving || invalidDrafts"
              @click="applyAndLeave"
            >
              {{ t('settingsLayout.apply') }}
            </button>
            <button
              class="text-button"
              :disabled="saving"
              @click="discardAndLeave"
            >
              {{ t('settingsLayout.discard') }}
            </button>
            <button
              class="text-button"
              :disabled="saving"
              @click="finishLeave(false)"
            >
              {{ t('settingsLayout.continueEditing') }}
            </button>
          </div>
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
  </section>
</template>

<style scoped>
.extension-settings-heading{margin:20px 0 0;font-size:16px;font-weight:600}
.extension-settings-advanced{margin-top:18px}
.extension-settings-transfer{display:flex;gap:14px;flex-wrap:wrap;margin-top:14px}
.settings-yaml-dialog{position:fixed;left:50%;top:50%;transform:translate(-50%,-50%);width:min(620px,calc(100vw - 32px));max-height:calc(100vh - 32px);overflow:auto;padding:22px;border:1px solid var(--line);border-radius:14px;background:var(--panel);z-index:101}
.settings-yaml-dialog h2{font-size:16px;margin:0 0 14px}
.settings-yaml-dialog textarea{box-sizing:border-box;width:100%;min-height:220px;resize:vertical;font:12px/1.5 ui-monospace,Consolas,monospace;overflow-wrap:anywhere}
.settings-yaml-dialog .settings-inline-actions{margin-top:14px}
</style>
