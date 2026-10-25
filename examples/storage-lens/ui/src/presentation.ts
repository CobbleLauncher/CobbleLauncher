// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "storage-lens",
    "tone": "data",
    "groups": ".cache-row",
    "results": ".move-preview",
    "icons": {
        "[data-action=\"refresh\"]": "refresh",
        "h1": "box",
        ".cache-row h2": "layers",
        "[data-action=\"clear-cache\"]": "trash",
        "[data-action=\"pick-destination\"]": "folder",
        "[data-action=\"start-move\"]": "export"
    }
} satisfies PresentationConfig;
