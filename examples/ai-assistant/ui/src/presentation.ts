// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "ai-assistant",
    "tone": "creative",
    "groups": ".setup-actions,.composer-actions",
    "icons": {
        "#save": "save",
        "#cancel": "close",
        "h1": "text",
        "#settings-toggle": "settings",
        "#send": "send",
        "#load-file": "file"
    }
} satisfies PresentationConfig;
