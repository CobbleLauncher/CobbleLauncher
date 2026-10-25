// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "crash-doctor",
    "tone": "diagnostic",
    "scope": ".toolbar",
    "groups": ".document",
    "results": "#results",
    "icons": {
        "#refresh": "refresh",
        "#cancel": "close",
        "#manual": "folder",
        "#check": "search",
        "#diagnosis-tab": "shield",
        "#timeline-tab": "clock",
        "#share-tab": "export",
        "#recovery-tab": "clock",
        "#experiments-tab": "tool",
        "#result-title": "shield"
    }
} satisfies PresentationConfig;
