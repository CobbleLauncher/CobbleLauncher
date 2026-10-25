// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "geometry-builder",
    "tone": "creative",
    "scope": ".controls",
    "results": ".geometry-result",
    "icons": {
        "h1": "box",
        "[data-action=\"generate\"]": "layers",
        "[data-action=\"save\"]": "save",
        "[data-action=\"open\"]": "folder"
    }
} satisfies PresentationConfig;
