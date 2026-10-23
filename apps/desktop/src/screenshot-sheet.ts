// SPDX-License-Identifier: MIT
export const MAX_SHEET_IMAGES=12;
export type ScreenshotSheetPlugin={plugin_id:string;name:string};
export type ScreenshotSheetPreview={preview_id:string;data_url:string;width:number;height:number};
export type ScreenshotSheetOrdering='selected'|'oldest'|'newest';
