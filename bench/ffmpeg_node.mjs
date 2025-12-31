/**
 * FFmpeg Node.js Bridge
 * Uses native FFmpeg binary for Node.js environments
 */

import { spawn } from 'child_process';
import { writeFileSync, readFileSync, unlinkSync, mkdtempSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

let ffmpegPath = null;
let isLoaded = false;

/**
 * Initialize the FFmpeg bridge
 */
async function initialize() {
    try {
        // Try to use ffmpeg-static
        const ffmpegStatic = await import('ffmpeg-static');
        ffmpegPath = ffmpegStatic.default;
        isLoaded = true;
        return true;
    } catch {
        // Try system ffmpeg
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

/**
 * Check if FFmpeg is available
 */
function isAvailable() {
    return isLoaded && ffmpegPath !== null;
}

// Map short format names to FFmpeg format names
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

// Map format names to file extensions for output
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

/**
 * Execute FFmpeg with the given arguments
 * @param {string[]} args - FFmpeg arguments (without -i input)
 * @param {Uint8Array} inputData - Input file data
 * @returns {Promise<{data: Uint8Array, error?: string}>}
 */
async function execute(args, inputData) {
    if (!isAvailable()) {
        return { error: 'FFmpeg not available' };
    }

    const tempDir = mkdtempSync(join(tmpdir(), 'ffmpeg-'));
    
    try {
        // Parse args: first -f is input format hint, second -f is output format
        let inputFormat = null;
        let outputFormat = 'mp4';
        const filteredArgs = [];
        let fCount = 0;
        
        for (let i = 0; i < args.length; i++) {
            if (args[i] === '-f' && i + 1 < args.length) {
                const fmt = args[i + 1];
                fCount++;
                if (fCount === 1) {
                    // First -f: input format hint (use for extension)
                    inputFormat = fmt;
                } else {
                    // Second -f: output format
                    outputFormat = FORMAT_MAP[fmt] || fmt;
                    filteredArgs.push('-f', outputFormat);
                }
                i++; // Skip format value
            } else {
                filteredArgs.push(args[i]);
            }
        }
        
        // Use data extension for input (let FFmpeg probe the format)
        // This is more reliable than forcing extension
        const inputExt = inputFormat || 'dat';
        const outputExt = EXT_MAP[outputFormat] || outputFormat;
        
        const inputFile = join(tempDir, `input.${inputExt}`);
        const outputFile = join(tempDir, `output.${outputExt}`);
        
        // Write input file
        writeFileSync(inputFile, inputData);
        
        // Build full command - let FFmpeg auto-detect input format
        const fullArgs = [
            '-y',                    // Overwrite output
            '-i', inputFile,         // Input file
            ...filteredArgs,         // User args with fixed formats
            outputFile               // Output file
        ];
        
        // Execute FFmpeg
        await execPromise(ffmpegPath, fullArgs);
        
        // Read output
        const outputData = readFileSync(outputFile);
        
        // Cleanup
        try { unlinkSync(inputFile); } catch {}
        try { unlinkSync(outputFile); } catch {}
        try { unlinkSync(tempDir); } catch {}
        
        // Return original if output is larger (no benefit)
        if (outputData.length >= inputData.length) {
            return { data: inputData };
        }
        
        return { data: new Uint8Array(outputData) };
    } catch (error) {
        return { error: error.message || 'FFmpeg execution failed' };
    }
}

/**
 * Get FFmpeg version
 */
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

// Register globally for Rust WASM access
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


