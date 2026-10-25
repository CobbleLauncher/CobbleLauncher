// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "play-manifest",
    "tone": "data",
    "groups": ".confirmations",
    "results": ".export-action",
    "icons": {
        "h1": "box",
        "header button": "search",
        ".export-action button": "export",
        ".review": "file"
    }
} satisfies PresentationConfig;
