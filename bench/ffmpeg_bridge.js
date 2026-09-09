import { outputOptions, errorMessage, validateArgs, validateInputs, bridgeLimits, createQueue, INPUT_OPTIONS, streamCopyCheckArgs, remainingTime } from './ffmpeg_args.js';

let ffmpegInstance = null;
const serialize = createQueue();
let limits = bridgeLimits();
let nextJob = 0;

async function initialize(ffmpeg, options) {
    const methods = ['exec', 'on', 'off', 'writeFile', 'readFile', 'deleteFile', 'createDir', 'deleteDir'];
    if (!ffmpeg || methods.some(method => typeof ffmpeg[method] !== 'function')) throw new TypeError('Invalid FFmpeg instance');
    const configured = bridgeLimits(options);
    await serialize(() => { ffmpegInstance = ffmpeg; limits = configured; });
}
function isAvailable() { return ffmpegInstance !== null && ffmpegInstance.loaded !== false; }

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
    const ffmpeg = ffmpegInstance;
    const directory = `/mmm-${Date.now().toString(36)}-${++nextJob}`;
    const files = [];
    const logs = [];
    let created = false;
    let listening = false;
    const onLog = ({ message }) => { logs.push(String(message).slice(-2048)); if (logs.length > 20) logs.shift(); };
    try {
        await ffmpeg.createDir(directory);
        created = true;
        const outputFile = `${directory}/output.${output.extension}`;
        files.push(outputFile);
        ffmpeg.on('log', onLog);
        listening = true;
        const paths = new Map();
        for (const [index, input] of inputs.entries()) {
            const path = `${directory}/input-${index}-${input.name}`;
            paths.set(input.name, path);
            files.push(path);
            await ffmpeg.writeFile(path, input.data);
        }
        const args = [...output.args];
        // Work backwards so inserting input restrictions preserves parsed indexes.
        for (const index of [...output.inputIndexes].reverse()) {
            args.splice(index, 2, ...INPUT_OPTIONS, '-i', paths.get(args[index + 1]));
        }
        const inputArgs = multiInput ? [] : [...INPUT_OPTIONS, '-i', paths.get(inputs[0].name)];
        const started = Date.now();
        const code = await ffmpeg.exec(['-nostdin', '-y', ...inputArgs, ...args, outputFile], limits.timeoutMs);
        if (code !== 0) throw new Error(`FFmpeg exited with code ${code}: ${logs.slice(-5).join(' | ')}`);
        if (output.codecs.includes('copy')) {
            logs.length = 0;
            const check = await ffmpeg.exec(streamCopyCheckArgs(outputFile), remainingTime(started, limits.timeoutMs));
            if (check !== 0) throw new Error(`Stream-copy output failed its initial decode check: ${logs.slice(-5).join(' | ')}`);
        }
        const data = await ffmpeg.readFile(outputFile);
        if (!(data instanceof Uint8Array) || data.length === 0) throw new Error('FFmpeg produced no output');
        if (data.byteLength > limits.maxOutputBytes) throw new Error('FFmpeg output exceeds the configured byte limit');
        return { data };
    } catch (error) {
        return { error: errorMessage(error) };
    } finally {
        // Cleanup must not replace a successful result or the original error.
        if (listening) { try { ffmpeg.off('log', onLog); } catch {} }
        await Promise.allSettled(files.map(async file => ffmpeg.deleteFile(file)));
        if (created) { try { await ffmpeg.deleteDir(directory); } catch {} }
    }
}
function getVersion() {
    return serialize(async () => {
        if (!isAvailable()) return 'FFmpeg not available';
        const ffmpeg = ffmpegInstance;
        const logs = [];
        let listening = false;
        const onLog = ({ message }) => { if (logs.length < 20) logs.push(String(message).slice(-2048)); };
        try {
            ffmpeg.on('log', onLog);
            listening = true;
            const code = await ffmpeg.exec(['-version'], Math.min(limits.timeoutMs, 10000));
            return code === 0 ? logs.join('\n') : 'Version check failed';
        } catch { return 'Version check failed'; }
        finally { if (listening) { try { ffmpeg.off('log', onLog); } catch {} } }
    });
}
globalThis.__ffmpeg__ = { initialize, isAvailable, validateArgs, execute, executeMultiInput, getVersion };
export { initialize, isAvailable, validateArgs, execute, executeMultiInput, getVersion };
