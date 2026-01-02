/**
 * FFmpeg Node.js Bridge
 * Uses native FFmpeg binary for Node.js environments.
 */

import { spawn } from 'child_process';
import { writeFileSync, readFileSync, unlinkSync, mkdtempSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

let ffmpegPath = null;
let isLoaded = false;

async function initialize() {
    try {
        const ffmpegStatic = await import('ffmpeg-static');
        ffmpegPath = ffmpegStatic.default;
        isLoaded = true;
        return true;
    } catch {
        ffmpegPath = 'ffmpeg';
        try {
            await execPromise(ffmpegPath, ['-version']);
            isLoaded = true;
            return true;
        } catch {
            isLoaded = false;
            return false;
        }
    }
}

function execPromise(cmd, args, options = {}) {
    return new Promise((resolve, reject) => {
        const proc = spawn(cmd, args, { ...options, stdio: ['pipe', 'pipe', 'pipe'] });
        let stdout = '';
        let stderr = '';
        
        proc.stdout?.on('data', (data) => { stdout += data; });
        proc.stderr?.on('data', (data) => { stderr += data; });
        
        proc.on('error', reject);
        proc.on('close', (code) => {
            if (code === 0) {
                resolve({ stdout, stderr });
            } else {
                reject(new Error(`FFmpeg exited with code ${code}: ${stderr}`));
            }
        });
    });
}

function isAvailable() {
    return isLoaded && ffmpegPath !== null;
}

const FORMAT_MAP = {
    'mkv': 'matroska',
    'mka': 'matroska',
    'aac': 'adts',
    'wmv': 'asf',
    'wma': 'asf',
    'mpg': 'mpeg',
    'ac3': 'ac3',
    'aiff': 'aiff',
    'amr': 'amr',
    'gif': 'gif',
    'webp': 'webp',
};

const EXT_MAP = {
    'matroska': 'mkv',
    'adts': 'aac',
    'asf': 'wmv',
    'mpeg': 'mpg',
    'ipod': 'm4a',
    'gif': 'gif',
    'webp': 'webp',
    'mjpeg': 'jpg',
};

async function execute(args, inputData) {
    if (!isAvailable()) {
        return { error: 'FFmpeg not available' };
    }

    const tempDir = mkdtempSync(join(tmpdir(), 'ffmpeg-'));
    
    try {
        let inputFormat = null;
        let outputFormat = 'mp4';
        const filteredArgs = [];
        let fCount = 0;
        
        for (let i = 0; i < args.length; i++) {
            if (args[i] === '-f' && i + 1 < args.length) {
                const fmt = args[i + 1];
                fCount++;
                if (fCount === 1) {
                    inputFormat = fmt;
                } else {
                    outputFormat = FORMAT_MAP[fmt] || fmt;
                    filteredArgs.push('-f', outputFormat);
                }
                i++;
            } else {
                filteredArgs.push(args[i]);
            }
        }
        
        const inputExt = inputFormat || 'dat';
        const outputExt = EXT_MAP[outputFormat] || outputFormat;
        
        const inputFile = join(tempDir, `input.${inputExt}`);
        const outputFile = join(tempDir, `output.${outputExt}`);
        
        writeFileSync(inputFile, inputData);
        
        const fullArgs = [
            '-y',
            '-i', inputFile,
            ...filteredArgs,
            outputFile
        ];
        
        await execPromise(ffmpegPath, fullArgs);
        
        const outputData = readFileSync(outputFile);
        
        try { unlinkSync(inputFile); } catch {}
        try { unlinkSync(outputFile); } catch {}
        try { unlinkSync(tempDir); } catch {}
        
        if (outputData.length >= inputData.length) {
            return { data: inputData };
        }
        
        return { data: new Uint8Array(outputData) };
    } catch (error) {
        return { error: error.message || 'FFmpeg execution failed' };
    }
}

async function getVersion() {
    if (!isAvailable()) {
        return 'FFmpeg not available';
    }
    try {
        const { stdout } = await execPromise(ffmpegPath, ['-version']);
        return stdout.split('\n')[0];
    } catch {
        return 'Version check failed';
    }
}

const bridge = {
    initialize,
    isAvailable,
    execute,
    getVersion
};

globalThis.__ffmpeg__ = bridge;
if (typeof global !== 'undefined') {
    global.__ffmpeg__ = bridge;
}

export { initialize, isAvailable, execute, getVersion };
