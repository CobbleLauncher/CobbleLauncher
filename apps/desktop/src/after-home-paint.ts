// SPDX-License-Identifier: MIT
/** Defer nonessential native work until the home screen has had a paint. */
export function afterHomePaint(run: () => void): () => void {
  let frame = requestAnimationFrame(() => {
    frame = requestAnimationFrame(run);
  });
  return () => cancelAnimationFrame(frame);
}
