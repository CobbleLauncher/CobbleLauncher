// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "world-chronicle",
    "tone": "data",
    "results": ".hero-metrics",
    "icons": {
        "h1": "clock",
        "header .primary": "file",
        ".ranking h2": "layers"
    }
} satisfies PresentationConfig;
