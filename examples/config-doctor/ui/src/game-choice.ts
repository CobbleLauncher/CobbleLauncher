// SPDX-License-Identifier: MIT
import type { Context } from '../../../../packages/plugin-web-sdk/src/index';

/** Select an available recent game; bound views retain their context. */
export function populateGames(select: HTMLSelectElement, context: Pick<Context, 'games' | 'instanceId' | 'recentGames'>, placeholder: string) {
  const document = select.ownerDocument;
  const options: HTMLOptionElement[] = [];
  if (!context.instanceId) {
    const choice = document.createElement('option');
    choice.value = '';
    choice.textContent = placeholder;
    choice.disabled = true;
    options.push(choice);
  }
  for (const game of context.games) {
    if (context.instanceId && game.id !== context.instanceId) continue;
    const option = document.createElement('option');
    option.value = game.id;
    option.textContent = `${game.name} · ${game.version}`;
    options.push(option);
  }
  select.replaceChildren(...options);
  const recent = context.recentGames?.find(game => context.games.some(available => available.id === game.id));
  select.value = context.instanceId ?? recent?.id ?? context.games[0]?.id ?? '';
}
