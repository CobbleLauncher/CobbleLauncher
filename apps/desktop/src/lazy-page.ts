// SPDX-License-Identifier: MIT
import { defineAsyncComponent, h, type AsyncComponentLoader, type Component } from 'vue';
import AppIcon from './ui/AppIcon.vue';
import { t } from './i18n';

/** Route modules do not initialize until the user first opens their workspace. */
export function lazyPage<T extends Component>(loader: AsyncComponentLoader<T>) {
  return defineAsyncComponent({
    loader,
    delay: 150,
    loadingComponent: {
      setup: () => () => h('div', {
        class: 'page-loading', role: 'status', 'aria-busy': 'true',
        'aria-label': t('navigation.loading'),
      }, [h(AppIcon, { name: 'loader-circle', class: 'ui-busy-icon' })]),
    },
  });
}
