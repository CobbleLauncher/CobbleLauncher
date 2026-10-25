// SPDX-License-Identifier: MIT
export const FIRST_RUN_CONSENT_VERSION = '2026-10-24';
export const guideSteps = ['welcome', 'profile', 'game', 'mesh'] as const;
export const firstRunSteps = ['consent', ...guideSteps] as const;
export type FirstRunStep = typeof firstRunSteps[number];
export function nextFirstRunStep(step: FirstRunStep): FirstRunStep {
  return firstRunSteps[Math.min(firstRunSteps.indexOf(step) + 1, firstRunSteps.length - 1)]!;
}
