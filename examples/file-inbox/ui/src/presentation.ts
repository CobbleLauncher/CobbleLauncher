// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "file-inbox",
    "tone": "data",
    "groups": ".detail-heading",
    "results": ".inbox-detail",
    "icons": {
        "h1": "folder",
        "header .primary": "plus",
        ".review": "check"
    }
} satisfies PresentationConfig;
