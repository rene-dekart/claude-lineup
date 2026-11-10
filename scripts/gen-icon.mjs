// Generates a placeholder 32x32 icon.ico so tauri-build can produce the
// Windows resource file. Replace with a real logo via `npx tauri icon logo.png`.
import { writeFileSync, mkdirSync } from 'node:fs'

const W = 32, H = 32
const BG = [0x1e, 0x29, 0x3b, 0xff] // BGRA: slate #1e293b
const FG = [0xf8, 0xfa, 0xfc, 0xff] // near-white

// 16x16 inner square accent — enough to not look broken in a taskbar.
const xor = Buffer.alloc(W * H * 4)
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const inner = x >= 8 && x < 24 && y >= 8 && y < 24
    const [b, g, r, a] = inner ? FG : BG
    const i = (y * W + x) * 4
    xor[i] = b; xor[i + 1] = g; xor[i + 2] = r; xor[i + 3] = a
  }
}
// AND mask: 32 bits per row, all opaque.
const and = Buffer.alloc(H * 4)

const bmp = Buffer.alloc(40)
bmp.writeUInt32LE(40, 0)      // biSize
bmp.writeInt32LE(W, 4)        // biWidth
bmp.writeInt32LE(H * 2, 8)    // biHeight: XOR + AND
bmp.writeUInt16LE(1, 12)      // biPlanes
bmp.writeUInt16LE(32, 14)     // biBitCount

const payload = Buffer.concat([bmp, xor, and])
const dir = Buffer.alloc(6)
dir.writeUInt16LE(0, 0)
dir.writeUInt16LE(1, 2)       // type: icon
dir.writeUInt16LE(1, 4)       // count

const entry = Buffer.alloc(16)
entry[0] = W
entry[1] = H
entry.writeUInt16LE(1, 4)     // planes
entry.writeUInt16LE(32, 6)    // bit count
entry.writeUInt32LE(payload.length, 8)
entry.writeUInt32LE(22, 12)   // offset

mkdirSync('src-tauri/icons', { recursive: true })
writeFileSync('src-tauri/icons/icon.ico', Buffer.concat([dir, entry, payload]))
console.log('src-tauri/icons/icon.ico written:', 22 + payload.length, 'bytes')
