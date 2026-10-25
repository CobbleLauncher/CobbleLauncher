// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "anvil-planner",
    "tone": "creative",
    "scope": ".context-row",
    "groups": ".calculate-actions",
    "results": ".output-panel",
    "icons": {
        "[data-action=\"copy\"]": "copy",
        "[data-action=\"cancel\"]": "close",
        "h1": "tool",
        "[data-action=\"calculate\"]": "tool",
        "[data-action=\"preset\"]": "layers",
        "[data-action=\"add-book\"],[data-action=\"add-existing\"]": "plus"
    }
} satisfies PresentationConfig;
