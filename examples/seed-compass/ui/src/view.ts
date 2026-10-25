// SPDX-License-Identifier: MIT
export const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text = "", className = "") => {
    const node = document.createElement(tag);
    node.textContent = text;
    node.className = className;
    return node;
};
export const button = (text: string, fn: () => void, cls = "") => {
    const node = el("button", text, cls);
    node.type = "button";
    node.onclick = fn;
    return node;
};
export const option = (select: HTMLSelectElement, value: string, label: string) => {
    const node = el("option", label);
    node.value = value;
    select.append(node);
};
const field = (text: string, node: HTMLElement) => {
    const label = el("label", text);
    label.append(node);
    return label;
};
export function createView(host: HTMLElement, tr: (en: string, cn: string) => string, actions: {
    cancel: () => void;
    copy: (value: string) => void;
    generate: () => void;
    spawn: () => void;
    copyCoordinates: () => void;
    jump: () => void;
}) {
    const header = el("header", "", "toolbar"), title = el("h1"), cancel = button("", actions.cancel), source = el("div", "", "source-row"), game = el("select"), world = el("select"), sourceGame = field("", game), sourceWorld = field("", world);
    header.append(title, cancel);
    const gameText=el("span","","context-value");gameText.hidden=true;sourceGame.append(gameText);
    source.append(sourceGame, sourceWorld);
    const notice = el("p", "", "notice"), error = el("p", "", "error"), status = el("p", "", "status");
    notice.hidden = true;
    error.hidden = true;
    error.role = "alert";
    status.role = "status";
    status.setAttribute("aria-live", "polite");
    const form = el("form", "", "seed-form"), seed = el("input"), version = el("select"), dimension = el("select"), preset = el("select"), y = el("input"), seedCopy = button("", () => actions.copy(seed.value)), generate = el("button", "", "primary");
    seed.type = "text";
    seed.inputMode = "numeric";
    seed.maxLength = 21;
    seed.autocomplete = "off";
    seed.spellcheck = false;
    seed.dataset.field = "seed";
    y.type = "number";
    y.value = "64";
    y.dataset.field = "y";
    generate.type = "button";
    const seedField = field("", seed);
    seedField.className = "seed-field";
    seedField.append(seedCopy);
    const versionField = field("", version), dimensionField = field("", dimension), presetField = field("", preset), yField = field("", y);
    const mappingNote=el("small","","version-mapping");mappingNote.hidden=true;versionField.append(mappingNote);versionField.classList.add("version-field");
    form.append(seedField, versionField, dimensionField, presetField, yField, generate);
    const layout = el("div", "", "compass-layout"), mapPanel = el("section", "", "inventory map-panel"), mapToolbar = el("div", "", "map-toolbar"), x = el("input"), z = el("input"), step = el("select"), go = button("", actions.generate), spawn = button("", actions.spawn), xField = field("X", x), zField = field("Z", z), stepField = field("", step);
    x.type = z.type = "number";
    x.value = z.value = "0";
    x.dataset.field = "x";
    z.dataset.field = "z";
    mapToolbar.append(xField, zField, stepField, go, spawn);
    const mapArea = el("div", "", "map-area"), empty = el("p", "", "map-empty"), canvas = el("canvas"), selection = el("div", "", "selection"), coordinates = el("output"), copyCoordinates = button("", actions.copyCoordinates), jump = button("", actions.jump), legend = el("div", "", "legend");
    canvas.width = canvas.height = 128;
    canvas.hidden = true;
    canvas.setAttribute("role", "img");
    selection.hidden = true;
    selection.append(coordinates, copyCoordinates, jump);
    const north = el("span", "N ↑", "map-north");
    north.setAttribute("aria-hidden", "true");
    const marker = el("span", "", "point-marker");
    marker.hidden = true;
    marker.setAttribute("aria-hidden", "true");
    mapArea.append(empty, canvas, north, marker);
    mapPanel.append(mapToolbar, mapArea, selection, legend);
    const searchPanel = el("section", "", "inventory search-panel"), searchHead = el("h2"), searchForm = el("form", "", "search-form"), target = el("select"), radius = el("select"), search = el("button"), targetField = field("", target), radiusField = field("", radius), accuracy = el("p", "", "muted"), results = el("div", "", "results");
    search.type = "button";
    searchForm.append(targetField, radiusField, search);
    searchPanel.append(searchHead, searchForm, accuracy, results);
    layout.append(mapPanel, searchPanel);
    host.replaceChildren(header, source, notice, error, form, status, layout);
    let labeled = false;
    function labels() {
        if (labeled)
            return;
        labeled = true;
        title.textContent = tr("Seed Compass", "种子罗盘");
        cancel.textContent = tr("Cancel", "取消");
        sourceGame.prepend(document.createTextNode(tr("Game", "游戏")));
        sourceWorld.prepend(document.createTextNode(tr("World", "世界")));
        seedField.prepend(document.createTextNode(tr("Seed", "种子")));
        versionField.prepend(document.createTextNode(tr("Minecraft version", "Minecraft 版本")));
        dimensionField.prepend(document.createTextNode(tr("Dimension", "维度")));
        presetField.prepend(document.createTextNode(tr("Generation", "生成方式")));
        yField.prepend(document.createTextNode("Y"));
        stepField.prepend(document.createTextNode(tr("Blocks / pixel", "方块 / 像素")));
        targetField.prepend(document.createTextNode(tr("Biome or structure", "生物群系或结构")));
        radiusField.prepend(document.createTextNode(tr("Search radius", "搜索半径")));
        seedCopy.textContent = tr("Copy", "复制");
        generate.textContent = tr("Draw map", "绘制地图");
        go.textContent = tr("Go", "前往");
        spawn.textContent = tr("Saved spawn", "存档出生点");
        copyCoordinates.textContent = tr("Copy coordinates", "复制坐标");
        jump.textContent = tr("Center here", "在此居中");
        searchHead.textContent = tr("Find nearby", "查找附近");
        search.textContent = tr("Search", "搜索");
        empty.textContent = tr("Enter a seed and draw a map.", "输入种子并绘制地图。");
        canvas.setAttribute("aria-label", tr("Generated biome map, north is up", "生成的生物群系地图，上方为北"));
    }
    for (const [name, node] of Object.entries({ game, world, version, dimension, preset, step, target, radius }))
        node.dataset.field = name;
    generate.dataset.action = "generate";
    search.dataset.action = "search";
    cancel.dataset.action = "cancel";
    return { mappingNote, marker, gameText, game, world, notice, error, status, form, seed, version, dimension, preset, y, generate, seedCopy, cancel, x, z, step, go, spawn, empty, canvas, selection, coordinates, jump, legend, searchForm, target, radius, search, accuracy, results, labels };
}
