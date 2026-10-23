import zhGameServers from './locales/zh-CN/gameServers.json';
import enGameServers from './locales/en-US/gameServers.json';
import zhPluginAccessLog from './locales/zh-CN/plugin-access-log.json';
import enPluginAccessLog from './locales/en-US/plugin-access-log.json';
import zhPluginList from './locales/zh-CN/plugin-list.json';
import enPluginList from './locales/en-US/plugin-list.json';
import zhMeshInspector from './locales/zh-CN/mesh-inspector.json';
import enMeshInspector from './locales/en-US/mesh-inspector.json';
import zhManagedConfigReports from './locales/zh-CN/managed-config-reports.json';
import enManagedConfigReports from './locales/en-US/managed-config-reports.json';
import zhScreenshotSheet from './locales/zh-CN/screenshot-sheet.json';
import enScreenshotSheet from './locales/en-US/screenshot-sheet.json';
import zhMemoryControl from './locales/zh-CN/memory-control.json';
import enMemoryControl from './locales/en-US/memory-control.json';
import zhPluginPermissions from './locales/zh-CN/plugin-permissions.json';
import enPluginPermissions from './locales/en-US/plugin-permissions.json';
import zhPackUpdate from './locales/zh-CN/pack-update.json';
import enPackUpdate from './locales/en-US/pack-update.json';
import zhMeshSocial from './locales/zh-CN/mesh-social.json';
import enMeshSocial from './locales/en-US/mesh-social.json';
import zhLocalPacks from './locales/zh-CN/localPacks.json';
import enLocalPacks from './locales/en-US/localPacks.json';
import zhCreateGame from './locales/zh-CN/createGame.json';
import enCreateGame from './locales/en-US/createGame.json';
import zhModUpdates from './locales/zh-CN/modUpdates.json';
import enModUpdates from './locales/en-US/modUpdates.json';
import zhCurseforge from './locales/zh-CN/curseforge.json';
import enCurseforge from './locales/en-US/curseforge.json';
import zhNavigation from './locales/zh-CN/navigation.json';
import enNavigation from './locales/en-US/navigation.json';
// SPDX-License-Identifier: MIT
import zhCommon from './locales/zh-CN/common.json';
import zhSettings from './locales/zh-CN/settings.json';
import enSettings from './locales/en-US/settings.json';
import enCommon from './locales/en-US/common.json';
import zhHome from './locales/zh-CN/home.json';
import enHome from './locales/en-US/home.json';
import zhVersions from './locales/zh-CN/versions.json';
import enVersions from './locales/en-US/versions.json';
import zhMarket from './locales/zh-CN/market.json';
import enMarket from './locales/en-US/market.json';
import zhRuntime from './locales/zh-CN/runtime.json';
import enRuntime from './locales/en-US/runtime.json';

import zhSkin from './locales/zh-CN/skin.json';
import enSkin from './locales/en-US/skin.json';
import zhPlugins from './locales/zh-CN/plugins.json';
import enPlugins from './locales/en-US/plugins.json';
import zhBlueprints from './locales/zh-CN/blueprints.json';
import enBlueprints from './locales/en-US/blueprints.json';
import zhProfiles from './locales/zh-CN/profiles.json';
import enProfiles from './locales/en-US/profiles.json';
import zhReports from './locales/zh-CN/reports.json';
import enReports from './locales/en-US/reports.json';

import zhWorldSnapshots from './locales/zh-CN/worldSnapshots.json';
import enWorldSnapshots from './locales/en-US/worldSnapshots.json';
import zhWorlds from './locales/zh-CN/worlds.json';
import enWorlds from './locales/en-US/worlds.json';

import zhTasks from './locales/zh-CN/tasks.json';
import enTasks from './locales/en-US/tasks.json';

import zhInspector from './locales/zh-CN/inspector.json';
import enInspector from './locales/en-US/inspector.json';
import zhMesh from './locales/zh-CN/mesh.json';
import enMesh from './locales/en-US/mesh.json';

import zhSettingsLayout from './locales/zh-CN/settingsLayout.json';
import enSettingsLayout from './locales/en-US/settingsLayout.json';
import zhExtensionSettings from './locales/zh-CN/extensionSettings.json';
import enExtensionSettings from './locales/en-US/extensionSettings.json';
import zhGameAppearance from './locales/zh-CN/gameAppearance.json';
import enGameAppearance from './locales/en-US/gameAppearance.json';
import zhShell from './locales/zh-CN/shell.json';
import enShell from './locales/en-US/shell.json';
import zhGameActions from './locales/zh-CN/gameActions.json';
import enGameActions from './locales/en-US/gameActions.json';
import zhTools from './locales/zh-CN/tools.json';
import enTools from './locales/en-US/tools.json';

import zhExtensionCommands from './locales/zh-CN/extensionCommands.json';
import enExtensionCommands from './locales/en-US/extensionCommands.json';
import zhBlueprintOnline from './locales/zh-CN/blueprintOnline.json';
import enBlueprintOnline from './locales/en-US/blueprintOnline.json';

import zhGameMods from './locales/zh-CN/gameMods.json';
import enGameMods from './locales/en-US/gameMods.json';
import zhGameDetails from './locales/zh-CN/gameDetails.json';
import enGameDetails from './locales/en-US/gameDetails.json';
import zhLauncherImport from './locales/zh-CN/launcherImport.json';
import enLauncherImport from './locales/en-US/launcherImport.json';
import zhImportPack from './locales/zh-CN/importPack.json';
import enImportPack from './locales/en-US/importPack.json';

import zhPluginDetails from './locales/zh-CN/plugin-details.json';
import enPluginDetails from './locales/en-US/plugin-details.json';

import zhScreenshots from './locales/zh-CN/screenshots.json';
import enScreenshots from './locales/en-US/screenshots.json';

import zhGameConfigs from './locales/zh-CN/game-configs.json';
import enGameConfigs from './locales/en-US/game-configs.json';
import zhCreatedGamePreferences from './locales/zh-CN/createdGamePreferences.json';
import enCreatedGamePreferences from './locales/en-US/createdGamePreferences.json';

export const messages = {
  'zh-CN': { gameServers: zhGameServers, createdGamePreferences: zhCreatedGamePreferences, pluginList: zhPluginList, meshInspector: zhMeshInspector, pluginAccessLog: zhPluginAccessLog, managedConfigReports: zhManagedConfigReports, screenshotSheet: zhScreenshotSheet, memoryControl: zhMemoryControl, pluginPermissions: zhPluginPermissions, packUpdate: zhPackUpdate, meshSocial: zhMeshSocial, gameConfigs: zhGameConfigs, screenshots: zhScreenshots, pluginDetails: zhPluginDetails, localPacks: zhLocalPacks, createGame: zhCreateGame, importPack: zhImportPack, launcherImport: zhLauncherImport, gameMods: zhGameMods, gameDetails: zhGameDetails, modUpdates: zhModUpdates, blueprintOnline: zhBlueprintOnline, extensionCommands: zhExtensionCommands, curseforge: zhCurseforge, home: zhHome, navigation: zhNavigation, settings: zhSettings, common: zhCommon, versions: zhVersions, market: zhMarket, runtime: zhRuntime, skin: zhSkin, plugins: zhPlugins, blueprints: zhBlueprints, profiles: zhProfiles, reports: zhReports, worldSnapshots: zhWorldSnapshots, worlds: zhWorlds, tasks: zhTasks, inspector: zhInspector, mesh: zhMesh, settingsLayout: zhSettingsLayout, extensionSettings: zhExtensionSettings, gameAppearance: zhGameAppearance, shell: zhShell, gameActions: zhGameActions, tools: zhTools },
  'en-US': { gameServers: enGameServers, createdGamePreferences: enCreatedGamePreferences, pluginList: enPluginList, meshInspector: enMeshInspector, pluginAccessLog: enPluginAccessLog, managedConfigReports: enManagedConfigReports, screenshotSheet: enScreenshotSheet, memoryControl: enMemoryControl, pluginPermissions: enPluginPermissions, packUpdate: enPackUpdate, meshSocial: enMeshSocial, gameConfigs: enGameConfigs, screenshots: enScreenshots, pluginDetails: enPluginDetails, localPacks: enLocalPacks, createGame: enCreateGame, importPack: enImportPack, launcherImport: enLauncherImport, gameMods: enGameMods, gameDetails: enGameDetails, modUpdates: enModUpdates, blueprintOnline: enBlueprintOnline, extensionCommands: enExtensionCommands, curseforge: enCurseforge, home: enHome, navigation: enNavigation, settings: enSettings, common: enCommon, versions: enVersions, market: enMarket, runtime: enRuntime, skin: enSkin, plugins: enPlugins, blueprints: enBlueprints, profiles: enProfiles, reports: enReports, worldSnapshots: enWorldSnapshots, worlds: enWorlds, tasks: enTasks, inspector: enInspector, mesh: enMesh, settingsLayout: enSettingsLayout, extensionSettings: enExtensionSettings, gameAppearance: enGameAppearance, shell: enShell, gameActions: enGameActions, tools: enTools },
};
export type MessageSchema = typeof messages['zh-CN'];
type Keys<T> = { [K in keyof T & string]: T[K] extends string ? K : `${K}.${Keys<T[K]>}` }[keyof T & string];
export type TranslationKey = Keys<MessageSchema>;




