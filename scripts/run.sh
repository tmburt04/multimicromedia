#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

INPUT_DIR="${1:-input}"
OUTPUT_DIR="${2:-output}"

# Validate input directory
if [[ ! -d "$INPUT_DIR" ]]; then
    echo "Error: Input directory '$INPUT_DIR' not found."
    echo "Usage: $0 [input_dir] [output_dir]"
    exit 1
fi

# Ensure WASM is built for Node.js
if [[ ! -f "pkg/compression_wasm.js" ]]; then
    echo "==> WASM not built. Building for nodejs..."
    wasm-pack build --target nodejs --release
fi

# Ensure sharp is installed
if [[ ! -d "node_modules/sharp" ]]; then
    echo "==> Installing sharp for image preprocessing..."
    npm install sharp --silent
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo "==> Processing files from '$INPUT_DIR' to '$OUTPUT_DIR'"
echo "    - Center crop to square"
echo "    - Resize to 256x256"
echo "    - Compress & output as PNG (64 colors, max compression)"
echo ""

node - "$INPUT_DIR" "$OUTPUT_DIR" << 'SCRIPT'
import { readFileSync, writeFileSync, readdirSync, statSync } from 'fs';
import { join, basename, extname } from 'path';
import sharp from 'sharp';

// Load and initialize WASM
const wasmBytes = readFileSync('./pkg/compression_wasm_bg.wasm');
const { initSync, compress, init_panic_hook } = await import('./pkg/compression_wasm.js');
initSync({ module: wasmBytes });
init_panic_hook();

const [,, inputDir, outputDir] = process.argv;

// Build MAXIMUM PNG compression - quantized to 64 colors
const config = JSON.stringify({
    output_format: 'png',
    quality: 50,
    png: {
        compression_level: 9,
        quantize: true,
        max_colors: 64
    }
});

const files = readdirSync(inputDir).filter(f => {
    const stat = statSync(join(inputDir, f));
    return stat.isFile() && /\.(jpe?g|png|webp|gif|avif|heic|bmp|tiff?)$/i.test(f);
});

if (files.length === 0) {
    console.log('No image files found in input directory.');
    process.exit(0);
}

console.log(`Processing ${files.length} file(s)...\n`);

let success = 0;
let failed = 0;
let totalOriginal = 0;
let totalCompressed = 0;

for (const file of files) {
    const inputPath = join(inputDir, file);
    const outputName = basename(file, extname(file)) + '.png';
    const outputPath = join(outputDir, outputName);

    try {
        // Step 1: Preprocess with sharp (crop + resize)
        const image = sharp(inputPath);
        const metadata = await image.metadata();
        
        const { width, height } = metadata;
        const size = Math.min(width, height);
        
        // Center crop to square
        const left = Math.floor((width - size) / 2);
        const top = Math.floor((height - size) / 2);
        
        // Aggressive preprocessing: crop, resize, quantize to 64 colors
        const preprocessed = await image
            .extract({ left, top, width: size, height: size })
            .resize(256, 256)
            .png({ 
                palette: true,
                colors: 64,
                effort: 10,
                compressionLevel: 9
            })
            .toBuffer();
        
        // Step 2: Compress with WASM
        const result = await compress(new Uint8Array(preprocessed), config);
        
        writeFileSync(outputPath, result.data);
        
        const prepKB = (preprocessed.length / 1024).toFixed(1);
        const compKB = (result.compressed_size / 1024).toFixed(1);
        const ratio = result.compression_ratio.toFixed(1);
        
        totalOriginal += preprocessed.length;
        totalCompressed += result.compressed_size;
        
        console.log(`✓ ${file} → ${outputName} [${prepKB}KB → ${compKB}KB, ${ratio}%]`);
        success++;
        
        result.free();
    } catch (err) {
        console.log(`✗ ${file}: ${err.message || err}`);
        failed++;
    }
}

const totalOrigKB = (totalOriginal / 1024).toFixed(1);
const totalCompKB = (totalCompressed / 1024).toFixed(1);
const totalRatio = ((totalCompressed / totalOriginal) * 100).toFixed(1);

console.log(`\n==> Done: ${success} succeeded, ${failed} failed`);
console.log(`==> Total: ${totalOrigKB}KB → ${totalCompKB}KB (${totalRatio}% of original)`);
SCRIPT

echo ""
echo "==> Output written to '$OUTPUT_DIR/'"


