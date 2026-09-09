/** Optional Node bridge; the Rust/WASM library has no native dependencies. */
import { spawn } from 'node:child_process';
import { writeFile, readFile, rm, mkdtemp, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { outputOptions, errorMessage, validateArgs as validateOutputArgs, validateInputs, bridgeLimits, createQueue, INPUT_OPTIONS, streamCopyCheckArgs, remainingTime } from './ffmpeg_args.js';

let ffmpegPath = null;
let encoders = null;
let limits = bridgeLimits();
const serialize = createQueue();
async function initialize(options) {
    const configured = bridgeLimits(options);
    return serialize(async () => {
        limits = configured;
        if (ffmpegPath) return true;
        const candidates = [process.env.FFMPEG_PATH];
        try { candidates.push((await import('ffmpeg-static')).default); } catch {}
        candidates.push('ffmpeg');
        for (const candidate of candidates.filter(Boolean)) {
            try {
                await execPromise(candidate, ['-version'], 10000);
                ffmpegPath = candidate;
                try {
                    const { stdout } = await execPromise(candidate, ['-hide_banner', '-encoders'], 10000);
                    // Discard a truncated inventory instead of falsely rejecting valid codecs.
                    if (stdout.length < 262144) {
                        const names = [...stdout.matchAll(/^\s*[VAS][A-Z.]{5}\s+(\S+)([^\r\n]*)/gm)]
                            .flatMap(match => [match[1], match[2].match(/\(codec ([^)]+)\)/)?.[1]])
                            .filter(name => name && name !== '=');
                        if (names.length) encoders = new Set(names);
                    }
                } catch { /* Inventory is optional; execution remains authoritative. */ }
                return true;
            } catch {}
        }
        return false;
    });
}
function execPromise(command, args, timeoutMs = 10000, cwd) {
    return new Promise((resolve, reject) => {
        const child = spawn(command, args, { windowsHide: true, cwd, stdio: ['ignore', 'pipe', 'pipe'] });
        let stdout = '';
        let stderr = '';
        let timedOut = false;
        const timer = setTimeout(() => { timedOut = true; child.kill('SIGKILL'); }, timeoutMs);
        timer.unref();
        child.stdout.setEncoding('utf8');
        child.stderr.setEncoding('utf8');
        child.stdout.on('data', data => { stdout = (stdout + data).slice(-262144); });
        child.stderr.on('data', data => { stderr = (stderr + data).slice(-16384); });
        child.once('error', error => { clearTimeout(timer); reject(error); });
        // Wait for close, including after a timeout, before deleting job files.
        child.once('close', (code, signal) => {
            clearTimeout(timer);
            if (timedOut) reject(new Error(`FFmpeg exceeded its ${timeoutMs} ms execution limit`));
            else if (code === 0) resolve({ stdout, stderr });
            else reject(new Error(`FFmpeg exited with ${signal || `code ${code}`}: ${stderr}`));
        });
    });
}
function isAvailable() { return ffmpegPath !== null; }
function validateArgs(args) { return validateOutputArgs(args, encoders); }
function execute(args, inputData) {
    return enqueue([{ name: 'input.bin', data: inputData }], args, false);
}
function executeMultiInput(inputs, args) {
    return enqueue(inputs, args, true);
}
function enqueue(inputs, args, multiInput) {
    try {
        const input = validateInputs(inputs);
        const output = outputOptions(args, multiInput ? input.names : null);
        return serialize(() => run(input.files, output, multiInput), input.bytes, limits);
    } catch (error) { return Promise.resolve({ error: errorMessage(error) }); }
}
async function run(inputs, output, multiInput) {
    if (!isAvailable()) return { error: 'FFmpeg not available' };
    let directory;
    try {
        directory = await mkdtemp(join(tmpdir(), 'mmm-ffmpeg-'));
        const outputFile = join(directory, `output.${output.extension}`);
        const paths = new Map();
        for (const [index, input] of inputs.entries()) {
            const path = join(directory, `input-${index}-${input.name}`);
            paths.set(input.name, path);
            await writeFile(path, input.data);
        }
        const args = [...output.args];
        for (const index of [...output.inputIndexes].reverse()) {
            args.splice(index, 2, ...INPUT_OPTIONS, '-i', paths.get(args[index + 1]));
        }
        const inputArgs = multiInput ? [] : [...INPUT_OPTIONS, '-i', paths.get(inputs[0].name)];
        const started = Date.now();
        await execPromise(ffmpegPath, ['-hide_banner', '-loglevel', 'error', '-nostdin', '-y', ...inputArgs, ...args, outputFile], limits.timeoutMs, directory);
        const info = await stat(outputFile);
        if (!info.isFile() || info.size > limits.maxOutputBytes) throw new Error('FFmpeg output exceeds the configured byte limit or is not a file');
        if (output.codecs.includes('copy')) {
            try { await execPromise(ffmpegPath, streamCopyCheckArgs(outputFile), remainingTime(started, limits.timeoutMs), directory); }
            catch (error) { throw new Error(`Stream-copy output failed its initial decode check: ${errorMessage(error)}`); }
        }
        // Buffer already extends Uint8Array; avoid duplicating the whole output.
        const data = await readFile(outputFile);
        if (!data.length) throw new Error('FFmpeg produced no output');
        return { data };
    } catch (error) { return { error: errorMessage(error) }; }
    finally {
        // directory comes directly from mkdtemp under the named mmm-ffmpeg prefix.
        if (directory) {
            try { await rm(directory, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 }); } catch {}
        }
    }
}
function getVersion() {
    return serialize(async () => {
        if (!isAvailable()) return 'FFmpeg not available';
        try { return (await execPromise(ffmpegPath, ['-version'])).stdout.split('\n')[0]; }
        catch { return 'Version check failed'; }
    });
}
globalThis.__ffmpeg__ = { initialize, isAvailable, validateArgs, execute, executeMultiInput, getVersion };
export { initialize, isAvailable, validateArgs, execute, executeMultiInput, getVersion };
