// Generate placeholder app icons using only Node.js built-ins
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

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
  const typeBuf = Buffer.from(type, "ascii");
  const payload = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(payload));
  return Buffer.concat([len, payload, crc]);
}

function createPNG(size) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  // IHDR
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;  // bit depth
  ihdr[9] = 6;  // RGBA

  // Pixel data: deep navy background (#1a1a2e) with centered "F" in accent blue
  const raw = Buffer.alloc(size * (1 + size * 4));
  const cx = size / 2, cy = size / 2, radius = size * 0.38;

  for (let y = 0; y < size; y++) {
    const row = y * (1 + size * 4);
    raw[row] = 0; // filter: none
    for (let x = 0; x < size; x++) {
      const off = row + 1 + x * 4;
      const dx = x - cx, dy = y - cy;
      const dist = Math.sqrt(dx * dx + dy * dy);

      if (dist <= radius) {
        // Circle background: gradient from #1a1a2e to #16213e
        const t = dist / radius;
        raw[off]     = Math.round(0x1a + t * (0x16 - 0x1a));
        raw[off + 1] = Math.round(0x1a + t * (0x21 - 0x1a));
        raw[off + 2] = Math.round(0x2e + t * (0x3e - 0x2e));
        raw[off + 3] = 255;

        // Draw "F" letter in accent blue (#60a5fa)
        const nx = (x - cx) / radius, ny = (y - cy) / radius;
        const inF =
          // Vertical bar
          (nx >= -0.3 && nx <= -0.1 && ny >= -0.45 && ny <= 0.45) ||
          // Top horizontal bar
          (nx >= -0.3 && nx <= 0.35 && ny >= -0.45 && ny <= -0.25) ||
          // Middle horizontal bar
          (nx >= -0.3 && nx <= 0.2 && ny >= -0.1 && ny <= 0.1);

        if (inF) {
          raw[off]     = 0x60;
          raw[off + 1] = 0xa5;
          raw[off + 2] = 0xfa;
          raw[off + 3] = 255;
        }
      } else {
        // Transparent outside circle
        raw[off] = raw[off + 1] = raw[off + 2] = raw[off + 3] = 0;
      }
    }
  }

  const idat = pngChunk("IDAT", deflateSync(raw));
  const iend = pngChunk("IEND", Buffer.alloc(0));

  return Buffer.concat([sig, pngChunk("IHDR", ihdr), idat, iend]);
}

// Generate ICO (simple: just embed a 32x32 PNG)
function createICO(png32) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);     // reserved
  header.writeUInt16LE(1, 2);     // ICO type
  header.writeUInt16LE(1, 4);     // 1 image

  const entry = Buffer.alloc(16);
  entry[0] = 32;                   // width
  entry[1] = 32;                   // height
  entry[2] = 0;                    // colors
  entry[3] = 0;                    // reserved
  entry.writeUInt16LE(1, 4);      // planes
  entry.writeUInt16LE(32, 6);     // bits per pixel
  entry.writeUInt32LE(png32.length, 8); // size
  entry.writeUInt32LE(22, 12);    // offset (6 + 16)

  return Buffer.concat([header, entry, png32]);
}

const iconsDir = new URL("../src-tauri/icons", import.meta.url).pathname;
mkdirSync(iconsDir, { recursive: true });

const png32 = createPNG(32);
const png128 = createPNG(128);

writeFileSync(`${iconsDir}/32x32.png`, png32);
writeFileSync(`${iconsDir}/128x128.png`, png128);
writeFileSync(`${iconsDir}/icon.ico`, createICO(png32));
writeFileSync(`${iconsDir}/icon.png`, createPNG(512));

console.log("Icons generated in src-tauri/icons/");
