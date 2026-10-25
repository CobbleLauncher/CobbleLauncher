// SPDX-License-Identifier: MIT
import { WAYPOINT_IMPORT_INTERFACE, WAYPOINT_IMPORT_VERSION, type Client, type HandoffReceiver, type WaypointPayload } from '../../../../packages/plugin-web-sdk/src/index';
type SelectedLocation = {
    gameId: string;
    worldId?: string;
    payload: WaypointPayload;
};
export function mountWaypointHandoff(client: Client, container: HTMLElement, tr: (en: string, zh: string) => string, selection: () => SelectedLocation | null, feedback: (message: string, error: boolean) => void) {
    const wrapper = document.createElement('span');
    wrapper.className = 'waypoint-handoff';
    wrapper.hidden = true;
    const receiver = document.createElement('select');
    receiver.setAttribute('aria-label', tr('Save location with', '保存地点的工具'));
    receiver.dataset.field = 'waypoint-receiver';
    const open = document.createElement('button');
    open.type = 'button';
    open.textContent = tr('Save location…', '保存地点…');
    open.dataset.action = 'bookmark';
    wrapper.append(receiver, open);
    container.append(wrapper);
    let providers: HandoffReceiver[] = [], sending = false;
    function refresh() {
        wrapper.hidden = providers.length === 0 || !selection();
        receiver.hidden = providers.length < 2;
        receiver.disabled = sending;
        open.disabled = sending;
    }
    async function load() {
        try {
            providers = await client.handoffs.receivers({ interfaceId: WAYPOINT_IMPORT_INTERFACE, version: WAYPOINT_IMPORT_VERSION });
            receiver.replaceChildren();
            for (const provider of providers) {
                const option = document.createElement('option');
                option.value = provider.pluginId;
                option.textContent = provider.name;
                receiver.append(option);
            }
        }
        catch {
            providers = [];
        }
        refresh();
    }
    open.onclick = () => {
        void send();
    };
    async function send() {
        const location = selection(), provider = providers.find(item => item.pluginId === receiver.value);
        if (sending || !location || !provider)
            return;
        sending = true;
        refresh();
        try {
            await client.handoffs.send({ targetPluginId: provider.pluginId, interfaceId: WAYPOINT_IMPORT_INTERFACE, version: WAYPOINT_IMPORT_VERSION, ...location });
            feedback(tr('Location ready for review. Save it in the coordinate tool.', '地点已准备好，请在坐标工具中确认保存。'), false);
        }
        catch {
            feedback(tr('Could not open the coordinate tool. Try again.', '无法打开坐标工具，请重试。'), true);
        }
        finally {
            sending = false;
            refresh();
        }
    }
    void load();
    return { refresh, reload: load };
}
