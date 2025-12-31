/**
 * Compression Testbench - Main Application
 */

// Paths (symlinks created by serve.sh)
const WASM_PATH = './pkg/compression_wasm.js';
const TESTDATA_PATH = './testdata';

// Testdata manifest - hardcoded file list for reliable loading
const TESTDATA_MANIFEST = {
    images: [
        'animated.gif', 'f0304009970c4085e68d38093bbf600d62063b3e.png',
        'IMAGE_1215.HEIC', 'IMAGE_avif.avif', 'IMAGE_gif.gif', 'IMAGE_jpg.jpg',
        'IMAGE_png.png', 'IMAGE_svg.svg', 'IMAGE_TIFF_5MB.tiff', 'IMAGE_webp.webp',
        'image.svg', 'progressive.jpg', 'sample.ai', 'sample.gif', 'sample.jpg',
        'sample.png', 'sample.tiff', 'sample.webp', 'svg-embedded.svg'
    ],
    audio: [
        'AUDIO_MP3_2MG.mp3', 'AUDIO_OOG_2MG.ogg', 'AUDIO_WAV_2MG.wav',
        'sample.aac', 'sample.ac3', 'sample.aiff', 'sample.amr', 'sample.flac',
        'sample.mka', 'sample.mp3', 'sample.ogg', 'sample.wav', 'sample.wma'
    ],
    video: [
        'sample.avi', 'sample.flv', 'sample.mkv', 'sample.mov', 'sample.mp4',
        'sample.mpg', 'sample.webm', 'sample.wmv', 'VIDEO_1215.mov',
        'VIDEO_AVI_1920_2_3MG.avi', 'VIDEO_MOV_1920_2_2MB.mov',
        'VIDEO_MP4_1920_18MG.mp4', 'VIDEO_WEBM_1280_3_6MB.webm',
        'VIDEO_WMV_1280_4_9MB.wmv'
    ]
};

let wasm = null;
let currentFile = null;
let currentFileData = null;
let compressedData = null;
let compressedMime = null;
let testdataFiles = [];
let batchResults = [];
let batchRunning = false;
let batchAbort = false;

// ============================================================================
// WASM Initialization
// ============================================================================

async function initWasm() {
    try {
        const module = await import(WASM_PATH);
        if (typeof module.default === 'function') {
            await module.default();
        }
        wasm = module;
        wasm.init_panic_hook();

        document.getElementById('wasm-status').classList.remove('loading');
        document.getElementById('wasm-status').classList.add('ready');
        document.getElementById('wasm-status-text').textContent = 'WASM Ready';
        document.getElementById('version').textContent = 'v' + wasm.get_version();

        loadTestdata();
        updateOutputFormats();

        // Load FFmpeg asynchronously (don't block UI)
        initFFmpeg();

        return true;
    } catch (err) {
        console.error('WASM init failed:', err);
        document.getElementById('wasm-status').classList.remove('loading');
        document.getElementById('wasm-status-text').textContent = 'WASM Error: ' + err.message;
        
        // Still load testdata tree even if WASM fails
        loadTestdata();
        return false;
    }
}

async function initFFmpeg() {
    const statusDot = document.getElementById('ffmpeg-status');
    const statusText = document.getElementById('ffmpeg-status-text');
    
    statusDot.classList.add('loading');
    statusText.textContent = 'FFmpeg: Loading...';
    
    try {
        if (typeof window.FFmpegLoader !== 'undefined') {
            const loaded = await window.FFmpegLoader.load();
            
            if (loaded && window.__ffmpeg__.isAvailable()) {
                statusDot.classList.remove('loading');
                statusDot.classList.add('ready');
                statusText.textContent = 'FFmpeg: Ready';
            } else {
                statusDot.classList.remove('loading');
                statusText.textContent = 'FFmpeg: Failed to load';
            }
        } else {
            statusDot.classList.remove('loading');
            statusText.textContent = 'FFmpeg: Loader not available';
        }
    } catch (err) {
        console.warn('FFmpeg init error:', err);
        statusDot.classList.remove('loading');
        statusText.textContent = 'FFmpeg: ' + (err.message || 'Error');
    }
}

// ============================================================================
// Testdata Loading
// ============================================================================

function loadTestdata() {
    const container = document.getElementById('testdata-content');
    container.innerHTML = '';

    testdataFiles = [];

    for (const [category, files] of Object.entries(TESTDATA_MANIFEST)) {
        if (files.length === 0) continue;

        const categoryEl = document.createElement('div');
        categoryEl.className = 'tree-category';
        categoryEl.innerHTML = `
            <div class="tree-category-header" onclick="toggleTreeCategory(this.parentElement)">
                <span>▶</span>
                <span>${category.charAt(0).toUpperCase() + category.slice(1)} (${files.length})</span>
            </div>
            <div class="tree-files"></div>
        `;

        const filesEl = categoryEl.querySelector('.tree-files');

        for (const fileName of files) {
            const fileEl = document.createElement('div');
            fileEl.className = 'tree-file';
            fileEl.dataset.path = `${TESTDATA_PATH}/${category}/${fileName}`;
            fileEl.dataset.category = category;
            fileEl.innerHTML = `
                <span>${fileName}</span>
                <span class="tree-file-size"></span>
            `;
            fileEl.onclick = () => loadTestdataFile(fileEl.dataset.path, fileName);
            filesEl.appendChild(fileEl);

            testdataFiles.push({
                path: `${TESTDATA_PATH}/${category}/${fileName}`,
                name: fileName,
                category: category,
                element: fileEl
            });
        }

        categoryEl.classList.add('collapsed');
        container.appendChild(categoryEl);
    }

    if (testdataFiles.length > 0) {
        document.getElementById('run-all-btn').disabled = !wasm;
        updateDashboardTable();
    } else {
        container.innerHTML = '<div class="empty-state">No testdata files configured.</div>';
    }
}

async function loadTestdataFile(path, name) {
    try {
        showLoading('Loading file...');
        const response = await fetch(path);
        if (!response.ok) throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);

        const arrayBuffer = await response.arrayBuffer();
        const data = new Uint8Array(arrayBuffer);

        document.querySelectorAll('.tree-file').forEach(el => el.classList.remove('selected'));
        const selectedEl = document.querySelector(`.tree-file[data-path="${path}"]`);
        if (selectedEl) {
            selectedEl.classList.add('selected');
            // Update size in tree
            const sizeEl = selectedEl.querySelector('.tree-file-size');
            if (sizeEl) sizeEl.textContent = formatBytes(data.length);
        }

        await handleFileData(data, name);
    } catch (err) {
        console.error('Load error:', err);
        alert('Failed to load file: ' + err.message);
    } finally {
        hideLoading();
    }
}

// ============================================================================
// File Handling
// ============================================================================

async function handleFileData(data, name) {
    currentFile = { name, data };
    currentFileData = data;
    compressedData = null;

    document.getElementById('file-info').classList.remove('hidden');
    document.getElementById('file-name').textContent = name;
    document.getElementById('file-size').textContent = formatBytes(data.length);

    if (wasm) {
        try {
            const analysis = wasm.analyze_file(data);
            const mime = wasm.detect_file_mime(data);
            const type = wasm.detect_file_type(data);

            document.getElementById('file-type').textContent = `${type.toUpperCase()} (${mime})`;

            if (analysis.width && analysis.height) {
                document.getElementById('file-dimensions').textContent = `${analysis.width} × ${analysis.height}`;
            } else {
                document.getElementById('file-dimensions').textContent = '-';
            }

            updateFormForType(analysis);
            updateOutputFormats(analysis);
            showOriginalPreview(data, mime);
        } catch (err) {
            console.error('Analysis error:', err);
            document.getElementById('file-type').textContent = 'Unknown';
            document.getElementById('file-dimensions').textContent = '-';
        }

        document.getElementById('compress-btn').disabled = false;
    } else {
        document.getElementById('file-type').textContent = 'WASM not loaded';
        document.getElementById('file-dimensions').textContent = '-';
        document.getElementById('compress-btn').disabled = true;
    }
}

function updateFormForType(analysis) {
    const format = analysis.format?.toLowerCase() || '';
    const isImage = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tiff', 'svg', 'avif', 'heic'].includes(format);
    const isAudioVideo = ['mp3', 'wav', 'flac', 'ogg', 'aac', 'mp4', 'webm', 'mov', 'avi', 'mkv'].includes(format);

    document.getElementById('image-options').style.display = isImage ? 'block' : 'none';
    document.getElementById('audio-video-options').style.display = isAudioVideo ? 'block' : 'none';
}

function updateOutputFormats(analysis) {
    const select = document.getElementById('output-format');
    select.innerHTML = '<option value="">Auto (same as input)</option>';

    if (!wasm) return;

    const formats = wasm.get_supported_formats();
    let relevantFormats = [];

    const format = analysis?.format?.toLowerCase() || '';
    const isImage = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tiff', 'svg', 'avif', 'heic', 'ico'].includes(format);
    const isAudio = ['mp3', 'wav', 'flac', 'ogg', 'aac', 'opus'].includes(format);
    const isVideo = ['mp4', 'webm', 'mov', 'avi', 'mkv', 'wmv', 'flv'].includes(format);

    if (isImage || !analysis) relevantFormats = relevantFormats.concat(formats.images || []);
    if (isAudio || !analysis) relevantFormats = relevantFormats.concat(formats.audio || []);
    if (isVideo || !analysis) relevantFormats = relevantFormats.concat(formats.video || []);

    for (const fmt of relevantFormats) {
        const option = document.createElement('option');
        option.value = fmt.extension;
        option.textContent = `${fmt.extension.toUpperCase()} (${fmt.mime})`;
        select.appendChild(option);
    }
}

// ============================================================================
// Preview
// ============================================================================

function showOriginalPreview(data, mime) {
    const container = document.getElementById('original-preview');
    container.innerHTML = '';

    if (mime && mime.startsWith('image/')) {
        const blob = new Blob([data], { type: mime });
        const url = URL.createObjectURL(blob);
        const img = document.createElement('img');
        img.src = url;
        img.onload = () => URL.revokeObjectURL(url);
        img.onerror = () => {
            container.innerHTML = '<span class="placeholder">Failed to load preview</span>';
        };
        container.appendChild(img);
    } else {
        container.innerHTML = '<span class="placeholder">Preview not available for this format</span>';
    }
}

function showCompressedPreview(data, mime) {
    const container = document.getElementById('compressed-preview');
    container.innerHTML = '';

    if (mime && mime.startsWith('image/')) {
        const blob = new Blob([data], { type: mime });
        const url = URL.createObjectURL(blob);
        const img = document.createElement('img');
        img.src = url;
        img.onload = () => URL.revokeObjectURL(url);
        img.onerror = () => {
            container.innerHTML = '<span class="placeholder">Failed to load preview</span>';
        };
        container.appendChild(img);
    } else {
        container.innerHTML = '<span class="placeholder">Preview not available for this format</span>';
    }
}

// ============================================================================
// Compression
// ============================================================================

async function compress() {
    if (!wasm || !currentFileData) return;

    showLoading('Compressing...');

    try {
        const config = buildConfig();
        const result = await wasm.compress(currentFileData, JSON.stringify(config));

        compressedData = result.data;
        compressedMime = getMimeForFormat(result.format_out);

        showCompressedPreview(compressedData, compressedMime);
        updateStats(result);

        document.getElementById('results-empty').classList.add('hidden');
        document.getElementById('results-content').classList.remove('hidden');
        document.getElementById('download-btn').disabled = false;
    } catch (err) {
        console.error('Compression error:', err);
        alert('Compression failed: ' + err.message);
    } finally {
        hideLoading();
    }
}

function buildConfig() {
    const config = {
        quality: parseInt(document.getElementById('quality').value)
    };

    const outputFormat = document.getElementById('output-format').value;
    if (outputFormat) config.output_format = outputFormat;

    const resizeWidth = document.getElementById('resize-width').value;
    const resizeHeight = document.getElementById('resize-height').value;
    if (resizeWidth || resizeHeight) {
        config.resize = {
            width: resizeWidth ? parseInt(resizeWidth) : null,
            height: resizeHeight ? parseInt(resizeHeight) : null,
            mode: document.getElementById('resize-mode').value,
            preserve_aspect: true
        };
    }

    const trimStart = document.getElementById('trim-start').value;
    const trimEnd = document.getElementById('trim-end').value;
    if (trimStart || trimEnd) {
        config.trim = {
            start_ms: trimStart ? parseInt(trimStart) : null,
            end_ms: trimEnd ? parseInt(trimEnd) : null
        };
    }

    const audioBitrate = document.getElementById('audio-bitrate').value;
    if (audioBitrate) {
        config.ffmpeg = { audio_bitrate: audioBitrate };
    }

    config.preserve_metadata = document.getElementById('preserve-metadata').checked;

    return config;
}

function updateStats(result) {
    const originalSize = result.original_size;
    const compressedSize = result.compressed_size;
    const ratio = result.compression_ratio;
    const time = result.time_ms;

    document.getElementById('original-size-badge').textContent = formatBytes(originalSize);
    document.getElementById('compressed-size-badge').textContent = formatBytes(compressedSize);

    document.getElementById('stats-original').textContent = formatBytes(originalSize);
    document.getElementById('stats-compressed').textContent = formatBytes(compressedSize);
    document.getElementById('stats-time').textContent = time.toFixed(0) + 'ms';

    const savingsEl = document.getElementById('stats-savings');
    const savingsPercent = (ratio * 100).toFixed(1);
    if (ratio > 0) {
        savingsEl.textContent = savingsPercent + '%';
        savingsEl.className = 'stats-card-value positive';
    } else {
        savingsEl.textContent = '+' + Math.abs(savingsPercent) + '%';
        savingsEl.className = 'stats-card-value negative';
    }
}

// ============================================================================
// Batch Processing
// ============================================================================

async function runBatch() {
    if (!wasm || testdataFiles.length === 0) return;

    batchRunning = true;
    batchAbort = false;
    batchResults = [];

    document.getElementById('run-all-btn').disabled = true;
    document.getElementById('stop-btn').disabled = false;
    document.getElementById('batch-progress').classList.remove('hidden');
    document.getElementById('batch-summary').classList.add('hidden');

    const config = buildConfig();
    let processed = 0;

    for (const file of testdataFiles) {
        if (batchAbort) break;

        updateBatchProgress(processed, testdataFiles.length, `Processing ${file.name}...`);
        updateFileRowStatus(file, 'processing');

        try {
            const response = await fetch(file.path);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            
            const arrayBuffer = await response.arrayBuffer();
            const data = new Uint8Array(arrayBuffer);

            const startTime = performance.now();
            const result = await wasm.compress(data, JSON.stringify(config));
            const endTime = performance.now();

            batchResults.push({
                file: file.name,
                category: file.category,
                originalSize: result.original_size,
                compressedSize: result.compressed_size,
                ratio: result.compression_ratio,
                time: Math.round(endTime - startTime),
                formatIn: result.format_in,
                formatOut: result.format_out,
                error: null
            });

            updateFileRowResult(file, batchResults[batchResults.length - 1]);
        } catch (err) {
            batchResults.push({
                file: file.name,
                category: file.category,
                originalSize: 0,
                compressedSize: 0,
                ratio: 0,
                time: 0,
                error: err.message || String(err)
            });
            updateFileRowError(file, err.message || String(err));
        }

        processed++;
    }

    batchRunning = false;
    document.getElementById('run-all-btn').disabled = false;
    document.getElementById('stop-btn').disabled = true;
    document.getElementById('batch-progress').classList.add('hidden');

    updateBatchSummary();
}

function stopBatch() {
    batchAbort = true;
}

function updateBatchProgress(current, total, text) {
    document.getElementById('batch-progress-text').textContent = text;
    document.getElementById('batch-progress-count').textContent = `${current}/${total}`;
    document.getElementById('batch-progress-bar').style.width = `${(current / total) * 100}%`;
}

function updateDashboardTable() {
    const tbody = document.getElementById('dashboard-tbody');
    tbody.innerHTML = '';

    for (const file of testdataFiles) {
        const tr = document.createElement('tr');
        tr.id = `row-${file.path.replace(/[^a-z0-9]/gi, '-')}`;
        tr.innerHTML = `
            <td class="status-cell">-</td>
            <td>${file.name}</td>
            <td><span class="badge badge-${file.category}">${file.category}</span></td>
            <td>-</td>
            <td>-</td>
            <td>-</td>
            <td>-</td>
        `;
        tbody.appendChild(tr);
    }
}

function updateFileRowStatus(file, status) {
    const rowId = `row-${file.path.replace(/[^a-z0-9]/gi, '-')}`;
    const row = document.getElementById(rowId);
    if (!row) return;

    const statusCell = row.querySelector('.status-cell');
    if (status === 'processing') {
        statusCell.innerHTML = '<div class="spinner"></div>';
    }
}

function updateFileRowResult(file, result) {
    const rowId = `row-${file.path.replace(/[^a-z0-9]/gi, '-')}`;
    const row = document.getElementById(rowId);
    if (!row) return;

    const cells = row.querySelectorAll('td');
    cells[0].textContent = '✓';
    cells[0].style.color = 'var(--success)';
    cells[3].textContent = formatBytes(result.originalSize);
    cells[4].textContent = formatBytes(result.compressedSize);

    const savingsPercent = (result.ratio * 100).toFixed(1);
    cells[5].textContent = result.ratio > 0 ? savingsPercent + '%' : '+' + Math.abs(savingsPercent) + '%';
    cells[5].className = result.ratio > 0 ? 'ratio-positive' : 'ratio-negative';
    cells[6].textContent = result.time + 'ms';
}

function updateFileRowError(file, error) {
    const rowId = `row-${file.path.replace(/[^a-z0-9]/gi, '-')}`;
    const row = document.getElementById(rowId);
    if (!row) return;

    const cells = row.querySelectorAll('td');
    cells[0].textContent = '✗';
    cells[0].style.color = 'var(--error)';
    cells[5].textContent = error.substring(0, 30) + (error.length > 30 ? '...' : '');
    cells[5].title = error;
}

function updateBatchSummary() {
    const summary = document.getElementById('batch-summary');
    summary.classList.remove('hidden');

    const successful = batchResults.filter(r => !r.error);
    const totalOriginal = successful.reduce((sum, r) => sum + r.originalSize, 0);
    const totalCompressed = successful.reduce((sum, r) => sum + r.compressedSize, 0);
    const totalSavings = totalOriginal > 0 ? ((totalOriginal - totalCompressed) / totalOriginal * 100).toFixed(1) : 0;

    document.getElementById('batch-total-files').textContent = successful.length + '/' + batchResults.length;
    document.getElementById('batch-total-original').textContent = formatBytes(totalOriginal);
    document.getElementById('batch-total-compressed').textContent = formatBytes(totalCompressed);

    const savingsEl = document.getElementById('batch-total-savings');
    savingsEl.textContent = totalSavings + '%';
    savingsEl.className = 'stats-card-value ' + (totalSavings > 0 ? 'positive' : 'negative');
}

function clearBatchResults() {
    batchResults = [];
    updateDashboardTable();
    document.getElementById('batch-summary').classList.add('hidden');
}

// ============================================================================
// Download & Export
// ============================================================================

function downloadCompressed() {
    if (!compressedData || !currentFile) return;

    const ext = compressedMime.split('/')[1] || 'bin';
    const fileName = currentFile.name.replace(/\.[^.]+$/, '') + '_compressed.' + ext;

    const blob = new Blob([compressedData], { type: compressedMime });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

function copyStats() {
    if (!compressedData) return;

    const stats = {
        file: currentFile.name,
        originalSize: document.getElementById('stats-original').textContent,
        compressedSize: document.getElementById('stats-compressed').textContent,
        savings: document.getElementById('stats-savings').textContent,
        time: document.getElementById('stats-time').textContent
    };

    navigator.clipboard.writeText(JSON.stringify(stats, null, 2));
}

// ============================================================================
// Utilities
// ============================================================================

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function getMimeForFormat(format) {
    const mimeMap = {
        png: 'image/png',
        jpg: 'image/jpeg',
        jpeg: 'image/jpeg',
        webp: 'image/webp',
        gif: 'image/gif',
        avif: 'image/avif',
        bmp: 'image/bmp',
        tiff: 'image/tiff',
        svg: 'image/svg+xml',
        mp3: 'audio/mpeg',
        wav: 'audio/wav',
        flac: 'audio/flac',
        ogg: 'audio/ogg',
        aac: 'audio/aac',
        mp4: 'video/mp4',
        webm: 'video/webm',
        mov: 'video/quicktime',
        avi: 'video/x-msvideo',
        mkv: 'video/x-matroska'
    };
    return mimeMap[format] || 'application/octet-stream';
}

function showLoading(text) {
    document.getElementById('loading-text').textContent = text;
    document.getElementById('loading-overlay').classList.remove('hidden');
}

function hideLoading() {
    document.getElementById('loading-overlay').classList.add('hidden');
}

// ============================================================================
// UI Toggles (exposed globally)
// ============================================================================

window.togglePanel = function(panelId) {
    const panel = document.getElementById(panelId);
    panel.classList.toggle('collapsed');
};

window.toggleTreeCategory = function(categoryEl) {
    categoryEl.classList.toggle('collapsed');
    const arrow = categoryEl.querySelector('.tree-category-header span:first-child');
    arrow.textContent = categoryEl.classList.contains('collapsed') ? '▶' : '▼';
};

// ============================================================================
// Event Listeners
// ============================================================================

function initEventListeners() {
    // Quality slider
    document.getElementById('quality').addEventListener('input', (e) => {
        document.getElementById('quality-value').textContent = e.target.value;
    });

    // Buttons
    document.getElementById('compress-btn').addEventListener('click', compress);
    document.getElementById('download-btn').addEventListener('click', downloadCompressed);
    document.getElementById('copy-stats-btn').addEventListener('click', copyStats);
    document.getElementById('run-all-btn').addEventListener('click', runBatch);
    document.getElementById('stop-btn').addEventListener('click', stopBatch);
    document.getElementById('clear-results-btn').addEventListener('click', clearBatchResults);

    // File input
    const dropZone = document.getElementById('drop-zone');
    const fileInput = document.getElementById('file-input');

    dropZone.addEventListener('click', () => fileInput.click());

    dropZone.addEventListener('dragover', (e) => {
        e.preventDefault();
        dropZone.classList.add('dragover');
    });

    dropZone.addEventListener('dragleave', () => {
        dropZone.classList.remove('dragover');
    });

    dropZone.addEventListener('drop', async (e) => {
        e.preventDefault();
        dropZone.classList.remove('dragover');

        const file = e.dataTransfer.files[0];
        if (file) {
            const arrayBuffer = await file.arrayBuffer();
            await handleFileData(new Uint8Array(arrayBuffer), file.name);
        }
    });

    fileInput.addEventListener('change', async (e) => {
        const file = e.target.files[0];
        if (file) {
            const arrayBuffer = await file.arrayBuffer();
            await handleFileData(new Uint8Array(arrayBuffer), file.name);
        }
    });
}

// ============================================================================
// Initialize
// ============================================================================

initEventListeners();
initWasm();


