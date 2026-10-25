// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "coordinate-book",
    "tone": "creative",
    "scope": ".book-scope",
    "groups": ".editor-form",
    "icons": {
        "[data-action=\"reload\"]": "refresh",
        "[data-action=\"save\"]": "save",
        "[data-action=\"copy\"]": "copy",
        "[data-action=\"delete\"]": "trash",
        "h1": "pin",
        "[data-action=\"new\"]": "plus",
        "[data-action=\"show-book\"]": "pin",
        "[data-action=\"show-planner\"]": "map",
        "[data-action=\"show-notes\"]": "text"
    }
} satisfies PresentationConfig;
