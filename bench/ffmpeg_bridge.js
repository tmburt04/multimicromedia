/**
 * FFmpeg WASM Bridge
 * Connects compression library to ffmpeg.wasm for A/V processing.
 */

(function() {
    'use strict';

    let ffmpegInstance = null;
    let isLoaded = false;

    async function initialize(ffmpeg) {
        ffmpegInstance = ffmpeg;
        isLoaded = true;
        console.log('[FFmpeg Bridge] Initialized');
    }

    function isAvailable() {
        return isLoaded && ffmpegInstance !== null;
    }

    async function execute(args, inputData) {
        if (!isAvailable()) {
            return { error: 'FFmpeg not available' };
        }

        try {
            const inputName = 'input';
            const outputName = 'output';
            
            let outputExt = 'mp4';
            const fIndex = args.indexOf('-f');
            if (fIndex !== -1 && fIndex + 1 < args.length) {
                outputExt = args[fIndex + 1];
            }
            
            const inputFile = `${inputName}.bin`;
            const outputFile = `${outputName}.${outputExt}`;
            
            await ffmpegInstance.writeFile(inputFile, inputData);
            
            const fullArgs = ['-i', inputFile, ...args, '-y', outputFile];
            
            await ffmpegInstance.exec(fullArgs);
            
            const outputData = await ffmpegInstance.readFile(outputFile);
            
            await ffmpegInstance.deleteFile(inputFile);
            await ffmpegInstance.deleteFile(outputFile);
            
            return { data: outputData };
        } catch (error) {
            console.error('[FFmpeg Bridge] Error:', error);
            return { error: error.message || 'FFmpeg execution failed' };
        }
    }

    async function executeMultiInput(inputs, args) {
        if (!isAvailable()) {
            return { error: 'FFmpeg not available' };
        }

        try {
            for (const input of inputs) {
                await ffmpegInstance.writeFile(input.name, input.data);
            }
            
            let outputExt = 'mp4';
            const fIndex = args.indexOf('-f');
            if (fIndex !== -1 && fIndex + 1 < args.length) {
                outputExt = args[fIndex + 1];
            }
            const outputFile = `output.${outputExt}`;
            
            const fullArgs = [...args, '-y', outputFile];
            
            await ffmpegInstance.exec(fullArgs);
            
            const outputData = await ffmpegInstance.readFile(outputFile);
            
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
        } catch {
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

    if (typeof module !== 'undefined' && module.exports) {
        module.exports = bridge;
    }
})();
