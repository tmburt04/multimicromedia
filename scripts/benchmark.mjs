/** Measurement runner; generated media and raw observations stay under target/. */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { cpus, platform, arch } from 'node:os';
import * as bridge from '../bench/ffmpeg_node.mjs';
import { renderReport } from './benchmark-report.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const args = process.argv.slice(2);
const usage = 'Usage: node scripts/benchmark.mjs [--repeats=1..20] [--label=name] [--reuse-media] [--extended] [--verify-decode]';
if (args.length === 1 && args[0] === '--help') {
    console.log(usage);
    process.exit(0);
}
if (args.some(arg => !['--reuse-media', '--extended', '--verify-decode'].includes(arg) && !/^--(repeats|label)=/.test(arg))) throw new Error(usage);
const reuseMedia = args.includes('--reuse-media');
const extended = args.includes('--extended');
const verifyDecode = args.includes('--verify-decode');
const option = (key, fallback) => args.find(arg => arg.startsWith(`--${key}=`))?.slice(key.length + 3) ?? fallback;
const repeats = Number(option('repeats', '3'));
const label = option('label', 'current');
if (!Number.isInteger(repeats) || repeats < 1 || repeats > 20 || !/^[a-z0-9_-]+$/i.test(label)) {
    throw new Error(usage);
}
const directory = resolve(root, 'target/benchmarks');
const mediaDirectory = join(directory, 'media');
await mkdir(mediaDirectory, { recursive: true });
const require = createRequire(import.meta.url);
let wasm;
try { wasm = require('../pkg/nodejs/mmm-node.js'); }
catch { throw new Error('Build Node bindings first: node scripts/build.mjs all'); }
let ffmpeg = process.env.FFMPEG_PATH;
if (!ffmpeg) { try { ffmpeg = require('ffmpeg-static'); } catch { ffmpeg = 'ffmpeg'; } }

function run(command, commandArgs) {
    return new Promise((resolveRun, reject) => {
        const child = spawn(command, commandArgs, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
        let stdout = '', stderr = '';
        const timer = setTimeout(() => child.kill('SIGKILL'), 30_000);
        child.stdout.on('data', data => { stdout = (stdout + data).slice(-32768); });
        child.stderr.on('data', data => { stderr = (stderr + data).slice(-32768); });
        child.once('error', error => { clearTimeout(timer); reject(error); });
        child.once('close', code => {
            clearTimeout(timer);
            if (code === 0) resolveRun(stdout + stderr);
            else reject(new Error(stderr.slice(-3000) || `process exited ${code}`));
        });
    });
}
const version = (await run(ffmpeg, ['-version'])).split('\n')[0];
if (!await bridge.initialize({ timeoutMs: 15_000, maxOutputBytes: 32 * 1024 * 1024 })) {
    throw new Error('FFmpeg initialization failed; set FFMPEG_PATH or install ffmpeg-static.');
}
const sources = [], unavailable = [];
async function generate(name, category, commandArgs) {
    const path = join(mediaDirectory, name);
    try {
        if (!reuseMedia) await run(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-y', ...commandArgs, path]);
        const data = await readFile(path);
        sources.push({ name, category, data, sha256: createHash('sha256').update(data).digest('hex') });
    } catch (error) {
        if (reuseMedia) throw new Error(`Cannot reuse ${name}: ${error.message}. Run without --reuse-media to generate fixtures.`);
        unavailable.push({ name, error: error.message });
    }
}
// A deterministic mixture of gradients, sharp edges, and pseudo-random texture.
const width = 192, height = 128, rgba = Buffer.alloc(width * height * 4);
let seed = 123456789;
for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const i = (y * width + x) * 4;
    rgba[i] = x < width / 2 ? x * 255 / width : seed >>> 24;
    rgba[i + 1] = y * 255 / height;
    rgba[i + 2] = (x ^ y) & 255;
    rgba[i + 3] = x < 16 ? x * 16 : 255;
}
const raw = join(mediaDirectory, 'source.rgba');
await writeFile(raw, rgba);
const imageInput = ['-f', 'rawvideo', '-pixel_format', 'rgba', '-video_size', `${width}x${height}`, '-i', raw, '-frames:v', '1'];
for (const [extension, settings] of [
    ['png', []], ['jpg', ['-q:v', '2']], ['webp', ['-c:v', 'libwebp', '-lossless', '1']],
    ['gif', []], ['bmp', []], ['tiff', []], ['ico', []],
    ['avif', ['-c:v', 'libaom-av1', '-cpu-used', '8', '-crf', '24']]
]) await generate(`image.${extension}`, 'image', [...imageInput, ...settings]);
await generate('gray16.png', 'image', [...imageInput, '-pix_fmt', 'gray16be']);
await generate('large.png', 'stress', [...imageInput, '-vf', 'scale=1536:1024']);
await generate('animated.gif', 'animation', ['-f', 'lavfi', '-i', 'testsrc2=size=128x96:rate=5:duration=0.8']);
await generate('animated.png', 'animation', ['-f', 'lavfi', '-i', 'testsrc2=size=128x96:rate=5:duration=0.8', '-f', 'apng', '-plays', '0']);
await generate('animated.webp', 'animation', ['-f', 'lavfi', '-i', 'testsrc2=size=128x96:rate=5:duration=0.8', '-c:v', 'libwebp_anim', '-loop', '0']);
const png = sources.find(source => source.name === 'image.png').data;
for (const [name, body] of [
    ['vector.svg', '<!-- retained geometry --><rect x="10" y="10" width="100" height="60" fill="red"/>'],
    ['embedded.svg', `<image width="192" height="128" href="data:image/png;base64,${png.toString('base64')}"/>`]
]) sources.push({ name, category: 'svg', data: Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="192" height="128">${body}</svg>`) });
const audio = ['-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=44100:duration=0.6'];
for (const [extension, settings] of [
    ['wav', []], ['mp3', ['-c:a', 'libmp3lame', '-b:a', '128k']], ['flac', []],
    ['ogg', ['-c:a', 'libvorbis']], ['aac', ['-c:a', 'aac']], ['opus', ['-c:a', 'libopus']],
    ['ac3', ['-c:a', 'ac3']], ['aiff', []], ['wma', ['-c:a', 'wmav2']],
    ['amr', ['-c:a', 'libopencore_amrnb', '-ar', '8000', '-ac', '1', '-b:a', '12.2k']]
]) await generate(`audio.${extension}`, 'audio', [...audio, ...settings]);
for (const [extension, vc, ac] of [
    ['mp4', 'libx264', 'aac'], ['mov', 'libx264', 'aac'], ['mkv', 'libx264', 'aac'],
    ['webm', 'libvpx-vp9', 'libopus'], ['avi', 'mpeg4', 'libmp3lame'],
    ['wmv', 'wmv2', 'wmav2'], ['flv', 'flv', 'libmp3lame'], ['mpg', 'mpeg2video', 'mp2']
]) await generate(`video.${extension}`, 'video', ['-f', 'lavfi', '-i', 'testsrc2=size=128x96:rate=25:duration=0.6', ...audio, '-c:v', vc, '-c:a', ac, '-shortest']);

if (extended) {
    await generate('tiny.png', 'image', [...imageInput, '-vf', 'scale=1:1']);
    await generate('rgba16.png', 'image', [...imageInput, '-pix_fmt', 'rgba64be']);
    await generate('gray-alpha.png', 'image', [...imageInput, '-pix_fmt', 'ya8']);
    await generate('stereo.wav', 'audio', [...audio, '-ar', '48000', '-ac', '2']);
    await generate('lowrate.wav', 'audio', [...audio, '-ar', '8000']);
    await generate('silent.mp4', 'video', ['-f', 'lavfi', '-i', 'testsrc2=size=128x96:rate=25:duration=0.6', '-c:v', 'libx264']);
    await generate('portrait.mp4', 'video', ['-f', 'lavfi', '-i', 'testsrc2=size=96x128:rate=25:duration=0.6', ...audio, '-ac', '2', '-c:v', 'libx264', '-c:a', 'aac', '-shortest']);
}
for (const source of sources) source.sha256 ??= createHash('sha256').update(source.data).digest('hex');

const cases = [];
const add = (source, name, config) => {
    source.sha256 ??= createHash('sha256').update(source.data).digest('hex');
    cases.push({ source, name, config });
};
const imageOutputs = ['png', 'jpeg', 'webp', 'gif', 'bmp', 'tiff', 'ico', 'avif'];
const audioOutputs = ['mp3', 'wav', 'flac', 'ogg', 'aac', 'opus', 'ac3', 'aiff', 'amr', 'wma'];
const videoOutputs = ['mp4', 'webm', 'mov', 'avi', 'mkv', 'wmv', 'flv', 'mpeg'];
for (const source of sources) {
    add(source, 'defaults', {});
    const outputs = source.category === 'image' ? imageOutputs : source.category === 'audio' ? audioOutputs : source.category === 'video' ? [...videoOutputs, ...audioOutputs] : [];
    for (const output_format of outputs) for (const quality of [20, 80, 100]) {
        add(source, `convert/${output_format}/q${quality}`, { output_format, quality });
    }
    if (['image', 'animation', 'svg', 'video', 'stress'].includes(source.category)) {
        for (const mode of ['fit', 'fill', 'cover', 'exact']) {
            add(source, `resize/${mode}`, { resize: { width: 65, height: 49, mode } });
        }
        add(source, 'resize/width', { resize: { width: 64 } });
        add(source, 'resize/stretch', { resize: { width: 64, preserve_aspect: false } });
        add(source, 'crop', { crop: { x: 4, y: 4, width: 64, height: 48 } });
        add(source, 'crop+resize', { crop: { x: 4, y: 4, width: 64, height: 48 }, resize: { width: 32 } });
    }
    if (['audio', 'video'].includes(source.category)) {
        add(source, 'trim/end', { trim: { end_ms: 300 } });
        add(source, 'trim/start+end', { trim: { start_ms: 100, end_ms: 400 } });
        add(source, 'trim/duration', { trim: { duration_ms: 300 } });
        add(source, 'codec/copy', { ffmpeg: source.category === 'audio' ? { audio_codec: 'copy' } : { video_codec: 'copy', audio_codec: 'copy' } });
    }
    add(source, 'metadata/strip', { preserve_metadata: false });
    if (extended && ['audio', 'video'].includes(source.category)) {
        const copy = source.category === 'audio' ? { audio_codec: 'copy' } : { video_codec: 'copy', audio_codec: 'copy' };
        add(source, 'copy+trim', { ffmpeg: copy, trim: { start_ms: 100, duration_ms: 300 }, preserve_metadata: false });
        for (const output_format of source.category === 'audio' ? audioOutputs : videoOutputs) {
            add(source, `remux/${output_format}`, { ffmpeg: copy, output_format });
        }
    }
    if (extended && source.category === 'image') {
        for (const output_format of ['png', 'jpeg', 'webp', 'gif', 'ico']) for (const quality of [20, 80]) {
            add(source, `combined/${output_format}/q${quality}`, { output_format, quality,
                resize: { width: 64, height: 48, mode: 'cover' }, preserve_metadata: false });
        }
    }
}
for (const source of sources.filter(s => ['image.png', 'gray16.png'].includes(s.name))) {
    for (const compression_level of [0, 1, 6, 9]) for (const max_colors of [2, 16, 256]) {
        add(source, `png/level${compression_level}/colors${max_colors}`, { png: { compression_level, max_colors, quantize: true } });
    }
}
const sample = sources.find(s => s.name === 'image.png');
for (const [name, config] of Object.entries({
    'unknown-field': { quailty: 50 }, 'zero-quality': { quality: 0 }, 'zero-resize': { resize: { width: 0 } },
    'empty-resize': { resize: {} }, 'overflow-crop': { crop: { x: 4294967295, y: 0, width: 2, height: 2 } },
    'zero-trim': { trim: { end_ms: 0 } }, 'ambiguous-trim': { trim: { end_ms: 30, duration_ms: 10 } },
    'unknown-output': { output_format: 'not-a-format' }, 'unsupported-output': { output_format: 'avif' },
    'memory-limit': { resize: { width: 100000, height: 100000, mode: 'exact' } },
    'jpeg-compatibility': { jpeg: { progressive: true } }, 'webp-compatibility': { webp: { method: 6 } },
    'png-compatibility': { png: { interlaced: true } }, 'input-hint': { input_hint: 'image/jpeg' }
})) add(sample, `config/${name}`, config);
for (const [name, data] of [['empty', Buffer.alloc(0)], ['unknown', Buffer.from('not media')], ['truncated-png', png.subarray(0, 24)]]) {
    add({ name, data, category: 'malformed' }, 'defaults', {});
}
const video = sources.find(s => s.name === 'video.mp4');
for (const config of [
    { ffmpeg: { video_codec: 'copy' }, resize: { width: 64 } },
    { output_format: 'avi', ffmpeg: { crf: 60 } },
    { ffmpeg: { extra_flags: ['-made-up', '1'] } },
    { ffmpeg: { video_codec: 'nonexistent_encoder' } }
]) if (video) add(video, `config/${JSON.stringify(config)}`, config);

console.log(`Measuring ${cases.length} configurations x ${repeats} runs; ${sources.length} sources, ${unavailable.length} unavailable generators.`);
const observations = [];
const decodeCache = new Map();
const started = performance.now();
let compressionMs = 0;
// Warm up the module separately; the reported medians include bridge/process overhead.
const warmup = await wasm.compress(sample.data, '{}'); warmup.free();
for (const [index, item] of cases.entries()) {
    const configJson = JSON.stringify(item.config);
    let preflight;
    try { preflight = JSON.parse(wasm.validate_config_for_file(configJson, item.source.data)); }
    catch (error) { preflight = { valid: false, error: error.message, code: error.code }; }
    const times = [];
    const samples = [];
    let lastBytes;
    let outcome;
    for (let iteration = 0; iteration < repeats; iteration++) {
        let result;
        const start = performance.now();
        try {
            result = await wasm.compress(item.source.data, configJson);
            const stats = JSON.parse(result.stats_json());
            const reportedUnchanged = result.unchanged;
            const bytes = Buffer.from(result.into_data());
            result = undefined;
            lastBytes = bytes;
            times.push(performance.now() - start);
            outcome = { status: 'ok', stats, bytes: bytes.length, reportedUnchanged, unchanged: bytes.equals(item.source.data),
                sha256: createHash('sha256').update(bytes).digest('hex'), sizeContract: bytes.length <= item.source.data.length,
                requestedFormatApplied: !item.config.output_format || stats.format_out === ({ jpeg: 'jpg', mpeg: 'mpg' }[item.config.output_format] ?? item.config.output_format) };
            if (item.source.category === 'image') {
                try { outcome.dimensions = wasm.analyze_file(bytes); } catch (error) { outcome.analysisError = error.message; }
            }
        } catch (error) {
            lastBytes = undefined;
            times.push(performance.now() - start);
            outcome = { status: 'error', code: error.code ?? 'JS_ERROR', field: error.field, message: error.message ?? String(error) };
        } finally { result?.free(); }
        samples.push({ ms: times.at(-1), status: outcome.status, bytes: outcome.bytes, format: outcome.stats?.format_out, code: outcome.code });
    }
    compressionMs += times.reduce((a, b) => a + b, 0);
    if (verifyDecode && lastBytes && outcome.stats.format_out !== 'svg') {
        if (!decodeCache.has(outcome.sha256)) {
            const checkFile = join(directory, 'decode-check.bin');
            await writeFile(checkFile, lastBytes);
            try {
                await run(ffmpeg, ['-hide_banner', '-loglevel', 'error', '-xerror', '-i', checkFile, '-map', '0:v?', '-map', '0:a?', '-f', 'null', '-']);
                decodeCache.set(outcome.sha256, { valid: true });
            } catch (error) { decodeCache.set(outcome.sha256, { valid: false, message: error.message }); }
        }
        outcome.decode = decodeCache.get(outcome.sha256);
    }
    times.sort((a, b) => a - b);
    observations.push({ source: item.source.name, category: item.source.category, name: item.name, config: item.config,
        inputBytes: item.source.data.length, inputSha256: item.source.sha256, preflight, samples,
        medianMs: (times[Math.floor((times.length - 1) / 2)] + times[Math.floor(times.length / 2)]) / 2,
        minMs: times[0], maxMs: times.at(-1), ...outcome });
    if ((index + 1) % 40 === 0) console.log(`${index + 1}/${cases.length} (${((performance.now() - started) / 1000).toFixed(0)}s)`);
}
const report = { label, measuredAt: new Date().toISOString(), runtime: process.version, platform: `${platform()}/${arch()}`, cpu: cpus()[0]?.model,
    ffmpeg: version, repeats, reuseMedia, extended, verifyDecode, compressionSeconds: compressionMs / 1000,
    uniqueDecodedOutputs: decodeCache.size, seconds: (performance.now() - started) / 1000, wasmVersion: wasm.get_version(),
    memoryAtEnd: process.memoryUsage(), unavailable,
    sources: sources.map(({ data, ...source }) => ({ ...source, bytes: data.length })), observations };
await writeFile(join(directory, `${label}.json`), JSON.stringify(report, null, 2));
await writeFile(join(directory, `${label}.md`), renderReport(report));
console.log(`Saved target/benchmarks/${label}.{json,md}; ${report.seconds.toFixed(1)}s.`);
