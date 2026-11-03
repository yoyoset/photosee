// Writes icons/icon.ico (32x32, 32bpp BMP entry). tauri-build refuses to build
// on Windows without an .ico for the resource file, and the spike has no art.
// Run once: `node make-icon.mjs`. The output is committed.
import { mkdirSync, writeFileSync } from 'node:fs';

const S = 32;
const pixels = Buffer.alloc(S * S * 4);
for (let i = 0; i < S * S; i++) pixels.writeUInt32LE(0xff2080e0, i * 4); // BGRA
const mask = Buffer.alloc(S * 4); // AND mask: 1 bit per pixel, rows padded to 4 bytes
const bmp = Buffer.alloc(40);
bmp.writeUInt32LE(40, 0);
bmp.writeInt32LE(S, 4);
bmp.writeInt32LE(S * 2, 8); // ICO quirk: height counts XOR + AND masks
bmp.writeUInt16LE(1, 12);
bmp.writeUInt16LE(32, 14);
const body = Buffer.concat([bmp, pixels, mask]);
const head = Buffer.alloc(22);
head.writeUInt16LE(1, 2); // type: icon
head.writeUInt16LE(1, 4); // count
head.writeUInt8(S, 6);
head.writeUInt8(S, 7);
head.writeUInt16LE(1, 10);
head.writeUInt16LE(32, 12);
head.writeUInt32LE(body.length, 14);
head.writeUInt32LE(22, 18);
mkdirSync('icons', { recursive: true });
writeFileSync('icons/icon.ico', Buffer.concat([head, body]));
