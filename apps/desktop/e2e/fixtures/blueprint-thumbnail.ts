// SPDX-License-Identifier: MIT
// Original synthetic planter illustration, generated locally for UI image/layout evidence only.
import { deflateSync } from 'node:zlib';

const width = 320, height = 180;
const pixels = Buffer.alloc(width * height * 4);
function polygon(points: [number, number][], color: [number, number, number]) {
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    let inside = false;
    for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
      const a = points[i]!, b = points[j]!;
      if ((a[1] > y) !== (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]) inside = !inside;
    }
    if (inside) { const offset = (y * width + x) * 4; pixels[offset] = color[0]; pixels[offset + 1] = color[1]; pixels[offset + 2] = color[2]; pixels[offset + 3] = 255; }
  }
}
polygon([[0, 0], [320, 0], [320, 180], [0, 180]], [234, 240, 230]);
polygon([[77, 141], [167, 112], [248, 140], [158, 169]], [205, 218, 199]);
polygon([[114, 108], [161, 123], [161, 153], [121, 138]], [157, 87, 60]);
polygon([[161, 123], [208, 108], [201, 138], [161, 153]], [114, 62, 48]);
polygon([[110, 103], [160, 87], [211, 103], [161, 120]], [198, 124, 83]);
polygon([[125, 102], [160, 91], [196, 102], [161, 113]], [74, 63, 44]);
polygon([[155, 57], [165, 57], [165, 105], [155, 108]], [85, 117, 57]);
polygon([[119, 61], [145, 52], [164, 63], [138, 72]], [116, 153, 76]);
polygon([[138, 72], [164, 63], [164, 80], [138, 88]], [72, 111, 55]);
polygon([[160, 46], [186, 37], [210, 47], [184, 56]], [137, 167, 84]);
polygon([[160, 46], [184, 56], [184, 76], [160, 66]], [79, 126, 59]);
polygon([[184, 56], [210, 47], [210, 66], [184, 76]], [63, 103, 51]);

function chunk(type: string, data: Buffer) {
  const name = Buffer.from(type), length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  let crc = 0xffffffff;
  for (const byte of Buffer.concat([name, data])) { crc ^= byte; for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0); }
  const checksum = Buffer.alloc(4); checksum.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([length, name, data, checksum]);
}
const header = Buffer.alloc(13); header.writeUInt32BE(width); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 6;
const rows = Buffer.alloc(height * (width * 4 + 1));
for (let y = 0; y < height; y++) pixels.copy(rows, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
const png = Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))]);
export const blueprintThumbnail = `data:image/png;base64,${png.toString('base64')}`;
