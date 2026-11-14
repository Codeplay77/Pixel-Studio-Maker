// Cria slime.png (32x32) sem usar o Pixel Studio Maker: é o "sprite de fora" que a
// Demonstração C importa. Escreve o PNG à mão com zlib.
import { writeFileSync } from "node:fs";
import { crc32, deflateSync } from "node:zlib";

const W = 32, H = 32;
const px = new Array(W * H).fill(null);
const set = (x, y, c) => x >= 0 && y >= 0 && x < W && y < H && (px[y * W + x] = c);
const hex = (h) => [1, 3, 5, 7].map((i) => (i < h.length ? parseInt(h.slice(i, i + 2), 16) : 255));

const OUT = "#14241a", DARK = "#2f7a3c", BODY = "#4cbb52", LIGHT = "#8be37a", SHINE = "#f2fff0", EYE = "#10161a", SHADOW = "#0b120e55";
const cx = 16, cy = 22, rx = 11.5, ry = 10;
const inside = (x, y) => y <= 29 && ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1;

for (let x = 6; x < 27; x++) for (let y = 29; y < 32; y++) if (((x + 0.5 - 16) / 10.5) ** 2 + ((y + 0.5 - 30.5) / 1.6) ** 2 <= 1) set(x, y, SHADOW);
for (let y = 0; y < H; y++)
  for (let x = 0; x < W; x++) {
    if (!inside(x, y)) continue;
    const edge = [[1, 0], [-1, 0], [0, 1], [0, -1]].some(([dx, dy]) => !inside(x + dx, y + dy));
    if (edge) set(x, y, OUT);
    else if (y >= 26 || x >= 23) set(x, y, DARK);
    else if (y <= 16 && x <= 15) set(x, y, LIGHT);
    else set(x, y, BODY);
  }
for (const [x, y] of [[9, 15], [10, 15], [9, 16], [10, 14]]) set(x, y, SHINE);
for (const ex of [12, 18]) {
  for (let y = 19; y <= 21; y++) for (let x = ex; x <= ex + 1; x++) set(x, y, EYE);
  set(ex, 19, SHINE);
}
for (const x of [14, 15, 16]) set(x, 24, OUT);

const raw = Buffer.alloc(H * (W * 4 + 1));
for (let y = 0; y < H; y++) {
  raw[y * (W * 4 + 1)] = 0;
  for (let x = 0; x < W; x++) {
    const c = px[y * W + x];
    if (c) raw.set(hex(c), y * (W * 4 + 1) + 1 + x * 4);
  }
}
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr.set([8, 6, 0, 0, 0], 8);
writeFileSync(new URL("./slime.png", import.meta.url), Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
console.log("slime.png criado");
