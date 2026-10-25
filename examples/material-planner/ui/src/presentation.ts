// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "material-planner",
    "tone": "creative",
    "scope": ".scope-row",
    "groups": ".title-row,.list-tools",
    "results": ".summary",
    "icons": {
        "[data-action=\"reload\"]": "refresh",
        "[data-action=\"save\"]": "save",
        "[data-action=\"copy\"]": "copy",
        "[data-action=\"export\"]": "export",
        "[data-action=\"delete\"]": "trash",
        "h1": "box",
        "[data-action=\"apply-stack\"]": "layers"
    }
} satisfies PresentationConfig;
