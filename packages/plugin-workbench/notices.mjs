// SPDX-License-Identifier: MIT
import { readFileSync } from 'node:fs';

// A separate indexed asset keeps notices intact after CSS minification.
export function frontendNotices() {
    return {
        name: 'cobble-plugin-frontend-notices',
        generateBundle() {
            this.emitFile({
                type: 'asset', fileName: 'licenses/lucide.json',
                source: readFileSync(new URL('./src/lucide-license.generated.json', import.meta.url), 'utf8'),
            });
        },
    };
}
