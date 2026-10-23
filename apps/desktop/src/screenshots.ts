// SPDX-License-Identifier: MIT
export type GameScreenshot = { id: string; name: string; modified_at: number | null; size_bytes: number };
export type GameScreenshotPage = { items: GameScreenshot[]; total: number; next_offset: number | null };
export type GameScreenshotImage = { data_url: string; width: number; height: number; original_width: number; original_height: number };
