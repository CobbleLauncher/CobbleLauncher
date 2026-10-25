// SPDX-License-Identifier: MIT
import type { Report, Transport } from './index';
export type LanguageFile = { id: string; namespace: string | null; locale: string; name: string };
export type LanguageSelection = { selectionId: string; sourceName: string; kind: 'resource_pack' | 'language_files'; packFormat: number | null; files: LanguageFile[] };
export type LanguageCheck = { selectionId: string; namespace: string | null; baselineLocale: string | null; targetLocale: string };
export function createLangPackage(transport: Transport) {
  const token = (value: string) => { if (!/^[a-f0-9]{64}$/.test(value)) throw new Error('LANG_PACKAGE_SELECTION_INVALID'); return value; };
  return Object.freeze({
    pick: () => transport({ method: 'langPackage.pick', params: {} }) as Promise<LanguageSelection | null>,
    check: (args: LanguageCheck) => transport({ method: 'langPackage.check', params: { ...args, selectionId: token(args.selectionId) } }) as Promise<Report | null>,
    document: (args: { selectionId: string; fileId: string }) => {
      if (!/^file-[0-9]{1,2}$/.test(args.fileId)) throw new Error('LANG_PACKAGE_FILE_NOT_FOUND');
      return transport({ method: 'langPackage.document', params: { selectionId: token(args.selectionId), fileId: args.fileId } }) as Promise<{ name: string; text: string }>;
    },
    cancel: () => transport({ method: 'langPackage.cancel', params: {} }) as Promise<null>,
  });
}
