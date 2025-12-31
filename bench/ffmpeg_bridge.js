/**
 * FFmpeg WASM Bridge
 * 
 * This module provides a bridge between the Rust WASM compression library
 * and ffmpeg.wasm for audio/video processing.
 * 
 * Usage:
 * 1. Load ffmpeg.wasm in your application
 * 2. Initialize this bridge with the FFmpeg instance
 * 3. The Rust WASM code will automatically use this bridge
 */

(function() {
    'use strict';

    let ffmpegInstance = null;
    let isLoaded = false;

    /**
     * Initialize the FFmpeg bridge with an FFmpeg instance
     * @param {Object} ffmpeg - The ffmpeg.wasm instance (already loaded)
     */
    async function initialize(ffmpeg) {
        ffmpegInstance = ffmpeg;
        isLoaded = true;
        console.log('[FFmpeg Bridge] Initialized');
    }

    /**
     * Check if FFmpeg is available
     * @returns {boolean}
     */
    function isAvailable() {
        return isLoaded && ffmpegInstance !== null;
    }

    /**
     * Execute FFmpeg with the given arguments
     * @param {string[]} args - FFmpeg arguments
     * @param {Uint8Array} inputData - Input file data
     * @returns {Promise<{data: Uint8Array, error?: string}>}
     */
    async function execute(args, inputData) {
        if (!isAvailable()) {
            return { error: 'FFmpeg not available' };
        }

        try {
            const inputName = 'input';
            const outputName = 'output';
            
            // Determine output extension from args
            let outputExt = 'mp4';
            const fIndex = args.indexOf('-f');
            if (fIndex !== -1 && fIndex + 1 < args.length) {
                outputExt = args[fIndex + 1];
            }
            
            const inputFile = `${inputName}.bin`;
            const outputFile = `${outputName}.${outputExt}`;
            
            // Write input file
            await ffmpegInstance.writeFile(inputFile, inputData);
            
            // Build full command
            const fullArgs = ['-i', inputFile, ...args, '-y', outputFile];
            
            // Execute FFmpeg
            await ffmpegInstance.exec(fullArgs);
            
            // Read output
            const outputData = await ffmpegInstance.readFile(outputFile);
            
            // Cleanup
            await ffmpegInstance.deleteFile(inputFile);
            await ffmpegInstance.deleteFile(outputFile);
            
            return { data: outputData };
        } catch (error) {
            console.error('[FFmpeg Bridge] Error:', error);
            return { error: error.message || 'FFmpeg execution failed' };
        }
    }

    /**
     * Execute FFmpeg with multiple input files
     * @param {Object[]} inputs - Array of {name: string, data: Uint8Array}
     * @param {string[]} args - FFmpeg arguments
     * @returns {Promise<{data: Uint8Array, error?: string}>}
     */
    async function executeMultiInput(inputs, args) {
        if (!isAvailable()) {
            return { error: 'FFmpeg not available' };
        }

        try {
            // Write all input files
            for (const input of inputs) {
                await ffmpegInstance.writeFile(input.name, input.data);
            }
            
            // Determine output file
            let outputExt = 'mp4';
            const fIndex = args.indexOf('-f');
            if (fIndex !== -1 && fIndex + 1 < args.length) {
                outputExt = args[fIndex + 1];
            }
            const outputFile = `output.${outputExt}`;
            
            // Build full command (assume inputs are already in args via -i)
            const fullArgs = [...args, '-y', outputFile];
            
            // Execute
            await ffmpegInstance.exec(fullArgs);
            
            // Read output
            const outputData = await ffmpegInstance.readFile(outputFile);
            
            // Cleanup
            for (const input of inputs) {
                await ffmpegInstance.deleteFile(input.name);
            }
            await ffmpegInstance.deleteFile(outputFile);
            
            return { data: outputData };
        } catch (error) {
            console.error('[FFmpeg Bridge] Error:', error);
            return { error: error.message || 'FFmpeg execution failed' };
        }
    }

    /**
     * Get FFmpeg version information
     * @returns {Promise<string>}
     */
    async function getVersion() {
        if (!isAvailable()) {
            return 'FFmpeg not available';
        }
        
        try {
            const logs = [];
            ffmpegInstance.on('log', ({ message }) => {
                logs.push(message);
            });
            
            await ffmpegInstance.exec(['-version']);
            
            return logs.join('\n');
        } catch (error) {
            return 'Version check failed';
        }
    }

    const bridge = {
        initialize,
        isAvailable,
        execute,
        executeMultiInput,
        getVersion
    };

    // Export to global scope for Rust WASM access
    // Support all environments: browser, web worker, Node.js
    if (typeof globalThis !== 'undefined') {
        globalThis.__ffmpeg__ = bridge;
    }
    if (typeof window !== 'undefined') {
        window.__ffmpeg__ = bridge;
    }
    if (typeof self !== 'undefined') {
        self.__ffmpeg__ = bridge;
    }
    if (typeof global !== 'undefined') {
        global.__ffmpeg__ = bridge;
    }

    // Also export as module if available
    if (typeof module !== 'undefined' && module.exports) {
        module.exports = bridge;
    }
})();


