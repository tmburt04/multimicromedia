/** PNG measurements with independent decoding; generated files stay under target/. */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { deflateSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';

const root = new URL('../', import.meta.url);
const require = createRequire(import.meta.url);
const args = process.argv.slice(2);
const usage = 'node scripts/benchmark-png.mjs [--label=name] [--wasm=path/to/mmm-node.js]';
if (args.includes('--help')) { console.log(usage); process.exit(0); }
if (args.some(arg => !/^--(label|wasm)=.+/.test(arg))) throw new Error(usage);
const option = (name, fallback) => args.find(arg => arg.startsWith(`--${name}=`))?.slice(name.length + 3) ?? fallback;
const label = option('label', 'current');
if (!/^[a-z0-9_-]+$/i.test(label)) throw new Error(usage);
const wasm = require(resolve(option('wasm', 'pkg/nodejs/mmm-node.js')));
let ffmpeg = process.env.FFMPEG_PATH;
if (!ffmpeg) { try { ffmpeg = require('ffmpeg-static'); } catch { ffmpeg = 'ffmpeg'; } }
const directory = new URL(`target/png-patch/${label}/`, root);
await mkdir(directory, { recursive: true });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

function chunk(type, data) {
    const result = Buffer.alloc(data.length + 12);
    result.writeUInt32BE(data.length);
    result.write(type, 4);
    data.copy(result, 8);
    let crc = 0xffffffff;
    for (const byte of result.subarray(4, -4)) {
        crc ^= byte;
        for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
    result.writeUInt32BE((crc ^ 0xffffffff) >>> 0, result.length - 4);
    return result;
}
function fixture(colors, alpha = false, texture = false) {
    const width = texture ? 768 : 257, height = texture ? 512 : 193, channels = alpha ? 4 : 3;
    const rows = Buffer.alloc(height * (width * channels + 1));
    let seed = 123456789;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
        seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
        const color = ((x / 13 | 0) + (y / 7 | 0) * 3) % colors;
        const i = y * (width * channels + 1) + 1 + x * channels;
        rows[i] = texture ? (x / 3 + (seed >>> 27)) & 255 : color * 71 & 255;
        rows[i + 1] = texture ? (y / 2 + (seed >>> 28)) & 255 : color * 137 & 255;
        rows[i + 2] = texture ? ((x + y) / 5 + (seed >>> 26)) & 255 : color * 233 & 255;
        if (alpha) rows[i + 3] = color % 3 === 0 ? 0 : color % 3 === 1 ? 127 : 255;
    }
    const header = Buffer.alloc(13);
    header.writeUInt32BE(width); header.writeUInt32BE(height, 4);
    header[8] = 8; header[9] = alpha ? 6 : 2;
    return Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'), chunk('IHDR', header), chunk('IDAT', deflateSync(rows, { level: 9 })), chunk('IEND', Buffer.alloc(0))]);
}
function decode(bytes, pixelFormat = 'rgba') {
    const decoded = spawnSync(ffmpeg, ['-v', 'error', '-xerror', '-i', 'pipe:0', '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', pixelFormat, 'pipe:1'],
        { input: bytes, timeout: 30_000, maxBuffer: 64 * 1024 * 1024, windowsHide: true });
    if (decoded.status !== 0) throw new Error(decoded.error?.message || decoded.stderr?.toString() || 'Decode failed');
    return decoded.stdout;
}
const sources = [2, 4, 16, 256].map(colors => ({ name: `rgb-${colors}`, bytes: fixture(colors) }));
sources.push({ name: 'rgba-16', bytes: fixture(16, true) }, { name: 'texture', bytes: fixture(256, false, true) });
for (const name of ['image', 'gray16', 'rgba16', 'gray-alpha']) {
    try { sources.push({ name, bytes: await readFile(new URL(`target/benchmarks/media/${name}.png`, root)) }); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
}
const modes = [
    ['default', {}], ['lossless', { png: { quantize: false, compression_level: 9 } }],
    ...[2, 4, 16, 64, 256].map(max_colors => [`colors-${max_colors}`, { png: { quantize: true, max_colors, compression_level: 9 } }]),
    ['resize-lossless', { resize: { width: 127 }, png: { quantize: false } }],
    ['resize-colors-16', { resize: { width: 127 }, png: { quantize: true, max_colors: 16 } }],
    ['webp', { output_format: 'webp' }], ['jpeg', { output_format: 'jpeg', quality: 80 }]
];
const observations = [];
for (const source of sources) {
    // Keep 16-bit precision where present. Expanding 8-bit RGB and indexed PNGs
    // to 16 bits can use different FFmpeg scaling paths; compare native 8-bit samples.
    const pixelFormat = source.bytes[24] === 16 ? 'rgba64le' : 'rgba';
    const decodedSource = decode(source.bytes, pixelFormat);
    for (const [mode, config] of modes) {
        const times = [];
        let bytes, format;
        for (let iteration = 0; iteration < 3; iteration++) {
            const start = performance.now();
            const result = await wasm.compress(source.bytes, JSON.stringify(config));
            format = result.format_out;
            bytes = Buffer.from(result.into_data());
            times.push(performance.now() - start);
        }
        const decoded = decode(bytes, pixelFormat);
        const pixelsEqual = decoded.equals(decodedSource);
        await writeFile(new URL(`${source.name}-${mode}.${format}`, directory), bytes);
        observations.push({ source: source.name, mode, config, inputBytes: source.bytes.length, inputSha256: hash(source.bytes),
            bytes: bytes.length, format, savingsPct: (1 - bytes.length / source.bytes.length) * 100,
            times, medianMs: [...times].sort((a, b) => a - b)[1], decoded: true, pixelsEqual,
            sizeContract: bytes.length <= source.bytes.length, unchanged: bytes.equals(source.bytes),
            bitDepth: format === 'png' ? bytes[24] : null, colorType: format === 'png' ? bytes[25] : null });
    }
    console.log(`Measured ${source.name}`);
}
// Exercise non-PNG input too: the PNG settings must survive the conversion dispatcher.
const webp = await readFile(new URL('texture-webp.webp', directory));
for (const quantize of [false, true]) {
    const result = await wasm.compress(webp, JSON.stringify({ output_format: 'png', png: { quantize, max_colors: 16 } }));
    const format = result.format_out, bytes = Buffer.from(result.into_data());
    observations.push({ source: 'texture-webp', mode: quantize ? 'convert-colors-16' : 'convert-lossless', inputBytes: webp.length,
        bytes: bytes.length, format, decoded: !!decode(bytes), sizeContract: bytes.length <= webp.length,
        bitDepth: format === 'png' ? bytes[24] : null, colorType: format === 'png' ? bytes[25] : null });
}
const report = { version: wasm.get_version(), label, repetitions: 3, observations };
await writeFile(new URL('report.json', directory), JSON.stringify(report, null, 2));
const lines = [`# PNG benchmark: ${label}`, '', `WASM ${report.version}; three sequential repetitions per PNG configuration. Independent FFmpeg decoding verifies returned pixels at the source sample depth. Synthetic inputs; timings are not browser or visual-quality measurements.`, '',
    '| Input | Mode | Input bytes | Output bytes | Savings | Median ms | Pixels identical |', '| --- | --- | ---: | ---: | ---: | ---: | --- |'];
for (const item of observations.filter(item => item.times)) lines.push(`| ${item.source} | ${item.mode} | ${item.inputBytes} | ${item.bytes} | ${item.savingsPct.toFixed(2)}% | ${item.medianMs.toFixed(2)} | ${item.pixelsEqual} |`);
await writeFile(new URL('report.md', directory), lines.join('\n') + '\n');
console.log(JSON.stringify({ version: report.version, configurations: observations.length,
    oversized: observations.filter(item => !item.sizeContract).length,
    losslessPixelChanges: observations.filter(item => ['default', 'lossless'].includes(item.mode) && !item.pixelsEqual).length }));
