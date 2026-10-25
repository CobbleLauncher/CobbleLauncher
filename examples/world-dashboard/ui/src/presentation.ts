// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "world-dashboard",
    "tone": "data",
    "scope": ".context",
    "results": ".snapshot-info",
    "icons": {
        "h1": "clock",
        "[data-action=refresh]": "refresh",
        ".timeline-header h2": "layers",
        ".timeline-actions .primary": "graph",
        ".recovery-action": "clock",
        ".world-check summary": "shield"
    }
} satisfies PresentationConfig;
