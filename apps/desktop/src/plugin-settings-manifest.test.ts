// SPDX-License-Identifier: MIT
import {afterEach,beforeEach,expect,it} from 'vitest';
import {mount} from '@vue/test-utils';
import PluginPermissions from './PluginPermissions.vue';
import {locale,setLocale} from './i18n';
import type {CatalogManifest} from './plugin-catalogs';
import {effectivePermissionNodes} from './plugin-permissions';
import {hasGlobalPluginSettings} from './plugin-details';
import {manifestSettingFields} from './extensions/manifest-settings';
const original=locale.value;
beforeEach(()=>setLocale('en-US'));
afterEach(()=>setLocale(original));
function manifest():CatalogManifest{return {id:'doctor',name:'Doctor',version:'0.2.1',api_version:'0.2.0',publisher:'Original',capabilities:['report'],file_extensions:['toml'],component_sha256:'a'.repeat(64),contributions:{schema_version:1,settings:{form:{schema_version:1,scope:'global',schema:{type:'object',additionalProperties:false,properties:{format:{type:'string',default:'auto',enum:['auto','toml'],title:'Text file format'},warn_duplicate_properties:{type:'boolean',default:true,title:'Warn about duplicate properties'}},required:['format','warn_duplicate_properties']},translations:{'zh-CN':{'format.label':'文本文件格式','warn_duplicate_properties.label':'提示重复配置项'}}}}}};}
it('normalizes form-only permissions and settings navigation without legacy categories',()=>{const value=manifest();expect(hasGlobalPluginSettings(value)).toBe(true);expect(effectivePermissionNodes(value)).toContainEqual({permission:'launcher:settings/read',keys:['global:format','global:warn_duplicate_properties']});value.contributions!.settings!.form!.scope='instance';expect(hasGlobalPluginSettings(value)).toBe(false);});
it('renders localized form field names instead of exposing settings keys',()=>{setLocale('zh-CN');const wrapper=mount(PluginPermissions,{props:{manifest:manifest()}});expect(wrapper.text()).toContain('文本文件格式');expect(wrapper.text()).toContain('提示重复配置项');expect(wrapper.text()).not.toContain('warn_duplicate_properties');expect(wrapper.text()).not.toContain('global:format');wrapper.unmount();});
it('preserves legacy labels and falls back safely for missing metadata',()=>{const value=manifest();value.contributions!.settings={categories:[{id:'legacy',fields:[{id:'rows',scope:'global',label:{en:'Rows','zh-CN':'行数'}}]}]};expect(manifestSettingFields(value.contributions!.settings)[0]?.label?.en).toBe('Rows');expect(hasGlobalPluginSettings(value)).toBe(true);const wrapper=mount(PluginPermissions,{props:{manifest:value,nodes:[{permission:'launcher:settings/read',keys:['global:undeclared_internal_key']}]}});expect(wrapper.text()).not.toContain('undeclared_internal_key');wrapper.unmount();expect(manifestSettingFields(null)).toEqual([]);});
