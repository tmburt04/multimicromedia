/** Shared bridge contract: args are output options; input probing is automatic. */
const MUXERS = new Map(Object.entries({ mkv: 'matroska', aac: 'adts', wmv: 'asf', wma: 'asf', mpg: 'mpeg', m4a: 'ipod' }));
const EXTENSIONS = new Map(Object.entries({ matroska: 'mkv', adts: 'aac', asf: 'wmv', mpeg: 'mpg', ipod: 'm4a', mjpeg: 'jpg', image2: 'png', image2pipe: 'png' }));
const FORMATS = new Set('mp4 mov matroska webm avi asf flv mpeg adts mp3 ogg opus flac wav aiff ac3 amr ipod image2 image2pipe mjpeg webp gif avif'.split(' '));
const SWITCHES = new Set(['-an', '-vn', '-sn', '-dn', '-shortest', '-bitexact']);
// Keep filenames and input/global options under the bridge's control. Filter
// expressions and codec settings are application configuration, not a sandbox.
const OPTIONS = new Set(`-f -c -codec -vcodec -acodec -b -q -qscale -crf -preset -tune
    -profile -level -pix_fmt -sample_fmt -movflags -fflags -flags -flags2 -threads
    -filter_threads -filter_complex_threads -s -r -ar -ac -channel_layout -ss -t -to
    -map -map_metadata -map_chapters -metadata -vf -af -filter -filter_complex
    -frames -vframes -aframes -deadline -cpu-used -row-mt -tile-columns -tile-rows
    -quality -compression_level -qmin -qmax -g -keyint_min -bf -refs -bufsize
    -maxrate -minrate -max_muxing_queue_size -fps_mode -vsync -strict
    -application -frame_duration -vbr -cutoff -brand -tag -avoid_negative_ts
    -disposition -color_range -colorspace -color_primaries -color_trc -sws_flags`.split(/\s+/));

// Restrict input probing to local media demuxers; custom filters remain trusted.
// These restrictions are not an FFmpeg sandbox.
export const INPUT_OPTIONS = Object.freeze(['-protocol_whitelist', 'file', '-format_whitelist',
    'mov,mp3,ogg,flac,wav,aiff,ac3,asf,amr,avi,flv,matroska,webm,mpeg,mpegvideo,mpegts,aac,image2,image2pipe,png_pipe,jpeg_pipe,webp_pipe,gif']);

// Muxers can exit successfully yet write unsupported codec/container combinations.
export function streamCopyCheckArgs(path) {
    return ['-hide_banner', '-loglevel', 'error', '-nostdin', '-xerror', ...INPUT_OPTIONS,
        '-i', path, '-map', '0:V?', '-map', '0:a?', '-frames:v', '1', '-frames:a', '1', '-f', 'null', '-'];
}

export function remainingTime(started, timeoutMs) {
    const remaining = timeoutMs - (Date.now() - started);
    if (remaining <= 0) throw new Error(`FFmpeg exceeded its ${timeoutMs} ms execution limit`);
    return remaining;
}

export function outputOptions(args, inputNames = null) {
    if (!Array.isArray(args) || args.length > 256 || args.some(arg => typeof arg !== 'string' || arg.length > 16384 || arg.includes('\0'))) {
        throw new TypeError('FFmpeg args must contain at most 256 bounded strings without NUL bytes');
    }
    const options = [...args];
    // Accept the older two -f contract, discarding only its leading input hint.
    if (options[0] === '-f' && options.lastIndexOf('-f') > 0) options.splice(0, 2);
    let format = null;
    let inputCount = 0;
    const inputIndexes = [];
    const codecs = [];
    for (let index = 0; index < options.length; index++) {
        const flag = options[index];
        if (SWITCHES.has(flag)) continue;
        const base = flag.split(':')[0];
        if (flag !== '-i' && !OPTIONS.has(base)) throw new Error(`Unsupported FFmpeg output option: ${flag}`);
        if (++index >= options.length || options[index] === '') throw new Error(`Missing value for ${flag}`);
        if (['-c', '-codec', '-vcodec', '-acodec'].includes(base)) codecs.push(options[index]);
        if (flag === '-i') {
            if (!inputNames?.has(options[index])) throw new Error('FFmpeg inputs must reference supplied filenames');
            inputCount++;
            inputIndexes.push(index - 1);
        } else if (flag === '-f') {
            if (format !== null) throw new Error('Specify exactly one FFmpeg output format');
            format = MUXERS.get(options[index]) || options[index];
            if (!FORMATS.has(format)) throw new Error(`Unsupported FFmpeg output format: ${format}`);
            options[index] = format;
        }
    }
    if (!format) throw new Error('Missing FFmpeg output format (-f)');
    if (inputNames && inputCount === 0) throw new Error('Multi-input execution requires at least one -i option');
    return { args: options, extension: EXTENSIONS.get(format) || format, inputIndexes, codecs };
}

export function validateInputs(inputs) {
    if (!Array.isArray(inputs) || inputs.length === 0 || inputs.length > 32) throw new Error('Supply between 1 and 32 FFmpeg inputs');
    const names = new Set();
    let bytes = 0;
    const files = inputs.map(input => {
        if (!input || typeof input.name !== 'string' || !/^[a-z0-9_][a-z0-9_.-]{0,127}$/i.test(input.name) || names.has(input.name)) {
            throw new Error('Input filenames must be unique, simple basenames');
        }
        if (!(input.data instanceof Uint8Array) || input.data.byteLength === 0) throw new TypeError('FFmpeg input data must be a nonempty Uint8Array');
        names.add(input.name);
        bytes += input.data.byteLength;
        return { name: input.name, data: input.data };
    });
    return { files, names, bytes };
}

/** Optional synchronous preflight hook used by WASM; null means no known issue. */
export function validateArgs(args, encoders = null) {
    try {
        const output = outputOptions(args);
        if (encoders) {
            for (const encoder of output.codecs) {
                if (encoder !== 'copy' && !encoders.has(encoder)) return `Encoder '${encoder}' is not listed by this FFmpeg build`;
            }
        }
        return null;
    } catch (error) { return errorMessage(error); }
}

export function bridgeLimits(options = {}) {
    const limits = { timeoutMs: 600000, maxQueuedBytes: 512 * 1024 * 1024, maxQueuedJobs: 16, maxOutputBytes: 512 * 1024 * 1024, ...options };
    for (const key of ['timeoutMs', 'maxQueuedBytes', 'maxQueuedJobs', 'maxOutputBytes']) {
        if (!Number.isSafeInteger(limits[key]) || limits[key] <= 0) throw new TypeError(`${key} must be a positive safe integer`);
    }
    if (limits.timeoutMs > 2147483647) throw new RangeError('timeoutMs exceeds the supported timer range');
    return limits;
}

export function createQueue() {
    let queue = Promise.resolve();
    let queuedBytes = 0;
    let queuedJobs = 0;
    return (operation, bytes = 0, limits = null) => {
        if (limits && (queuedJobs >= limits.maxQueuedJobs || bytes > limits.maxQueuedBytes - queuedBytes)) {
            return Promise.resolve({ error: 'FFmpeg queue capacity exceeded; wait for an active job to finish' });
        }
        queuedJobs++;
        queuedBytes += bytes;
        const job = queue.then(operation).finally(() => { queuedJobs--; queuedBytes -= bytes; });
        // Do not keep the last job's potentially large output alive in the queue.
        queue = job.then(() => {}, () => {});
        return job;
    };
}

export function errorMessage(error) {
    return String(error?.message || error || 'FFmpeg execution failed').slice(-16384);
}
