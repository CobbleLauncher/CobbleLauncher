// SPDX-License-Identifier: MIT
import { presentation } from './presentation';
import { installPluginPresentation } from '../../../../packages/plugin-workbench/src/presentation';
import { displayName } from "./names";
import {mountWaypointHandoff} from "./handoff";
import { createView, el, button, option } from "./view";
import { hostContext, connect, type Client, type Context } from "../../../../packages/plugin-web-sdk/src/index";
import { decimalSeed, blockCoordinate, rasterPixels, mapPoint, worldRows, versionCatalog, versionTargets, versionHeight, type Raster } from "./model";
import "../../../../packages/plugin-workbench/src/style.css";
import "./style.css";
import '../../../../packages/plugin-workbench/src/host-appearance.css';
type Capabilities = Awaited<ReturnType<Client["seedMap"]["capabilities"]>>;
type Metadata = Awaited<ReturnType<Client["worlds"]["metadata"]>>;
type SearchResult = Awaited<ReturnType<Client["seedMap"]["search"]>>;
export function mountSeedCompass(client: Client, host = document.getElementById("app")!) {
    host.className = "workbench seed-compass";
    let context: Context | null = null, capabilities: Capabilities | null = null, metadata: Metadata | null = null, zh = false, epoch = 0, sequence = 0, activeId: string | null = null, busy = false, disposed = false;
    let map: Raster | null = null, matches: SearchResult["matches"] = [], selected: {
        x: number;
        z: number;
    } | null = null;
    const tr = (en: string, cn: string) => zh ? cn : en;
    const { mappingNote, marker, gameText, game, world, notice, error, status, form, seed, version, dimension, preset, y, generate, seedCopy, cancel, x, z, step, go, spawn, empty, canvas, selection, coordinates, jump, legend, searchForm, target, radius, search, accuracy, results, labels } = createView(host, tr, { cancel: () => void cancelWork(), copy: value => void copy(value), generate: () => void generateMap(), spawn: () => {
            if (metadata?.spawn) {
                x.value = String(metadata.spawn.x);
                z.value = String(metadata.spawn.z);
                void generateMap();
            }
        }, copyCoordinates: () => {
            if (selected)
                void copy(`${selected.x} ${selected.z}`);
        }, jump: () => {
            if (selected) {
                x.value = String(selected.x);
                z.value = String(selected.z);
                void generateMap();
            }
        } });
    let cancellation: Promise<void> = Promise.resolve();
    let waypoint:ReturnType<typeof mountWaypointHandoff>|null=null;
    function failure(cause: unknown) {
        const code = String(cause);
        error.hidden = false;
        error.textContent = /PERMISSION|permission/i.test(code) ? tr("Allow saved-world seed access in the tool permissions, or enter a seed manually.", "请在工具权限中允许读取存档种子，或手动输入种子。") : /UNSUPPORTED_VERSION/.test(code) ? tr("This Minecraft version is not supported. Select a supported version manually.", "尚不支持此 Minecraft 版本，请手动选择受支持版本。") : /BUSY|DUPLICATE/.test(code) ? tr("A map calculation is still finishing. Try again shortly.", "地图计算尚未结束，请稍后重试。") : /CANCELLED/.test(code) ? tr("Cancelled.", "已取消。") : tr("Could not complete this request. Check the inputs and try again.", "无法完成请求，请检查输入后重试。");
    }
    function savedReason() {
        if (!metadata)
            return world.value ? tr("Choose manual input or refresh this saved world to read its seed.", "请选择手动输入，或重新选择存档以读取种子。") : "";
        if (metadata.unavailable || !metadata.seed)
            return tr("The saved seed is unavailable. Enter a seed and version manually.", "无法读取存档种子，请手动输入种子和版本。");
        if (!versionCatalog(capabilities,metadata.version ?? ""))
            return tr("The saved Minecraft version is unsupported. Use manual input to choose a supported version.", "尚不支持此存档的 Minecraft 版本，请切换手动输入并选择受支持版本。");
        if (metadata.generator.custom === true || !capabilities?.presets.some(value => value === metadata!.generator.preset))
            return tr("This world uses custom or unknown generation. Predictions are unavailable; manual input uses only the selected vanilla generation.", "此世界的生成方式为自定义或未知，无法预测；手动输入仅使用所选原版生成方式。");
        return "";
    }
    function ui() {
        host.setAttribute("aria-busy", String(busy));
        cancel.hidden = !busy;
        game.disabled = !!context?.instanceId || busy;
        world.disabled = busy;
        const bounds=versionHeight(versionCatalog(capabilities,version.value),dimension.value,y.value);
        const invalid = !capabilities || decimalSeed(seed.value) === null || !versionCatalog(capabilities,version.value) || !bounds || bounds.value!==Number(y.value) || !capabilities.presets.some(value => value === preset.value) || !capabilities.dimensions.some(value => value === dimension.value) || !!savedReason();
        generate.disabled = busy || invalid;
        search.disabled = busy || invalid || !target.value;
        go.disabled = busy || invalid;
        seedCopy.disabled = decimalSeed(seed.value) === null;
        spawn.hidden = !metadata?.spawn;
        spawn.disabled = busy || invalid;
        jump.disabled = busy;
        notice.textContent = savedReason() || (metadata ? tr("Saved world metadata · generated predictions may differ from explored terrain.", "存档元数据 · 生成预测可能与已探索地形不同。") : "");
        notice.hidden = !notice.textContent;
        notice.className = savedReason() ? "notice" : "muted saved-source";
        waypoint?.refresh();
    }
    async function cancelWork() {
        epoch++;
        const id = activeId;
        activeId = null;
        busy = false;
        if (id)
            cancellation = cancellation.then(async () => {
                try {
                    await client.seedMap.cancel({ requestId: id });
                }
                catch {
                    // A completed request may no longer have a native job slot.
                }
            });
        await cancellation;
        ui();
    }
    async function invalidate() {
        await cancelWork();
        map = null;
        matches = [];
        selected = null;
        canvas.hidden = true;
        empty.hidden = false;
        empty.textContent = tr("Draw a map with these inputs.", "使用当前输入绘制地图。");
        legend.replaceChildren();
        results.replaceChildren();
        selection.hidden = true;
        marker.hidden = true;
        error.hidden = true;
        status.textContent = "";
        ui();
    }
    function common() {
        const parsed = decimalSeed(seed.value);
        if (!parsed || !capabilities || savedReason() || !versionCatalog(capabilities,version.value))
            throw Error("SEED_MAP_REQUEST_INVALID");
        const height = Number(y.value), dim = capabilities.dimensions.find(value => value === dimension.value), generation = capabilities.presets.find(value => value === preset.value);
        const bounds=versionHeight(versionCatalog(capabilities,version.value),dimension.value,y.value);
        if (!Number.isInteger(height) || !bounds || height<bounds.min || height>bounds.max || !dim || !generation)
            throw Error("SEED_MAP_REQUEST_INVALID");
        return { seed: parsed, version: version.value, dimension: dim, preset: generation, y: height };
    }
    function center() {
        const cx = blockCoordinate(x.value), cz = blockCoordinate(z.value);
        if (cx === null || cz === null)
            throw Error("SEED_MAP_REQUEST_INVALID");
        return { x: cx, z: cz };
    }
    async function job<T>(kind: string, run: (id: string) => Promise<T>): Promise<T | null> {
        await cancelWork();
        const current = ++epoch, id = `seed_${Date.now().toString(36)}_${++sequence}`;
        activeId = id;
        busy = true;
        error.hidden = true;
        status.textContent = kind;
        ui();
        try {
            const value = await run(id);
            return !disposed && current === epoch ? value : null;
        }
        catch (cause) {
            if (!disposed && current === epoch)
                failure(cause);
            return null;
        }
        finally {
            if (current === epoch) {
                activeId = null;
                busy = false;
                status.textContent = "";
                ui();
            }
        }
    }
    async function generateMap() {
        let input: ReturnType<typeof common>, position: ReturnType<typeof center>;
        try {
            input = common();
            position = center();
        }
        catch (cause) {
            failure(cause);
            return;
        }
        await invalidate();
        const scale = Number(step.value), width = 128;
        const result = await job(tr("Drawing map…", "正在绘制地图…"), requestId => client.seedMap.generate({ requestId, map: { ...input, x: position.x - width / 2 * scale, z: position.z - width / 2 * scale, width, height: width, step: scale } }));
        if (!result)
            return;
        try {
            const pixels = rasterPixels(result);
            map = result;
            matches = [];
            selected = null;
            results.replaceChildren();
            selection.hidden = true;
            marker.hidden = true;
            const image = new ImageData(pixels, result.width, result.height);
            canvas.width = result.width;
            canvas.height = result.height;
            canvas.getContext("2d")!.putImageData(image, 0, 0);
            canvas.hidden = false;
            empty.hidden = true;
            legend.replaceChildren();
            for (const item of result.palette) {
                const tag = el("span", displayName(item.name,zh?"zh-CN":"en","biome"));
                const swatch = el("i");
                swatch.style.background = item.color;
                tag.prepend(swatch);
                legend.append(tag);
            }
            selectPoint(position);
            status.textContent = `X ${result.x}…${result.x + (result.width - 1) * result.step} · Z ${result.z}…${result.z + (result.height - 1) * result.step}`;
        }
        catch (cause) {
            map = null;
            canvas.hidden = true;
            failure(cause);
        }
    }
    function selectPoint(point: {
        x: number;
        z: number;
    }) {
        selected = point;
        if (map) {
            const left = (point.x - map.x) / (map.width * map.step) * 100, top = (point.z - map.z) / (map.height * map.step) * 100;
            marker.hidden = left < 0 || left > 100 || top < 0 || top > 100;
            marker.style.left = `${left}%`;
            marker.style.top = `${top}%`;
        }
        coordinates.textContent = `X ${point.x} · Z ${point.z}`;
        selection.hidden = false;
        waypoint?.refresh();
    }
    canvas.onclick = event => {
        if (!map)
            return;
        const bounds = canvas.getBoundingClientRect();
        selectPoint(mapPoint(map, (event.clientX - bounds.left) / bounds.width, (event.clientY - bounds.top) / bounds.height));
    };
    async function searchNearby() {
        let input: ReturnType<typeof common>, position: ReturnType<typeof center>;
        try {
            input = common();
            position = center();
        }
        catch (cause) {
            failure(cause);
            return;
        }
        const [kind, id] = target.value.split(":");
        if (!id || !["biome", "structure"].includes(kind!))
            return;
        const result = await job(tr("Searching…", "正在搜索…"), requestId => client.seedMap.search({ requestId, search: { ...input, ...position, radius: Number(radius.value), target: { kind: kind as "biome" | "structure", id }, limit: 16 } }));
        if (!result)
            return;
        matches = result.matches;
        results.replaceChildren();
        accuracy.textContent = result.sampledStep ? tr(`Biome search samples every ${result.sampledStep} blocks.`, `生物群系搜索每 ${result.sampledStep} 方块采样。`) : tr("Predicted locations; terrain may affect generation.", "预测位置，地形可能影响生成。");
        if (!matches.length)
            results.append(el("p", tr("No matches within this radius.", "此范围内没有匹配结果。"), "empty"));
        for (const match of matches) {
            const row = el("article", "", "result-row"), pick = button(`X ${match.x} · Z ${match.z}`, () => {
                selectPoint(match);
                x.value = String(match.x);
                z.value = String(match.z);
                void generateMap();
            }), distance = el("span", `${Math.round(match.distance)} ${tr("blocks", "方块")}`, "muted"), copyButton = button(tr("Copy", "复制"), () => void copy(`${match.x} ${match.z}`));
            row.append(pick, distance, copyButton);
            results.append(row);
        }
    }
    async function copy(text: string) {
        try {
            await client.clipboard.writeText(text);
            status.textContent = tr("Copied.", "已复制。");
        }
        catch {
            const input = el("input");
            input.value = text;
            input.readOnly = true;
            input.setAttribute("aria-label", tr("Select and copy", "选择并复制"));
            status.replaceChildren(document.createTextNode(tr("Copy this value: ", "请复制此值：")), input);
            input.focus();
            input.select();
        }
    }
    function targets() {
        const previous=target.value,catalog=versionCatalog(capabilities,version.value);
        target.replaceChildren();
        if (!capabilities) return;
        const available=versionTargets(catalog,dimension.value);
        const biomeGroup=el("optgroup"),structureGroup=el("optgroup");
        biomeGroup.label=tr("Biomes","生物群系");structureGroup.label=tr("Structures","结构");
        for(const item of available.biomes){const opt=el("option",displayName(item.name,zh?"zh-CN":"en","biome"));opt.value=`biome:${item.id}`;biomeGroup.append(opt);}
        for(const item of available.structures){const opt=el("option",displayName(item.id,zh?"zh-CN":"en","structure"));opt.value=`structure:${item.id}`;structureGroup.append(opt);}
        target.append(biomeGroup,structureGroup);
        if(Array.from(target.options).some(item=>item.value===previous))target.value=previous;
        if(dimension.value!=="overworld"&&preset.value==="large_biomes")preset.value="default";
        for(const opt of preset.options)opt.disabled=opt.value==="large_biomes"&&dimension.value!=="overworld";
        const height=versionHeight(catalog,dimension.value,y.value);
        y.disabled=!height;
        if(height){y.min=String(height.min);y.max=String(height.max);y.value=String(height.value);}else{y.removeAttribute("min");y.removeAttribute("max");}
        mappingNote.hidden=catalog?.mapping!=="family";
        mappingNote.textContent=catalog?.mapping==="family"?tr(`${catalog.engineVersion} generation family`,`${catalog.engineVersion} 系列生成规则`):"";
    }
    async function chooseWorld() {
        await invalidate();
        metadata = null;
        if (!world.value) {
            ui();
            return;
        }
        const current = ++epoch;
        busy = true;
        ui();
        try {
            const value = await client.worlds.metadata(game.value, world.value);
            if (current !== epoch || disposed)
                return;
            metadata = value;
            seed.value = value.seed ?? "";
            if (value.version && !Array.from(version.options).some(option => option.value === value.version))
                option(version, value.version, `${value.version} · ${tr("unsupported", "不支持")}`);
            version.value = value.version ?? "";
            preset.value = value.generator.preset ?? "";
            targets();
            if (value.spawn) {
                x.value = String(value.spawn.x);
                z.value = String(value.spawn.z);
            }
            ui();
        }
        catch (cause) {
            if (current === epoch)
                failure(cause);
        }
        finally {
            if (current === epoch) {
                busy = false;
                ui();
            }
        }
    }
    async function loadWorlds() {
        await invalidate();
        metadata = null;
        world.replaceChildren();
        option(world, "", tr("Manual input", "手动输入"));
        preset.value = "default";
        if (!game.value) {
            ui();
            return;
        }
        const configuredVersion=(context?.inspectionGames??context?.games??[]).find(item=>item.id===game.value)?.version;
        if(configuredVersion){
            if(!Array.from(version.options).some(item=>item.value===configuredVersion)) option(version,configuredVersion,`${configuredVersion} · ${tr("unsupported","不支持")}`);
            version.value=configuredVersion;
            targets();
        }
        const current = ++epoch;
        try {
            const snapshot = await client.inspection.snapshot(game.value, "worlds");
            if (current !== epoch || disposed)
                return;
            for (const row of worldRows(snapshot.data))
                option(world, row.id, row.name);
        }
        catch (cause) {
            if (current === epoch)
                failure(cause);
        }
        ui();
    }
    generate.onclick = () => { if (!generate.disabled && form.reportValidity()) void generateMap(); };
    search.onclick = () => { if (!search.disabled && searchForm.reportValidity()) void searchNearby(); };
    // The isolated frontend cannot submit forms. Handle text-field Enter without enabling allow-forms.
    function enterAction(event: KeyboardEvent, button: HTMLButtonElement) {
        if (event.key !== "Enter" || event.isComposing || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || !(event.target instanceof HTMLInputElement)) return;
        event.preventDefault();
        button.click();
    }
    form.onkeydown = event => enterAction(event, generate);
    searchForm.onkeydown = event => enterAction(event, search);
    form.onsubmit = event => event.preventDefault();
    searchForm.onsubmit = event => event.preventDefault();
    game.onchange = () => void loadWorlds();
    world.onchange = () => void chooseWorld();
    for (const input of [version, preset])
        input.onchange = () => {
            world.value = "";
            metadata = null;
            targets();
            void invalidate();
        };
    seed.oninput = () => {
        world.value = "";
        metadata = null;
        targets();
        void invalidate();
    };
    y.oninput = () => void invalidate();
    dimension.onchange = () => {
        targets();
        void invalidate();
    };
    async function start() {
        try {
            context = await hostContext(client.context());
            zh = context.locale === "zh-CN";
            document.documentElement.lang = zh ? "zh-CN" : "en";
            document.documentElement.dataset.theme = context.theme === "system" ? (matchMedia("(prefers-color-scheme:dark)").matches ? "dark" : "light") : context.theme;
            labels();
            waypoint=mountWaypointHandoff(client,selection,tr,()=>{
                if(!selected||!game.value||!Array.from(game.options).some(item=>item.value===game.value)||!['overworld','nether','end'].includes(dimension.value))return null;
                return {gameId:game.value,...(world.value&&metadata?{worldId:world.value}:{}),payload:{name:tr(`Map location ${selected.x}, ${selected.z}`,`地图地点 ${selected.x}, ${selected.z}`),dimension:dimension.value as 'overworld'|'nether'|'end',x:selected.x,z:selected.z,source:'Seed Compass'}};
            },(message,isError)=>{if(isError){error.textContent=message;error.hidden=false;}else status.textContent=message;});
            capabilities = await client.seedMap.capabilities();
            for (const value of capabilities.supportedVersions)
                option(version, value, value);
            for (const value of capabilities.dimensions)
                option(dimension, value, ({ overworld: tr("Overworld", "主世界"), nether: tr("Nether", "下界"), end: tr("End", "末地") } as Record<string, string>)[value] ?? value);
            for (const value of capabilities.presets)
                option(preset, value, value === "default" ? tr("Default", "默认") : tr("Large biomes", "巨型生物群系"));
            for (const value of capabilities.steps)
                option(step, String(value), String(value));
            step.value = "4";
            for (const value of [512, 1024, 2048, 4096, 8192].filter(value => value <= capabilities!.maxRadius))
                option(radius, String(value), String(value));
            radius.value = "2048";
            targets();
            for (const item of (context.inspectionGames ?? context.games).filter(item => !context!.instanceId || item.id === context!.instanceId))
                option(game, item.id, item.name);
            game.hidden=!!context.instanceId;gameText.hidden=!context.instanceId;gameText.textContent=game.selectedOptions[0]?.textContent??"";
            await loadWorlds();
            ui();
        }
        catch (cause) {
            labels();
            failure(cause);
            ui();
        }
    }
    void start();
    const close = () => {
        disposed = true;
        void cancelWork();
    };
    window.addEventListener("pagehide", close, { once: true });
    return { cancel: cancelWork, dispose: close };
}
try {
    mountSeedCompass(connect());
}
catch {
    const node = document.getElementById("app");
    if (node)
        node.textContent = "Open Seed Compass from Cobble Launcher / 请从 Cobble Launcher 打开种子罗盘";
}

installPluginPresentation(presentation);
