// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "lang-doctor",
    "tone": "diagnostic",
    "scope": ".selection-row",
    "results": ".metrics",
    "icons": {
        "[data-action=\"export\"]": "export",
        "[data-action=\"cancel\"]": "close",
        "h1": "text",
        "[data-action=\"pick\"]": "folder",
        "[data-action=\"check\"]": "check"
    }
} satisfies PresentationConfig;
