// SPDX-License-Identifier: MIT
// Generic presentation primitives; each consumer owns its selectors and layout.
import './presentation.css';
import './lucide-icons.generated.css';

export type PresentationIcon = 'refresh' | 'file' | 'folder' | 'search' | 'check' | 'copy' | 'save' | 'export' | 'plus' | 'close' | 'trash' | 'map' | 'pin' | 'graph' | 'layers' | 'tool' | 'clock' | 'shield' | 'settings' | 'send' | 'text' | 'box';
export type PresentationConfig = {
    id?: string;
    tone: 'data' | 'creative' | 'diagnostic';
    icons?: Record<string, PresentationIcon>;
    scope?: string;
    groups?: string;
    results?: string;
};

export function installPluginPresentation(config: PresentationConfig) {
    const root = document.querySelector<HTMLElement>('#app') ?? document.querySelector<HTMLElement>('main');
    if (!root) return;
    const rules = [
        ...Object.entries(config.icons ?? {}).map(([selector, icon]) => ({ selector, decorate: (element: HTMLElement) => {
            if (element.dataset.presentationIcon !== icon) element.dataset.presentationIcon = icon;
        } })),
        ...([[config.scope, 'plugin-scope'], [config.groups, 'plugin-group'], [config.results, 'plugin-result']] as const)
            .filter(([selector]) => Boolean(selector)).map(([selector, cls]) => ({ selector: selector!, decorate: (element: HTMLElement) => {
                if (!element.classList.contains(cls)) element.classList.add(cls);
            } })),
    ];
    root.dataset.tone = config.tone;
    if (config.id) root.dataset.plugin = config.id;
    const keepRootClass = () => { if (!root.classList.contains('plugin-surface')) root.classList.add('plugin-surface'); };
    const decorate = (subtree: HTMLElement) => {
        for (const rule of rules) {
            if (subtree.matches(rule.selector)) rule.decorate(subtree);
            for (const element of subtree.querySelectorAll<HTMLElement>(rule.selector)) rule.decorate(element);
        }
    };
    keepRootClass(); decorate(root);
    // A batch visits only newly inserted subtrees. Text/label changes need no scan:
    // icons are CSS masks, so they survive textContent updates without DOM writes.
    const observer = new MutationObserver(records => {
        keepRootClass();
        const added = new Set<HTMLElement>();
        for (const record of records) for (const node of record.addedNodes) if (node instanceof HTMLElement) added.add(node);
        for (const node of added) {
            if (!root.contains(node)) continue;
            let parent = node.parentElement;
            while (parent && !added.has(parent)) parent = parent.parentElement;
            if (!parent) decorate(node);
        }
    });
    observer.observe(root, { childList: true, subtree: true });
    const contextMenu = (event: MouseEvent) => {
        if (event.defaultPrevented || !(event.target instanceof Element)) return;
        const field = event.target.closest('input,textarea,[contenteditable]');
        if (field instanceof HTMLTextAreaElement || field instanceof HTMLElement && field.isContentEditable) return;
        if (field instanceof HTMLInputElement && ['text', 'search', 'password', 'email', 'number', 'tel', 'url'].includes(field.type)) return;
        event.preventDefault();
    };
    window.addEventListener('contextmenu', contextMenu);
    window.addEventListener('pagehide', () => { observer.disconnect(); window.removeEventListener('contextmenu', contextMenu); }, { once: true });
}
