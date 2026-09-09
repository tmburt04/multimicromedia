import { readdir, readFile, writeFile, mkdir, copyFile, stat } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';

const root = fileURLToPath(new URL('../', import.meta.url));
const manifest = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
async function newestSource(directory) {
    let newest = 0;
    for (const item of await readdir(directory, { withFileTypes: true })) {
        const path = join(directory, item.name);
        newest = Math.max(newest, item.isDirectory() ? await newestSource(path) : (await stat(path)).mtimeMs);
    }
    return newest;
}
const sourceTime = Math.max(await newestSource(join(root, 'src')), ...await Promise.all(
    ['Cargo.toml', 'Cargo.lock'].map(async file => (await stat(join(root, file))).mtimeMs)));
for (const [source, target, name] of [['web', 'web', 'mmm-js'], ['nodejs', 'node', 'mmm-node'], ['bundler', 'bundler', 'mmm-js']]) {
    const sourceDir = join(root, 'pkg', source), targetDir = join(root, 'dist', target);
    const built = JSON.parse(await readFile(join(sourceDir, 'package.json'), 'utf8'));
    if (built.version !== manifest.version || (await stat(join(sourceDir, `${name}_bg.wasm`))).mtimeMs < sourceTime) {
        throw new Error(`Stale ${source} build. Run npm run build:all before packing.`);
    }
    await mkdir(targetDir, { recursive: true });
    for (const file of await readdir(sourceDir)) {
        if (file === 'package.json' || /\.(js|wasm|d\.ts)$/.test(file)) await copyFile(join(sourceDir, file), join(targetDir, file));
    }
}
const wasm = createRequire(import.meta.url)('../dist/node/mmm-node.js');
if (wasm.get_version() !== manifest.version) throw new Error('WASM runtime version does not match package.json');
for (const file of ['ffmpeg_args.js', 'ffmpeg_node.mjs', 'ffmpeg_bridge.js', 'ffmpeg_node.d.mts', 'ffmpeg_bridge.d.ts', 'ffmpeg_types.d.ts']) {
    await copyFile(join(root, 'bench', file), join(root, 'dist', file));
}
const result = spawnSync('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'wasm32-unknown-unknown'], { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, windowsHide: true });
if (result.status !== 0) throw new Error(result.stderr || 'Could not read dependency metadata');
const metadata = JSON.parse(result.stdout);
const notices = ['# Third-party notices', '', 'The WASM artifacts include the following Rust dependencies. Build-time dependencies are also listed. FFmpeg is supplied separately and is not bundled.', ''];
for (const pkg of metadata.packages.filter(pkg => pkg.source).sort((a, b) => a.name.localeCompare(b.name))) {
    if (!pkg.license || /(?:AGPL|LGPL|GPL)-/.test(pkg.license)) throw new Error(`Review dependency license before publication: ${pkg.name}: ${pkg.license}`);
    const dir = dirname(pkg.manifest_path);
    const files = (await readdir(dir)).filter(file => /^(LICENSE|LICENCE|COPYING|UNLICENSE|NOTICE)(?:[._-].*)?$/i.test(file));
    const texts = [];
    for (const file of files) if ((await stat(join(dir, file))).isFile()) texts.push(`### ${file}\n\n${await readFile(join(dir, file), 'utf8')}`);
    if (!texts.length && pkg.name === 'color_quant') {
        const source = await readFile(join(dir, 'src/lib.rs'), 'utf8');
        texts.push(source.slice(source.indexOf('/*') + 2, source.indexOf('*/')));
    }
    if (!texts.length) throw new Error(`No dependency license text found for ${pkg.name}; add its notice before publishing.`);
    notices.push(`## ${pkg.name} ${pkg.version}`, '', `License: ${pkg.license}`, '', ...texts, '');
}
await writeFile(join(root, 'dist', 'THIRD_PARTY_NOTICES.md'), notices.join('\n'));
console.log(`Prepared ${manifest.name}@${manifest.version}: web, Node, bundler, optional bridges and dependency notices.`);
