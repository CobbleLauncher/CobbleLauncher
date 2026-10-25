// SPDX-License-Identifier: MIT
const cardControls = 'button, a, input, select, textarea, summary, label, [role="button"], [role="menuitem"], [data-card-control]';

export function isGameCardSurface(event: Pick<MouseEvent, 'target' | 'currentTarget'>): boolean {
  return event.target instanceof Element
    && event.currentTarget instanceof Element
    && event.currentTarget.contains(event.target)
    && !event.target.closest(cardControls);
}
