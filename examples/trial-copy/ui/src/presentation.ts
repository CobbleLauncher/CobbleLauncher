// SPDX-License-Identifier: MIT
import type { PresentationConfig } from '../../../../packages/plugin-workbench/src/presentation';

export const presentation = {
    "id": "trial-copy",
    "tone": "data",
    "scope": ".fields",
    "groups": ".form",
    "results": ".plan",
    "icons": {
        "h1": "copy",
        "[data-action=preview-copy]": "search",
        "[data-action=pick-destination]": "folder",
        "[data-action=create-copy]": "copy",
        ".plan h2": "check"
    }
} satisfies PresentationConfig;
