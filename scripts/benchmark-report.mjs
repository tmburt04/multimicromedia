import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const escape = value => String(value ?? '').replace(/\|/g, '\\|').replace(/[\r\n]+/g, ' ');
const sum = (rows, fn) => rows.reduce((n, row) => n + fn(row), 0);
const successful = rows => rows.filter(row => row.status === 'ok');
const percentile = (values, p) => {
    if (!values.length) return null;
    const sorted = [...values].sort((a, b) => a - b);
    return p === .5 ? (sorted[Math.floor((sorted.length - 1) / 2)] + sorted[Math.floor(sorted.length / 2)]) / 2
        : sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)];
};
const number = value => value == null ? '—' : value.toFixed(2);
const saving = rows => {
    const input = sum(rows, r => r.inputBytes);
    return input ? 100 * (1 - sum(rows, r => r.bytes) / input) : null;
};
const table = (head, rows) => [
    `| ${head.join(' | ')} |`, `| ${head.map(() => '---').join(' | ')} |`,
    ...rows.map(row => `| ${row.map(escape).join(' | ')} |`), ''
];
function grouped(rows, key) {
    const groups = new Map();
    for (const row of rows) {
        const value = key(row);
        if (!groups.has(value)) groups.set(value, []);
        groups.get(value).push(row);
    }
    return [...groups];
}
function breakdown(rows, key) {
    return table(['Group', 'Cases', 'Completed', 'Smaller', 'Unchanged', 'Rejected in preflight', 'Runtime errors', 'Median ms', 'P95 ms', 'Bytes saved %'],
        grouped(rows, key).map(([name, cases]) => {
            const ok = successful(cases), times = ok.map(r => r.medianMs);
            return [name, cases.length, ok.length, ok.filter(r => r.bytes < r.inputBytes).length,
                ok.filter(r => r.unchanged).length, cases.filter(r => !r.preflight.valid && r.status === 'error').length,
                cases.filter(r => r.preflight.valid && r.status === 'error').length,
                number(percentile(times, .5)), number(percentile(times, .95)), number(saving(ok))];
        }));
}
function failureKind(row) {
    if (row.message.includes('initial decode check')) return 'Stream-copy output rejected after decode';
    if (row.name.startsWith('remux/')) return 'Explicit stream-copy/container request';
    if (row.source === 'silent.mp4' && row.name.startsWith('convert/')) return 'Audio extraction from silent video';
    if (row.field === 'trim.start_ms' && row.name === 'copy+trim') return 'Nonzero video start with stream copy';
    if (row.category === 'malformed') return 'Malformed input';
    if (!row.preflight.valid) return `Preflight rejection: ${row.field ?? row.code}`;
    return `${row.code}: ${row.source}`;
}
export function renderReport(report, previous) {
    const rows = report.observations, ok = successful(rows);
    const conversions = rows.filter(r => r.name.startsWith('convert/') || r.name.startsWith('combined/') || r.name.startsWith('remux/'));
    const lines = [`# Compression benchmark: ${report.label}`, '',
        `${rows.length.toLocaleString()} configurations × ${report.repeats} repetitions = ${(rows.length * report.repeats).toLocaleString()} measured calls; ${report.sources.length} generated source files.`,
        `${report.measuredAt}; ${report.platform}; ${String(report.cpu).trim()}; Node ${report.runtime}. ${report.ffmpeg.trim()}.`, '',
        `Completed: ${ok.length}; smaller outputs: ${ok.filter(r => r.bytes < r.inputBytes).length}; unchanged outputs: ${ok.filter(r => r.unchanged).length}; errors: ${rows.length - ok.length}.`,
        `Size-contract violations: ${ok.filter(r => !r.sizeContract).length}; unchanged-property mismatches: ${ok.filter(r => r.reportedUnchanged !== undefined && r.reportedUnchanged !== r.unchanged).length}.`,
        `Compression-call time summed across repetitions: ${number(report.compressionSeconds)} s. Overall loop: ${number(report.seconds)} s (includes preflight, hashing and optional decode verification).`, '',
        '## Reading these numbers', '',
        '- Completed means the API returned bytes. The output-size ceiling may return the original, so it does not guarantee a requested conversion or transform happened.',
        '- Smaller and unchanged are independent observations; changed output can have equal byte length. Savings are weighted by input bytes across successful cases, including unchanged results. Repeated configurations count the same input repeatedly.',
        '- Latency columns summarize successful case medians only. P95 is the 95th percentile across those case medians, not across individual requests. Three sequential repetitions are insufficient for reliable tail-latency claims.',
        '- Output details describe the last repetition. Raw JSON retains every repetition’s time, status, size and format. Decode verification, when enabled, checks each distinct returned byte sequence once using native FFmpeg; SVG is excluded.', '',
        '## By input family', '', ...breakdown(rows, r => r.category),
        '## By operation', '', ...breakdown(rows, r => r.name.split('/')[0]),
        '## Requested output format', '',
        ...table(['Target', 'Requests', 'Completed', 'Target delivered', 'Original returned', 'Errors', 'Savings when target delivered %', 'Median ms when delivered'],
            grouped(conversions, r => r.config.output_format).map(([format, cases]) => {
                const done = successful(cases), delivered = done.filter(r => r.requestedFormatApplied);
                return [format, cases.length, done.length, delivered.length, done.filter(r => r.unchanged).length,
                    cases.length - done.length, number(saving(delivered)), number(percentile(delivered.map(r => r.medianMs), .5))];
            })),
        'Target delivered means the detected output format matches the requested format. It does not verify dimensions, trim duration, metadata or perceptual quality.', '',
        '## Default compression, one row per input', '',
        ...table(['Input', 'Input bytes', 'Output bytes', 'Actual format', 'Saved %', 'Median ms', 'Unchanged'],
            rows.filter(r => r.name === 'defaults').map(r => [r.source, r.inputBytes, r.bytes ?? r.code, r.stats?.format_out,
                r.status === 'ok' ? number(saving([r])) : '—', number(r.medianMs), r.unchanged ?? '—'])),
        '## Quality sweep (plain conversion requests)', '',
        ...breakdown(rows.filter(r => r.name.startsWith('convert/')), r => `${r.category} / quality ${r.config.quality}`),
        'Quality values are encoder controls, not comparable perceptual scores. Lossless encoders may ignore quality; size fallback can also hide an encoder change.', '',
        '## Slowest completed configurations', '',
        ...table(['Input', 'Operation', 'Median ms', 'Min–max ms', 'Input bytes', 'Output bytes', 'Actual format'],
            [...ok].sort((a, b) => b.medianMs - a.medianMs).slice(0, 12).map(r => [r.source, r.name, number(r.medianMs),
                `${number(r.minMs)}–${number(r.maxMs)}`, r.inputBytes, r.bytes, r.stats.format_out])),
        '## Failure breakdown', '',
        ...table(['Reason', 'Count', 'Preflight missed', 'Example', 'Diagnostic'],
            grouped(rows.filter(r => r.status === 'error'), failureKind).map(([code, cases]) =>
                [code, cases.length, cases.filter(r => r.preflight.valid).length, `${cases[0].source}: ${cases[0].name}`, cases[0].message.slice(0, 300)])),
        'Error totals include deliberately invalid settings, unsupported animation/SVG operations and explicit stream-copy requests whose codecs cannot be placed in the selected container.', '',
        '## Output decoding and repetition stability', '',
        `Unique outputs decoded: ${report.uniqueDecodedOutputs ?? 0}. Cases with decode rejection: ${ok.filter(r => r.decode?.valid === false).length}. Cases without a decode check: ${ok.filter(r => !r.decode).length}.`,
        `Decode rejections on changed outputs: ${ok.filter(r => r.decode?.valid === false && !r.unchanged).length}; on byte-identical original outputs: ${ok.filter(r => r.decode?.valid === false && r.unchanged).length}. A decoder rejecting unchanged original bytes is not evidence of corruption introduced by compression.`,
        `Cases whose repetitions differed in status, byte count or format: ${rows.filter(r => new Set((r.samples ?? []).map(s => JSON.stringify([s.status, s.bytes, s.format, s.code]))).size > 1).length}. Byte hashes may still vary due to container IDs.`, '',
        ...table(['Input', 'Operation', 'Decode diagnostic'], ok.filter(r => r.decode?.valid === false).map(r => [r.source, r.name, r.decode.message.slice(0, 350)])),
        '## Scope and reproduction', '',
        'Short synthetic Node/native FFmpeg workloads; no browser performance, peak-memory, concurrency, perceptual-quality or exhaustive format claims. End-of-run process memory is recorded in JSON but is not a memory bound. Hardware encoders listed by FFmpeg may require unavailable devices.', '',
        `Run: npm run benchmark -- --repeats=${report.repeats} --label=next${report.extended ? ' --extended' : ''}${report.verifyDecode ? ' --verify-decode' : ''} --reuse-media`,
        'Build all targets first. Set FFMPEG_PATH to the same executable and retain the generated media for comparisons. Omit --reuse-media on the first run.', '',
        'Unavailable generators:', ...report.unavailable.map(r => `- ${escape(r.name)}: ${escape(r.error).slice(0, 350)}`), ''];
    if (previous) {
        const key = r => JSON.stringify([r.source, r.name, r.config]);
        const prior = new Map(previous.observations.map(r => [key(r), r]));
        const pairs = rows.map(r => [r, prior.get(key(r))]).filter(([, old]) => old);
        const same = pairs.filter(([r, old]) => r.inputSha256 && r.inputSha256 === old.inputSha256);
        lines.push('## Comparison with ' + previous.label, '',
            `Matched configurations: ${pairs.length}; matched configurations with identical recorded input hashes: ${same.length}.`,
            `Newly completed: ${same.filter(([r, old]) => r.status === 'ok' && old.status === 'error').length}. Newly rejected: ${same.filter(([r, old]) => r.status === 'error' && old.status === 'ok').length} (includes newly enforced restrictions and unusable-output rejection).`,
            `Changed outputs rejected by independent decoding: ${successful(previous.observations).filter(r => r.decode?.valid === false && !r.unchanged).length} previously; ${ok.filter(r => r.decode?.valid === false && !r.unchanged).length} now.`, '');
    }
    return lines.join('\n') + '\n';
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
    const [input, previous] = process.argv.slice(2);
    if (!input?.endsWith('.json')) throw new Error('Usage: node scripts/benchmark-report.mjs report.json [previous.json]');
    const report = JSON.parse(await readFile(input, 'utf8'));
    const before = previous ? JSON.parse(await readFile(previous, 'utf8')) : undefined;
    await writeFile(input.replace(/\.json$/, '.md'), renderReport(report, before));
}
