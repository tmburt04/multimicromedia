/**
 * Compression Testbench
 */

// Individual browser build, then the all-target browser build.
const WASM_PATHS = ['./pkg/mmm-js.js', './pkg/web/mmm-js.js'];
const TESTDATA_PATH = './testdata';

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
let currentAnalysis = null;
let compressedData = null;
let compressedMime = null;
let compressedExtension = null;
let testdataFiles = [];
let batchResults = [];
let batchRunning = false;
let batchAbort = false;
let compressionRunning = false;
let fileLoadVersion = 0;
let fileLoadController = null;
let batchController = null;
let loadingOperation = 0;
let supportedFormats = null;
const previewUrls = new Map();

// WASM Initialization

async function initWasm() {
    let failure;
    for (const path of WASM_PATHS) {
        try {
            const module = await import(path);
            if (typeof module.default === 'function') await module.default();
            module.init_panic_hook();
            supportedFormats = module.get_supported_formats();
            wasm = module;
            document.getElementById('wasm-status').classList.remove('loading');
            document.getElementById('wasm-status').classList.add('ready');
            document.getElementById('wasm-status-text').textContent = 'WASM Ready';
            document.getElementById('version').textContent = 'v' + wasm.get_version();
            loadTestdata();
            if (currentFileData) handleFileData(currentFileData, currentFile.name);
            else updateOutputFormats();
            initFFmpeg();
            return true;
        } catch (error) { failure = error; }
    }
    wasm = null;
    console.error('WASM init failed:', failure);
    document.getElementById('wasm-status').classList.remove('loading');
    document.getElementById('wasm-status-text').textContent = 'WASM Error: ' + errorMessage(failure);
    loadTestdata();
    return false;
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

// Testdata Loading

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

function beginFileLoad() {
    fileLoadController?.abort();
    fileLoadController = new AbortController();
    fileLoadVersion++;
    currentFile = null;
    currentFileData = null;
    currentAnalysis = null;
    clearCompressionResult();
    clearPreview('original-preview');
    document.getElementById('file-info').classList.add('hidden');
    document.getElementById('compress-btn').disabled = true;
    return fileLoadVersion;
}

async function loadTestdataFile(path, name) {
    const version = beginFileLoad();
    const loading = showLoading('Loading file...');
    try {
        const response = await fetch(path, { signal: fileLoadController.signal });
        if (!response.ok) throw new Error(`Failed to fetch: ${response.status} ${response.statusText}`);
        const data = new Uint8Array(await response.arrayBuffer());
        if (version !== fileLoadVersion) return;
        document.querySelectorAll('.tree-file').forEach(el => el.classList.remove('selected'));
        const selectedEl = testdataFiles.find(file => file.path === path)?.element;
        if (selectedEl) {
            selectedEl.classList.add('selected');
            selectedEl.querySelector('.tree-file-size').textContent = formatBytes(data.length);
        }
        handleFileData(data, name);
    } catch (error) {
        if (version === fileLoadVersion && error?.name !== 'AbortError') {
            console.error('Load error:', error);
            alert('Failed to load file: ' + errorMessage(error));
        }
    } finally { hideLoading(loading); }
}

async function loadLocalFile(file) {
    const version = beginFileLoad();
    const loading = showLoading('Loading file...');
    try {
        const data = new Uint8Array(await file.arrayBuffer());
        if (version === fileLoadVersion) handleFileData(data, file.name);
    } catch (error) {
        if (version === fileLoadVersion) alert('Failed to load file: ' + errorMessage(error));
    } finally { hideLoading(loading); }
}

// File Handling

function handleFileData(data, name) {
    currentFile = { name };
    currentFileData = data;
    currentAnalysis = null;
    clearCompressionResult();
    clearPreview('original-preview');
    document.getElementById('compress-btn').disabled = true;

    document.getElementById('file-info').classList.remove('hidden');
    document.getElementById('file-name').textContent = name;
    document.getElementById('file-size').textContent = formatBytes(data.length);

    if (wasm) {
        try {
            const analysis = wasm.analyze_file(data);
            currentAnalysis = analysis;
            const mime = analysis.mime;
            const type = analysis.format;

            document.getElementById('file-type').textContent = `${type.toUpperCase()} (${mime})`;

            if (analysis.width && analysis.height) {
                document.getElementById('file-dimensions').textContent = `${analysis.width} × ${analysis.height}`;
            } else {
                document.getElementById('file-dimensions').textContent = '-';
            }

            updateFormForType(analysis);
            updateOutputFormats(analysis);
            showOriginalPreview(data, mime);
            document.getElementById('compress-btn').disabled = compressionRunning || batchRunning || type === 'unknown';
        } catch (err) {
            console.error('Analysis error:', err);
            document.getElementById('file-type').textContent = 'Unknown';
            document.getElementById('file-dimensions').textContent = '-';
        }

    } else {
        document.getElementById('file-type').textContent = 'WASM not loaded';
        document.getElementById('file-dimensions').textContent = '-';
        document.getElementById('compress-btn').disabled = true;
    }
}

function updateFormForType(analysis) {
    const mime = analysis.mime || '';
    const transforms = /^(image|video)\//.test(mime) && analysis.format !== 'svg'
        && !(analysis.isAnimated && ['png', 'webp'].includes(analysis.format));
    document.getElementById('image-options').style.display = transforms ? 'block' : 'none';
    document.getElementById('audio-video-options').style.display = /^(audio|video)\//.test(mime) ? 'block' : 'none';
}

function updateOutputFormats(analysis) {
    const select = document.getElementById('output-format');
    select.innerHTML = '<option value="">Auto</option>';

    if (!wasm) return;

    const formats = supportedFormats;
    let relevantFormats = [];

    const mime = analysis?.mime || '';
    if (mime.startsWith('image/') || !analysis) relevantFormats.push(...formats.images);
    if (mime.startsWith('audio/') || mime.startsWith('video/') || !analysis) relevantFormats.push(...formats.audio);
    if (mime.startsWith('video/') || !analysis) relevantFormats.push(...formats.video);
    if (analysis?.format === 'svg') relevantFormats = [];
    if (analysis?.isAnimated) relevantFormats = relevantFormats.filter(fmt => fmt.extension === analysis.format);
    relevantFormats = relevantFormats.filter(fmt => !['svg', 'heic', 'avif'].includes(fmt.extension));

    for (const fmt of relevantFormats) {
        const option = document.createElement('option');
        option.value = fmt.extension;
        option.textContent = `${fmt.extension.toUpperCase()} (${fmt.mime})`;
        select.appendChild(option);
    }
}

// Preview

function clearPreview(containerId) {
    const url = previewUrls.get(containerId);
    if (url) URL.revokeObjectURL(url);
    previewUrls.delete(containerId);
    document.getElementById(containerId).replaceChildren();
}

function clearCompressionResult() {
    compressedData = null;
    compressedMime = null;
    compressedExtension = null;
    clearPreview('compressed-preview');
    document.getElementById('download-btn').disabled = true;
    document.getElementById('results-content').classList.add('hidden');
    document.getElementById('results-empty').classList.remove('hidden');
}

function showPreview(containerId, data, mime) {
    clearPreview(containerId);
    const container = document.getElementById(containerId);
    if (mime?.startsWith('image/')) {
        const url = URL.createObjectURL(new Blob([data], { type: mime }));
        previewUrls.set(containerId, url);
        const img = document.createElement('img');
        img.alt = containerId === 'original-preview' ? 'Original image' : 'Compressed image';
        const release = failed => {
            URL.revokeObjectURL(url);
            if (previewUrls.get(containerId) !== url) return;
            previewUrls.delete(containerId);
            if (failed) container.textContent = 'Failed to load preview';
        };
        img.onload = () => release(false);
        img.onerror = () => release(true);
        img.src = url;
        container.appendChild(img);
    } else { container.textContent = 'Preview not available for this format'; }
}
const showOriginalPreview = (data, mime) => showPreview('original-preview', data, mime);
const showCompressedPreview = (data, mime) => showPreview('compressed-preview', data, mime);

// Compression

function resultStats(result) {
    return {
        original_size: result.original_size,
        compressed_size: result.compressed_size,
        compression_ratio: result.compression_ratio,
        time_ms: result.time_ms,
        format_in: result.format_in,
        format_out: result.format_out
    };
}

function updateProcessingButtons() {
    document.getElementById('compress-btn').disabled = !wasm || !currentFileData || !currentAnalysis || currentAnalysis.format === 'unknown' || compressionRunning || batchRunning;
    document.getElementById('run-all-btn').disabled = !wasm || compressionRunning || batchRunning || !testdataFiles.length;
    document.getElementById('clear-results-btn').disabled = batchRunning;
}

async function compress() {
    if (!wasm || !currentFileData || !currentAnalysis || currentAnalysis.format === 'unknown' || compressionRunning || batchRunning) return;
    compressionRunning = true;
    const file = currentFile;
    const loading = showLoading('Compressing...');
    clearCompressionResult();
    updateProcessingButtons();
    let result;
    try {
        result = await wasm.compress(currentFileData, JSON.stringify(buildConfig()));
        if (file !== currentFile) return;
        const stats = resultStats(result);
        const ownedResult = result;
        result = null; // into_data consumes the Rust result.
        compressedData = ownedResult.into_data();
        compressedExtension = stats.format_out;
        compressedMime = Object.values(supportedFormats).flat()
            .find(format => format.extension === compressedExtension)?.mime || 'application/octet-stream';
        showCompressedPreview(compressedData, compressedMime);
        updateStats(stats);
        document.getElementById('results-empty').classList.add('hidden');
        document.getElementById('results-content').classList.remove('hidden');
        document.getElementById('download-btn').disabled = false;
    } catch (error) {
        console.error('Compression error:', error);
        if (file === currentFile) {
            clearCompressionResult();
            alert('Compression failed: ' + errorMessage(error));
        }
    } finally {
        result?.free();
        compressionRunning = false;
        updateProcessingButtons();
        hideLoading(loading);
    }
}

function buildConfig(forBatch = false) {
    const format = currentAnalysis?.format;
    const mime = currentAnalysis?.mime || '';
    const mediaOptions = forBatch || /^(audio|video)\//.test(mime);
    const imageOptions = forBatch || ((mime.startsWith('image/') || mime.startsWith('video/'))
        && format !== 'svg' && !(currentAnalysis?.isAnimated && ['png', 'webp'].includes(format)));
    const config = {
        quality: parseInt(document.getElementById('quality').value)
    };

    const outputFormat = document.getElementById('output-format').value;
    if (outputFormat) config.output_format = outputFormat;

    const resizeWidth = document.getElementById('resize-width').value;
    const resizeHeight = document.getElementById('resize-height').value;
    if (imageOptions && (resizeWidth || resizeHeight)) {
        config.resize = {
            width: resizeWidth ? parseInt(resizeWidth) : null,
            height: resizeHeight ? parseInt(resizeHeight) : null,
            mode: document.getElementById('resize-mode').value,
            preserve_aspect: true
        };
    }

    const trimStart = document.getElementById('trim-start').value;
    const trimEnd = document.getElementById('trim-end').value;
    if (mediaOptions && (trimStart || trimEnd)) {
        config.trim = {
            start_ms: trimStart ? parseInt(trimStart) : null,
            end_ms: trimEnd ? parseInt(trimEnd) : null
        };
    }

    const audioBitrate = document.getElementById('audio-bitrate').value;
    if (mediaOptions && audioBitrate) {
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

// Batch Processing

async function runBatch() {
    if (!wasm || batchRunning || compressionRunning || testdataFiles.length === 0) return;

    batchRunning = true;
    batchAbort = false;
    batchResults = [];
    batchController = new AbortController();
    updateDashboardTable();
    updateProcessingButtons();

    document.getElementById('run-all-btn').disabled = true;
    document.getElementById('stop-btn').disabled = false;
    document.getElementById('batch-progress').classList.remove('hidden');
    document.getElementById('batch-summary').classList.add('hidden');

    let processed = 0;
    try {
        const configJson = JSON.stringify(buildConfig(true));

        for (const file of testdataFiles) {
            if (batchAbort) break;

            updateBatchProgress(processed, testdataFiles.length, `Processing ${file.name}...`);
            updateFileRowStatus(file, 'processing');

            let result;
            try {
                const response = await fetch(file.path, { signal: batchController.signal });
                if (!response.ok) throw new Error(`HTTP ${response.status}`);

                const arrayBuffer = await response.arrayBuffer();
                const data = new Uint8Array(arrayBuffer);
                if (batchAbort) break;

                const startTime = performance.now();
                result = await wasm.compress(data, configJson);
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
                if (batchAbort && err?.name === 'AbortError') break;
                batchResults.push({
                    file: file.name,
                    category: file.category,
                    originalSize: 0,
                    compressedSize: 0,
                    ratio: 0,
                    time: 0,
                    error: errorMessage(err)
                });
                updateFileRowError(file, errorMessage(err));
            } finally { result?.free(); }

            processed++;
        }

    } catch (error) {
        alert('Batch failed: ' + errorMessage(error));
    } finally {
        batchRunning = false;
        batchController = null;
        updateProcessingButtons();
        document.querySelectorAll('#dashboard-tbody .spinner').forEach(spinner => { spinner.parentElement.textContent = 'Stopped'; });
        document.getElementById('stop-btn').disabled = true;
        document.getElementById('batch-progress').classList.add('hidden');

        updateBatchSummary();
    }
}

function stopBatch() {
    batchAbort = true;
    batchController?.abort();
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
            <td><span class="badge badge-${file.category === 'images' ? 'image' : file.category}">${file.category}</span></td>
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
    if (batchRunning) return;
    batchResults = [];
    updateDashboardTable();
    document.getElementById('batch-summary').classList.add('hidden');
}

// Download & Export

function downloadCompressed() {
    if (!compressedData || !currentFile) return;

    const ext = compressedExtension || 'bin';
    const fileName = currentFile.name.replace(/\.[^.]+$/, '') + '_compressed.' + ext;

    const blob = new Blob([compressedData], { type: compressedMime });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 1000);
}

async function copyStats() {
    if (!compressedData) return;

    const stats = {
        file: currentFile.name,
        originalSize: document.getElementById('stats-original').textContent,
        compressedSize: document.getElementById('stats-compressed').textContent,
        savings: document.getElementById('stats-savings').textContent,
        time: document.getElementById('stats-time').textContent
    };

    try { await navigator.clipboard.writeText(JSON.stringify(stats, null, 2)); }
    catch (error) { alert('Could not copy stats: ' + (error.message || String(error))); }
}

// Utilities

function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.max(0, Math.min(sizes.length - 1, Math.floor(Math.log(bytes) / Math.log(k))));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function errorMessage(error) {
    return error?.message || String(error ?? 'Unknown error');
}

function showLoading(text) {
    const operation = ++loadingOperation;
    document.getElementById('loading-text').textContent = text;
    document.getElementById('loading-overlay').classList.remove('hidden');
    return operation;
}

function hideLoading(operation) {
    if (operation !== loadingOperation) return;
    document.getElementById('loading-overlay').classList.add('hidden');
}

// UI Toggles (exposed globally)

window.togglePanel = function(panelId) {
    const panel = document.getElementById(panelId);
    panel.classList.toggle('collapsed');
};

window.toggleTreeCategory = function(categoryEl) {
    categoryEl.classList.toggle('collapsed');
    const arrow = categoryEl.querySelector('.tree-category-header span:first-child');
    arrow.textContent = categoryEl.classList.contains('collapsed') ? '▶' : '▼';
};

// Event Listeners

function initEventListeners() {
    document.getElementById('quality').addEventListener('input', (e) => {
        document.getElementById('quality-value').textContent = e.target.value;
    });

    document.getElementById('compress-btn').addEventListener('click', compress);
    document.getElementById('download-btn').addEventListener('click', downloadCompressed);
    document.getElementById('copy-stats-btn').addEventListener('click', copyStats);
    document.getElementById('run-all-btn').addEventListener('click', runBatch);
    document.getElementById('stop-btn').addEventListener('click', stopBatch);
    document.getElementById('clear-results-btn').addEventListener('click', clearBatchResults);

    const dropZone = document.getElementById('drop-zone');
    const fileInput = document.getElementById('file-input');

    dropZone.addEventListener('click', () => fileInput.click());
    dropZone.addEventListener('keydown', event => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); fileInput.click(); }
    });

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
            await loadLocalFile(file);
        }
    });

    fileInput.addEventListener('change', async (e) => {
        const file = e.target.files[0];
        if (file) {
            await loadLocalFile(file);
        }
    });
}

// Initialize

initEventListeners();
initWasm();
