import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const [requested = 'web', profile = 'release', ...extra] = process.argv.slice(2);
const target = requested === 'node' ? 'nodejs' : requested;
if (!['web', 'nodejs', 'bundler', 'all'].includes(target) || !['release', 'dev'].includes(profile)) {
    console.error('Usage: node scripts/build.mjs [web|node|bundler|all] [release|dev] [wasm-pack options]');
    process.exit(requested === '--help' ? 0 : 1);
}
for (const selected of target === 'all' ? ['web', 'nodejs', 'bundler'] : [target]) {
    const result = spawnSync('wasm-pack', ['build', '--target', selected, `--${profile}`,
        '--out-name', selected === 'nodejs' ? 'mmm-node' : 'mmm-js',
        '--out-dir', target === 'all' ? `pkg/${selected}` : 'pkg', ...extra],
    { cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: 'inherit', windowsHide: true });
    if (result.error) console.error(`Unable to run wasm-pack: ${result.error.message}`);
    if (result.status !== 0) process.exit(result.status ?? 1);
}
