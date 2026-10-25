// SPDX-License-Identifier: MIT
export function bytes(value: number | null, locale = 'en'): string {
    if (value === null) return locale === 'zh-CN' ? '不可读取' : 'Unavailable';
    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB']; let n = value, unit = 0;
    while (n >= 1024 && unit < units.length - 1) { n /= 1024; unit++; }
    return `${n.toLocaleString(locale, { maximumFractionDigits: unit ? 1 : 0 })} ${units[unit]}`;
}
export function percent(completed: number, total: number): number { return total > 0 ? Math.min(100, Math.floor(completed / total * 100)) : 0; }
export function problem(code: unknown, zh: boolean): string {
    const value = String(code);
    const messages: [RegExp, string, string][] = [
        [/SPACE_LOW/, 'There is not enough space in the selected destination.', '目标磁盘的可用空间不足。'],
        [/TARGET_EXISTS/, 'This destination already contains a folder for this game. Choose another folder.', '目标已存在此游戏目录，请选择其他文件夹。'],
        [/SOURCE_CHANGED/, 'The original files changed. Choose the destination again to review a fresh copy.', '源文件已更改，请重新选择目标并预览。'],
        [/INSTANCE_BUSY|INSTANCE_CHANGED|SOURCE_IN_USE/, 'Close the game and finish other operations before moving it.', '请先关闭游戏并等待其他操作完成。'],
        [/LINK_DENIED|SPECIAL_FILE/, 'This game contains linked or special files. It has not been moved.', '此游戏包含链接或特殊文件，尚未迁移。'],
        [/PLAN_EXPIRED/, 'This preview expired. Choose the destination again.', '预览已过期，请重新选择目标。'],
        [/RECYCLE/, 'The original copy could not be recycled. It is still in its original folder.', '旧副本无法移入回收站，仍保留在原目录。'],
        [/INDEX_FAILED|RECEIPT_FAILED/, 'The copy was verified, but its location could not be saved. The original game is kept.', '副本已核验，但无法保存新位置。原游戏仍保留。'],
        [/CANCELLED/, 'Move cancelled. The original game is kept.', '迁移已取消，原游戏仍保留。'],
        [/PERMISSION|REVOKED/, 'Review this tool’s storage permissions and try again.', '请检查此工具的存储权限后重试。'],
    ];
    return messages.find(([pattern]) => pattern.test(value))?.[zh ? 2 : 1] ?? (zh ? '操作未完成，原游戏仍保留。请重试。' : 'The operation did not finish. The original game is kept. Try again.');
}
