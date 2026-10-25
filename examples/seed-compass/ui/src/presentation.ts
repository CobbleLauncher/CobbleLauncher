// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "seed-compass",
    "tone": "creative",
    "scope": ".source-row",
    "groups": ".seed-form",
    "results": ".map-panel",
    "icons": {
        "[data-action=\"cancel\"]": "close",
        "h1": "map",
        "[data-action=\"generate\"]": "map",
        "[data-action=\"search\"]": "search",
        ".search-panel h2": "pin"
    }
} satisfies PresentationConfig;
