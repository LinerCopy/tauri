#!/usr/bin/env node
/**
 * Генерирует placeholder иконки для Tauri (PNG, ICO, ICNS).
 * Создаёт одноцветные (синие) квадраты нужных размеров.
 */
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';
import { deflateSync } from 'zlib';

const ICONS_DIR = join(import.meta.dirname, '..', 'src-tauri', 'icons');
mkdirSync(ICONS_DIR, { recursive: true });

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc ^= buf[i];
    for (let j = 0; j < 8; j++) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeAndData = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typeAndData));
  return Buffer.concat([len, typeAndData, crc]);
}

function createPng(width, height) {
  const r = 0x1a, g = 0x56, b = 0xdb, a = 0xff;

  const rowSize = 1 + width * 4;
  const raw = Buffer.alloc(rowSize * height);
  for (let y = 0; y < height; y++) {
    const offset = y * rowSize;
    raw[offset] = 0;
    for (let x = 0; x < width; x++) {
      const px = offset + 1 + x * 4;
      raw[px] = r;
      raw[px + 1] = g;
      raw[px + 2] = b;
      raw[px + 3] = a;
    }
  }

  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  const ihdrData = Buffer.alloc(13);
  ihdrData.writeUInt32BE(width, 0);
  ihdrData.writeUInt32BE(height, 4);
  ihdrData[8] = 8;
  ihdrData[9] = 6;
  ihdrData[10] = 0;
  ihdrData[11] = 0;
  ihdrData[12] = 0;

  const ihdr = pngChunk('IHDR', ihdrData);
  const idat = pngChunk('IDAT', deflateSync(raw));
  const iend = pngChunk('IEND', Buffer.alloc(0));

  return Buffer.concat([signature, ihdr, idat, iend]);
}

function createIco(pngBuf) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(1, 4);

  const entry = Buffer.alloc(16);
  entry[0] = 32;
  entry[1] = 32;
  entry[2] = 0;
  entry[3] = 0;
  entry.writeUInt16LE(1, 4);
  entry.writeUInt16LE(32, 6);
  entry.writeUInt32LE(pngBuf.length, 8);
  entry.writeUInt32LE(22, 12);

  return Buffer.concat([header, entry, pngBuf]);
}

function createIcns(png128) {
  const type = Buffer.from('ic07', 'ascii');
  const entryLen = Buffer.alloc(4);
  entryLen.writeUInt32BE(8 + png128.length);

  const magic = Buffer.from('icns', 'ascii');
  const totalLen = Buffer.alloc(4);
  totalLen.writeUInt32BE(8 + 8 + png128.length);

  return Buffer.concat([magic, totalLen, type, entryLen, png128]);
}

const sizes = [32, 128, 256];
const pngs = {};
for (const s of sizes) {
  pngs[s] = createPng(s, s);
}

writeFileSync(join(ICONS_DIR, '32x32.png'), pngs[32]);
writeFileSync(join(ICONS_DIR, '128x128.png'), pngs[128]);
writeFileSync(join(ICONS_DIR, '128x128@2x.png'), pngs[256]);
writeFileSync(join(ICONS_DIR, 'icon.ico'), createIco(pngs[32]));
writeFileSync(join(ICONS_DIR, 'icon.icns'), createIcns(pngs[128]));
writeFileSync(join(ICONS_DIR, 'android-icon.png'), createPng(512, 512));
writeFileSync(join(ICONS_DIR, 'ios-icon.png'), createPng(1024, 1024));

console.log('✓ Icons generated');
