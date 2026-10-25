// SPDX-License-Identifier: MIT
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';

it('keeps both isolated WebView allowlists aligned with the typed SDK and native broker', () => {
  const views = readFileSync('src-tauri/src/plugin_views.rs', 'utf8');
  const sdk = readFileSync('../../packages/plugin-web-sdk/src/index.ts', 'utf8');
  const broker = readFileSync('src-tauri/src/plugin_frontend_bridge.rs', 'utf8');
  const sdkMethods = [...sdk.match(/export type FrontendMethod = ([^;]+);/)![1]!.matchAll(/'([^']+)'/g)].map(match => match[1]).sort();
  for (const name of ['SHELL_BOOTSTRAP', 'FRAME_BOOTSTRAP']) {
    const script = views.split(`const ${name}: &str = r#"`)[1]!.split('"#;')[0]!;
    const list = script.match(/\[[^\]]*'context'[^\]]*\]/)![0];
    expect([...list.matchAll(/'([^']+)'/g)].map(match => match[1]).sort(), name).toEqual(sdkMethods);
  }
  const variants = broker.split('pub enum FrontendRequest {')[1]!.split('\n}')[0]!;
  expect([...variants.matchAll(/#\[serde\(rename = "([^"]+)"\)\]/g)].map(match => match[1]).sort()).toEqual(sdkMethods);
});
