#!/usr/bin/env node
// Generates minimal valid icon files at src-tauri/icons/ from a single
// solid-color design. Replace later with `npx @tauri-apps/cli icon <source.png>`.

import { writeFileSync, mkdirSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { Buffer } from "node:buffer";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const iconsDir = path.resolve(__dirname, "..", "src-tauri", "icons");
mkdirSync(iconsDir, { recursive: true });

const BRAND = [37, 99, 235, 255]; // blue-600  — idle / normal
const OK = [16, 185, 129, 255]; //  emerald-500 — last detection matched
const WARN = [239, 68, 68, 255]; //  red-500     — last detection mismatched
const FG = [255, 255, 255, 255]; // foreground for the "IP" glyphs

// 5x7 pixel glyphs for "I" and "P". 1 = filled, 0 = empty.
// Rendered at runtime-chosen scale, centered inside the disk.
const GLYPH_I = [
  [1, 1, 1, 1, 1],
  [0, 0, 1, 0, 0],
  [0, 0, 1, 0, 0],
  [0, 0, 1, 0, 0],
  [0, 0, 1, 0, 0],
  [0, 0, 1, 0, 0],
  [1, 1, 1, 1, 1],
];
const GLYPH_P = [
  [1, 1, 1, 1, 0],
  [1, 0, 0, 0, 1],
  [1, 0, 0, 0, 1],
  [1, 1, 1, 1, 0],
  [1, 0, 0, 0, 0],
  [1, 0, 0, 0, 0],
  [1, 0, 0, 0, 0],
];
// Text layout: "I" (5w) + 1col gap + "P" (5w) = 11 cols total, 7 rows tall.
const TEXT_W = 11;
const TEXT_H = 7;

function crc32(buf) {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let j = 0; j < 8; j++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[i] = c;
  }
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = table[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}

function makePng(size, color = BRAND, opts = {}) {
  const drawText = opts.drawText !== false;
  const [r, g, b, a] = color;
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  // RGBA raster + per-row filter byte 0
  const stride = 1 + size * 4;
  const raw = Buffer.alloc(size * stride);
  const cx = size / 2;
  const cy = size / 2;
  // Slightly inset radius so the disk has a hint of breathing room and
  // doesn't get cropped to a square by the rendering host at small sizes.
  const radius = size * 0.48;

  for (let y = 0; y < size; y++) {
    const rowStart = y * stride;
    raw[rowStart] = 0;
    for (let x = 0; x < size; x++) {
      const idx = rowStart + 1 + x * 4;
      const dx = x - cx;
      const dy = y - cy;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist > radius) {
        raw[idx] = 0;
        raw[idx + 1] = 0;
        raw[idx + 2] = 0;
        raw[idx + 3] = 0;
      } else {
        // Solid coloured disk. Adds a 1-pixel soft alpha at the edge for AA.
        const edge = radius - dist;
        const alpha = edge >= 1 ? a : Math.round(a * Math.max(0, edge));
        raw[idx] = r;
        raw[idx + 1] = g;
        raw[idx + 2] = b;
        raw[idx + 3] = alpha;
      }
    }
  }

  if (drawText) {
    drawIp(raw, size, FG);
  }

  const idatData = deflateSync(raw);
  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", idatData),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/**
 * Stamp "IP" onto the RGBA raster, centred. Chooses a pixel scale that fills
 * roughly 60% of the disk diameter — readable even at 16×16.
 */
function drawIp(raw, size, color) {
  // The text is rendered tight: column 5 of "I" already gives a 1-col gap to "P".
  // So the effective drawable text is 5 + 5 = 10 cols wide once we shift "P"
  // by 5 cols (column 5 of "I" is empty for rows 1–5, providing the gap).
  const scale = Math.max(
    1,
    Math.floor(Math.min((size * 0.7) / TEXT_W, (size * 0.7) / TEXT_H))
  );
  const drawW = TEXT_W * scale;
  const drawH = TEXT_H * scale;
  const x0 = Math.floor((size - drawW) / 2);
  const y0 = Math.floor((size - drawH) / 2);
  const stride = 1 + size * 4;
  const [cr, cg, cb, ca] = color;

  function setPixel(px, py) {
    if (px < 0 || px >= size || py < 0 || py >= size) return;
    const idx = py * stride + 1 + px * 4;
    raw[idx] = cr;
    raw[idx + 1] = cg;
    raw[idx + 2] = cb;
    raw[idx + 3] = ca;
  }

  function blit(glyph, gx, gy) {
    for (let r = 0; r < 7; r++) {
      for (let c = 0; c < 5; c++) {
        if (!glyph[r][c]) continue;
        const px0 = gx + c * scale;
        const py0 = gy + r * scale;
        for (let dy = 0; dy < scale; dy++) {
          for (let dx = 0; dx < scale; dx++) {
            setPixel(px0 + dx, py0 + dy);
          }
        }
      }
    }
  }

  blit(GLYPH_I, x0, y0);
  // "P" starts after the 5 cols of "I"; the gap between them comes from the
  // empty col-4 of "I" (rows 1–5) and col-4 of "P" being filled only on row 0/3.
  // We add an explicit 1-col scale-width gap for visual breathing room.
  blit(GLYPH_P, x0 + (5 + 1) * scale, y0);
}

function makeIco(pngs) {
  // ICONDIR + N x ICONDIRENTRY + N x image bytes. Windows Vista+ accepts PNG
  // images directly inside an ICO container.
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type icon
  header.writeUInt16LE(pngs.length, 4);
  const entries = [];
  const blobs = [];
  let offset = 6 + 16 * pngs.length;
  for (const { size, png } of pngs) {
    const e = Buffer.alloc(16);
    e[0] = size === 256 ? 0 : size; // width
    e[1] = size === 256 ? 0 : size; // height
    e[2] = 0; // colour palette
    e[3] = 0; // reserved
    e.writeUInt16LE(1, 4); // colour planes
    e.writeUInt16LE(32, 6); // bpp
    e.writeUInt32LE(png.length, 8);
    e.writeUInt32LE(offset, 12);
    entries.push(e);
    blobs.push(png);
    offset += png.length;
  }
  return Buffer.concat([header, ...entries, ...blobs]);
}

const sizes = [16, 32, 48, 64, 128, 256];
const pngBySize = Object.fromEntries(
  sizes.map((s) => [s, makePng(s)])
);

writeFileSync(path.join(iconsDir, "32x32.png"), pngBySize[32]);
writeFileSync(path.join(iconsDir, "128x128.png"), pngBySize[128]);
writeFileSync(path.join(iconsDir, "128x128@2x.png"), pngBySize[256]);
writeFileSync(path.join(iconsDir, "icon.png"), pngBySize[256]);

// Tray icon variants — 64x64 is the largest size Windows renders in the system
// tray; macOS and Linux scale these down. Three colours map to the runtime
// states surfaced in src-tauri/src/tray.rs.
writeFileSync(path.join(iconsDir, "tray-idle.png"), makePng(64, BRAND));
writeFileSync(path.join(iconsDir, "tray-ok.png"), makePng(64, OK));
writeFileSync(path.join(iconsDir, "tray-warn.png"), makePng(64, WARN));

// Windows .ico — bundle 16/32/48/64/128/256
const ico = makeIco(sizes.map((s) => ({ size: s, png: pngBySize[s] })));
writeFileSync(path.join(iconsDir, "icon.ico"), ico);

// Minimal .icns: an ICNS file is composed of icon family + sub-types.
// For an absolute minimum we embed a 256x256 PNG under "ic08".
function makeIcns(png) {
  const header = Buffer.from("icns", "ascii");
  const size = Buffer.alloc(4);
  size.writeUInt32BE(8 + 8 + png.length, 0);
  const subType = Buffer.from("ic08", "ascii"); // 256x256 RGBA
  const subSize = Buffer.alloc(4);
  subSize.writeUInt32BE(8 + png.length, 0);
  return Buffer.concat([header, size, subType, subSize, png]);
}
writeFileSync(path.join(iconsDir, "icon.icns"), makeIcns(pngBySize[256]));

console.log(`icons written to ${iconsDir}`);
