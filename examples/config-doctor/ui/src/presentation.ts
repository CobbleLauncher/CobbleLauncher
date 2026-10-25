// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "config-doctor",
    "tone": "diagnostic",
    "scope": ".toolbar",
    "results": ".results",
    "icons": {
        "#refresh": "refresh",
        "#cancel": "close",
        "#check": "shield",
        "#configs-tab": "settings",
        "#recovery-tab": "clock",
        "#bindings-tab": "layers",
        "#preview-title": "file",
        "#results-title": "shield"
    }
} satisfies PresentationConfig;
