// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "modpack-inspector",
    "tone": "data",
    "scope": ".toolbar",
    "groups": ".filters",
    "results": ".mod-details",
    "icons": {
        "[data-action=refresh]": "refresh",
        ".map-panel h2": "graph",
        "[data-action=expand-neighborhood]": "plus",
        ".analysis-summary>summary": "shield"
    }
} satisfies PresentationConfig;
