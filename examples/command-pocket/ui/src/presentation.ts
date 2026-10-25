// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "command-pocket",
    "tone": "creative",
    "scope": ".toolbar",
    "groups": ".editor",
    "results": ".editor>pre",
    "icons": {
        "[data-action=show-book]": "text",
        "[data-action=show-generator]": "tool",
        "[data-action=save-command]": "save",
        "[data-action=copy-command]": "copy"
    }
} satisfies PresentationConfig;
