/**
 * utf8proj Playground - Main JavaScript Module
 *
 * This module handles:
 * - Monaco editor initialization with custom syntax highlighting
 * - WASM module loading and interaction
 * - UI controls (run, format, examples, share, theme)
 * - Panel resizing
 */

// ============================================================================
// WASM Module Loading
// ============================================================================

let playground = null;
let wasmModule = null;
let wasmReady = false;

async function initWasm() {
    try {
        wasmModule = await import('../pkg/utf8proj_wasm.js');
        await wasmModule.default();
        playground = new wasmModule.Playground();
        wasmReady = true;
        setStatus('Ready', 'success');
        console.log('WASM module loaded successfully');
    } catch (error) {
        console.error('Failed to load WASM module:', error);
        setStatus('Failed to load WASM module', 'error');
    }
}

// ============================================================================
// Monaco Editor Setup
// ============================================================================

let editor = null;

// Native DSL syntax definition
const nativeDslLanguage = {
    defaultToken: '',
    tokenPostfix: '.proj',

    keywords: [
        'project', 'task', 'resource', 'calendar', 'milestone',
        'effort', 'duration', 'depends', 'assign', 'allocate',
        'start', 'end', 'priority', 'complete', 'capacity',
        'rate', 'efficiency', 'working_hours', 'working_days',
        'holiday', 'must_start_on', 'must_finish_on',
        // RFC-0008: Progress-aware scheduling
        'remaining', 'status_date',
        // RFC-0004: Progressive resource refinement
        'profile', 'trait', 'specializes', 'skills',
        // Extended task/resource attributes
        'tag', 'note', 'cost', 'payment', 'leave', 'role', 'email',
        'currency', 'timezone', 'summary', 'constraint'
    ],

    typeKeywords: ['true', 'false'],

    operators: [':', ',', '{', '}', '@', '!', '~', '+', '-'],

    symbols: /[=><!~?:&|+\-*\/\^%@]+/,

    tokenizer: {
        root: [
            // Comments
            [/#.*$/, 'comment'],

            // Strings
            [/"([^"\\]|\\.)*$/, 'string.invalid'],
            [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],

            // Numbers with units
            [/\d+(\.\d+)?[dhwm]/, 'number.unit'],
            [/\d+(\.\d+)?%/, 'number.percentage'],
            [/\d+(\.\d+)?/, 'number'],

            // Dates
            [/\d{4}-\d{2}-\d{2}/, 'number.date'],

            // Identifiers and keywords
            [/[a-zA-Z_][\w]*/, {
                cases: {
                    '@keywords': 'keyword',
                    '@typeKeywords': 'keyword.type',
                    '@default': 'identifier'
                }
            }],

            // Whitespace
            { include: '@whitespace' },

            // Delimiters and operators
            [/[{}()\[\]]/, '@brackets'],
            [/@symbols/, {
                cases: {
                    '@operators': 'operator',
                    '@default': ''
                }
            }],

            // Comma
            [/,/, 'delimiter.comma'],
            [/:/, 'delimiter.colon'],
        ],

        string: [
            [/[^\\"]+/, 'string'],
            [/\\./, 'string.escape'],
            [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }]
        ],

        whitespace: [
            [/[ \t\r\n]+/, 'white'],
        ],
    }
};

// TaskJuggler syntax definition
const tjpLanguage = {
    defaultToken: '',
    tokenPostfix: '.tjp',

    keywords: [
        'project', 'task', 'resource', 'account', 'shift', 'leaves',
        'effort', 'duration', 'length', 'depends', 'allocate',
        'start', 'end', 'period', 'priority', 'complete', 'limits',
        'rate', 'efficiency', 'vacation', 'workinghours', 'timezone',
        'milestone', 'flags', 'note', 'journalentry', 'booking',
        'supplement', 'include', 'macro', 'report', 'taskreport',
        'resourcereport', 'textreport', 'tracereport', 'statusreport',
        'icalreport', 'export', 'navigator', 'columns', 'formats',
        'scenarios', 'extend', 'projection', 'now'
    ],

    typeKeywords: ['yes', 'no'],

    operators: [':', ',', '{', '}', '!', '~', '+', '-', '|', '&'],

    symbols: /[=><!~?:&|+\-*\/\^%]+/,

    tokenizer: {
        root: [
            // Comments
            [/#.*$/, 'comment'],
            [/\/\/.*$/, 'comment'],

            // Strings
            [/"([^"\\]|\\.)*$/, 'string.invalid'],
            [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],

            // Numbers with units
            [/\d+(\.\d+)?[dhwmy]/, 'number.unit'],
            [/\d+(\.\d+)?%/, 'number.percentage'],
            [/\d+(\.\d+)?/, 'number'],

            // Dates
            [/\d{4}-\d{2}-\d{2}/, 'number.date'],

            // Identifiers and keywords
            [/[a-zA-Z_][\w.]*/, {
                cases: {
                    '@keywords': 'keyword',
                    '@typeKeywords': 'keyword.type',
                    '@default': 'identifier'
                }
            }],

            // Whitespace
            { include: '@whitespace' },

            // Delimiters and operators
            [/[{}()\[\]]/, '@brackets'],
            [/@symbols/, {
                cases: {
                    '@operators': 'operator',
                    '@default': ''
                }
            }],

            // Delimiters
            [/,/, 'delimiter.comma'],
        ],

        string: [
            [/[^\\"]+/, 'string'],
            [/\\./, 'string.escape'],
            [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }]
        ],

        whitespace: [
            [/[ \t\r\n]+/, 'white'],
        ],
    }
};

// Theme definitions
const lightTheme = {
    base: 'vs',
    inherit: true,
    rules: [
        { token: 'comment', foreground: '6a9955' },
        { token: 'keyword', foreground: '0000ff', fontStyle: 'bold' },
        { token: 'keyword.type', foreground: '0000ff' },
        { token: 'string', foreground: 'a31515' },
        { token: 'number', foreground: '098658' },
        { token: 'number.unit', foreground: '098658', fontStyle: 'bold' },
        { token: 'number.date', foreground: '098658' },
        { token: 'number.percentage', foreground: '098658' },
        { token: 'identifier', foreground: '001080' },
        { token: 'operator', foreground: '000000' },
    ],
    colors: {}
};

const darkTheme = {
    base: 'vs-dark',
    inherit: true,
    rules: [
        { token: 'comment', foreground: '6a9955' },
        { token: 'keyword', foreground: '569cd6', fontStyle: 'bold' },
        { token: 'keyword.type', foreground: '569cd6' },
        { token: 'string', foreground: 'ce9178' },
        { token: 'number', foreground: 'b5cea8' },
        { token: 'number.unit', foreground: 'b5cea8', fontStyle: 'bold' },
        { token: 'number.date', foreground: 'b5cea8' },
        { token: 'number.percentage', foreground: 'b5cea8' },
        { token: 'identifier', foreground: '9cdcfe' },
        { token: 'operator', foreground: 'd4d4d4' },
    ],
    colors: {}
};

function initMonaco() {
    return new Promise((resolve) => {
        require.config({ paths: { 'vs': 'https://cdn.jsdelivr.net/npm/monaco-editor@0.45.0/min/vs' } });

        require(['vs/editor/editor.main'], function() {
            // Register custom languages
            monaco.languages.register({ id: 'proj' });
            monaco.languages.register({ id: 'tjp' });

            monaco.languages.setMonarchTokensProvider('proj', nativeDslLanguage);
            monaco.languages.setMonarchTokensProvider('tjp', tjpLanguage);

            // Register themes
            monaco.editor.defineTheme('utf8proj-light', lightTheme);
            monaco.editor.defineTheme('utf8proj-dark', darkTheme);

            // Create editor
            const container = document.getElementById('editor');
            const isDark = document.documentElement.getAttribute('data-theme') === 'dark';

            editor = monaco.editor.create(container, {
                value: getDefaultCode(),
                language: 'proj',
                theme: isDark ? 'utf8proj-dark' : 'utf8proj-light',
                automaticLayout: true,
                minimap: { enabled: false },
                fontSize: 14,
                lineNumbers: 'on',
                scrollBeyondLastLine: false,
                wordWrap: 'on',
                tabSize: 4,
                insertSpaces: true,
            });

            // Keyboard shortcuts
            editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, runSchedule);

            // Auto-validate on change
            editor.onDidChangeModelContent(debounce(validateInput, 500));

            resolve();
        });
    });
}

function getDefaultCode() {
    return `# My Project
# Edit this code or load an example from the dropdown

project "My First Project" {
    start: 2025-01-06
}

resource dev "Developer" {
    capacity: 1.0
}

task planning "Planning" {
    effort: 3d
    assign: dev
}

task development "Development" {
    effort: 10d
    depends: planning
    assign: dev
}

task testing "Testing" {
    effort: 5d
    depends: development
    assign: dev
}

task release "Release" {
    milestone: true
    depends: testing
}
`;
}

// ============================================================================
// Core Functionality
// ============================================================================

/**
 * Auto-detect format from content
 * Native DSL uses colons (start:, effort:, depends:)
 * TJP uses spaces (effort 5d, depends !task)
 */
function detectFormat(content) {
    // Look for Native DSL patterns (with colons)
    const hasNativePatterns = /\b(start|effort|duration|depends|assign|capacity|priority|complete|milestone):\s/.test(content);

    // Look for TJP patterns (without colons for these keywords)
    const hasTjpPatterns = /\b(effort|duration|depends|allocate|milestone)\s+[^:]/.test(content);

    // TJP project has id before name: "project id "name""
    const hasTjpProject = /^project\s+\w+\s+"/.test(content.trim());

    // Native project: "project "name" {"
    const hasNativeProject = /^project\s+"[^"]+"\s*\{/.test(content.trim());

    if (hasNativeProject || (hasNativePatterns && !hasTjpPatterns)) {
        return 'native';
    }
    if (hasTjpProject || (hasTjpPatterns && !hasNativePatterns)) {
        return 'tjp';
    }

    // Default to current selection if ambiguous
    return null;
}

function runSchedule() {
    if (!wasmReady) {
        setStatus('WASM module not loaded', 'error');
        return;
    }

    const input = editor.getValue();
    let format = document.getElementById('format-select').value;

    // Auto-detect and fix format mismatch
    const detectedFormat = detectFormat(input);
    if (detectedFormat && detectedFormat !== format) {
        format = detectedFormat;
        document.getElementById('format-select').value = format;
        monaco.editor.setModelLanguage(editor.getModel(), format === 'native' ? 'proj' : 'tjp');
    }
    const leveling = document.getElementById('leveling-checkbox').checked;
    const focusInput = document.getElementById('focus-input').value.trim();
    const contextDepth = parseInt(document.getElementById('context-depth-select').value, 10);

    setStatus('Scheduling...', '');

    playground.set_resource_leveling(leveling);
    const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
    playground.set_dark_theme(isDark);

    // Apply focus view settings
    if (focusInput) {
        const patterns = focusInput.split(',').map(p => p.trim()).filter(p => p);
        playground.set_focus(patterns);
        playground.set_context_depth(contextDepth);
    } else {
        playground.clear_focus();
    }

    try {
        const result = playground.schedule(input, format);

        if (result.success) {
            // Show Gantt chart
            const ganttHtml = playground.render_gantt();
            displayGantt(ganttHtml);

            // Show JSON
            const jsonOutput = document.getElementById('json-output');
            jsonOutput.textContent = JSON.stringify(result.data, null, 2);

            // Update status
            const data = result.data;
            setStatus('Scheduled successfully', 'success');
            document.getElementById('schedule-info').textContent =
                `${data.tasks.length} tasks | ${data.duration_days} days | ${data.critical_path.length} critical`;
        } else {
            setStatus(result.error || 'Unknown error', 'error');
            showError(result.error);
        }
    } catch (error) {
        console.error('Schedule error:', error);
        setStatus(error.message || 'Schedule failed', 'error');
        showError(error.message || 'An unexpected error occurred');
    }
}

function validateInput() {
    if (!wasmReady) return;

    const input = editor.getValue();
    let format = document.getElementById('format-select').value;

    // Use auto-detected format for validation
    const detectedFormat = detectFormat(input);
    if (detectedFormat) {
        format = detectedFormat;
    }

    const result = playground.validate(input, format);

    // Clear existing markers
    const model = editor.getModel();
    monaco.editor.setModelMarkers(model, 'validation', []);

    if (result.errors && result.errors.length > 0) {
        const markers = result.errors.map(err => ({
            severity: monaco.MarkerSeverity.Error,
            message: err.message,
            startLineNumber: err.line || 1,
            startColumn: err.column || 1,
            endLineNumber: err.line || 1,
            endColumn: 1000,
        }));
        monaco.editor.setModelMarkers(model, 'validation', markers);
    }
}

function displayGantt(html) {
    const container = document.getElementById('gantt-output');

    // Create an iframe to isolate the Gantt chart styles
    container.innerHTML = '';
    const iframe = document.createElement('iframe');
    iframe.style.width = '100%';
    iframe.style.height = '100%';
    iframe.style.border = 'none';
    iframe.style.minHeight = '400px';
    container.appendChild(iframe);

    // Write HTML to iframe
    const doc = iframe.contentDocument || iframe.contentWindow.document;
    doc.open();
    doc.write(html);
    doc.close();
}

function showError(message) {
    const container = document.getElementById('gantt-output');
    container.innerHTML = `
        <div class="error-display">
            <strong>Error:</strong><br>
            <pre>${escapeHtml(message)}</pre>
        </div>
    `;

    document.getElementById('json-output').textContent = JSON.stringify({ error: message }, null, 2);
}

// ============================================================================
// UI Controls
// ============================================================================

function setupEventListeners() {
    // Run button
    document.getElementById('run-btn').addEventListener('click', runSchedule);

    // Format selector
    document.getElementById('format-select').addEventListener('change', (e) => {
        const format = e.target.value;
        const model = editor.getModel();
        monaco.editor.setModelLanguage(model, format === 'native' ? 'proj' : 'tjp');
        validateInput();
    });

    // Example selector
    document.getElementById('example-select').addEventListener('change', (e) => {
        const value = e.target.value;
        if (!value) return;

        let code = '';
        let format = 'native';

        switch (value) {
            case 'native':
                code = wasmModule ? wasmModule.Playground.get_example_native() : getDefaultCode();
                format = 'native';
                break;
            case 'tjp':
                code = wasmModule ? wasmModule.Playground.get_example_tjp() : '';
                format = 'tjp';
                break;
            case 'hierarchical':
                code = wasmModule ? wasmModule.Playground.get_example_hierarchical() : '';
                format = 'native';
                break;
            case 'progress':
                code = wasmModule ? wasmModule.Playground.get_example_progress() : '';
                format = 'native';
                break;
            default:
                return;
        }

        document.getElementById('format-select').value = format;
        monaco.editor.setModelLanguage(editor.getModel(), format === 'native' ? 'proj' : 'tjp');
        editor.setValue(code);
        e.target.value = ''; // Reset selector
    });

    // Share button
    document.getElementById('share-btn').addEventListener('click', showShareModal);
    document.getElementById('share-modal-close').addEventListener('click', hideShareModal);
    document.getElementById('copy-url-btn').addEventListener('click', copyShareUrl);
    document.getElementById('share-modal').addEventListener('click', (e) => {
        if (e.target.id === 'share-modal') hideShareModal();
    });

    // Theme toggle
    document.getElementById('theme-btn').addEventListener('click', toggleTheme);

    // Download buttons
    document.getElementById('download-proj-btn').addEventListener('click', downloadProject);
    document.getElementById('export-btn').addEventListener('click', exportGantt);

    // Tab switching
    document.querySelectorAll('.tab').forEach(tab => {
        tab.addEventListener('click', (e) => {
            const tabName = e.target.dataset.tab;
            switchTab(tabName);
        });
    });

    // Resize handle
    setupResizeHandle();

    // Load from URL on page load
    loadFromUrl();
}

function switchTab(tabName) {
    // Update tab buttons
    document.querySelectorAll('.tab').forEach(tab => {
        tab.classList.toggle('active', tab.dataset.tab === tabName);
    });

    // Show/hide containers
    document.getElementById('preview-gantt').classList.toggle('active', tabName === 'gantt');
    document.getElementById('preview-json').classList.toggle('active', tabName === 'json');
}

function toggleTheme() {
    const html = document.documentElement;
    const isDark = html.getAttribute('data-theme') === 'dark';
    const newTheme = isDark ? 'light' : 'dark';

    html.setAttribute('data-theme', newTheme);
    localStorage.setItem('theme', newTheme);

    // Update Monaco theme
    monaco.editor.setTheme(newTheme === 'dark' ? 'utf8proj-dark' : 'utf8proj-light');

    // Update button
    document.getElementById('theme-btn').textContent = newTheme === 'dark' ? '☀️' : '🌙';

    // Update playground theme if we have a schedule
    if (playground && playground.has_schedule()) {
        playground.set_dark_theme(newTheme === 'dark');
        const ganttHtml = playground.render_gantt();
        displayGantt(ganttHtml);
    }
}

function loadSavedTheme() {
    const saved = localStorage.getItem('theme');
    if (saved === 'dark') {
        document.documentElement.setAttribute('data-theme', 'dark');
        document.getElementById('theme-btn').textContent = '☀️';
    }
}

// ============================================================================
// Share Functionality
// ============================================================================

function showShareModal() {
    const code = editor.getValue();
    const format = document.getElementById('format-select').value;
    const leveling = document.getElementById('leveling-checkbox').checked;

    // Compress and encode the project using LZ-string
    const data = {
        code,
        format,
        leveling
    };

    const compressed = LZString.compressToEncodedURIComponent(JSON.stringify(data));
    // Use fragment (#) instead of query param (?) to avoid server header limits
    const url = `${window.location.origin}${window.location.pathname}#p=${compressed}`;

    // Check URL length (browsers typically support up to ~64KB for fragments)
    if (url.length > 64000) {
        setStatus(`Warning: URL is ${url.length} chars (may be too long)`, 'error');
    }

    document.getElementById('share-url').value = url;
    document.getElementById('share-modal').classList.remove('hidden');
}

function hideShareModal() {
    document.getElementById('share-modal').classList.add('hidden');
}

async function copyShareUrl() {
    const input = document.getElementById('share-url');
    try {
        await navigator.clipboard.writeText(input.value);
        const btn = document.getElementById('copy-url-btn');
        const original = btn.textContent;
        btn.textContent = 'Copied!';
        setTimeout(() => btn.textContent = original, 2000);
    } catch (err) {
        input.select();
        document.execCommand('copy');
    }
}

function loadFromUrl() {
    // Check fragment first (new format), then query param (backward compat)
    let encoded = null;

    // Try fragment (#p=...)
    const hash = window.location.hash;
    if (hash.startsWith('#p=')) {
        encoded = hash.substring(3);
    }

    // Fallback to query param (?p=...)
    if (!encoded) {
        const params = new URLSearchParams(window.location.search);
        encoded = params.get('p');
    }

    if (encoded) {
        try {
            let data;

            // Try LZ-string decompression first (new format)
            const decompressed = LZString.decompressFromEncodedURIComponent(encoded);
            if (decompressed) {
                data = JSON.parse(decompressed);
            } else {
                // Fallback to old base64 encoding for backward compatibility
                data = JSON.parse(decodeURIComponent(atob(encoded)));
            }

            if (data.code) {
                editor.setValue(data.code);
            }
            if (data.format) {
                document.getElementById('format-select').value = data.format;
                monaco.editor.setModelLanguage(editor.getModel(), data.format === 'native' ? 'proj' : 'tjp');
            }
            if (data.leveling !== undefined) {
                document.getElementById('leveling-checkbox').checked = data.leveling;
            }

            // Auto-run if we have a shared project
            setTimeout(() => {
                if (wasmReady) runSchedule();
            }, 500);
        } catch (e) {
            console.error('Failed to load from URL:', e);
        }
    }
}

// ============================================================================
// Download Functions
// ============================================================================

function downloadProject() {
    const code = editor.getValue();
    const format = document.getElementById('format-select').value;
    const ext = format === 'native' ? 'proj' : 'tjp';

    downloadFile(`project.${ext}`, code, 'text/plain');
}

function exportGantt() {
    if (!playground || !playground.has_schedule()) {
        setStatus('No schedule to export', 'error');
        return;
    }

    const format = document.getElementById('export-format-select').value;
    let content, filename, mimeType;

    switch (format) {
        case 'html':
            content = playground.render_gantt();
            filename = 'gantt.html';
            mimeType = 'text/html';
            break;
        case 'mermaid':
            content = playground.render_mermaid();
            filename = 'gantt.mmd';
            mimeType = 'text/plain';
            break;
        case 'plantuml':
            content = playground.render_plantuml();
            filename = 'gantt.puml';
            mimeType = 'text/plain';
            break;
        default:
            setStatus('Unknown export format', 'error');
            return;
    }

    downloadFile(filename, content, mimeType);
    setStatus(`Exported ${format.toUpperCase()} successfully`, 'success');
}

function downloadFile(filename, content, mimeType) {
    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

// ============================================================================
// Panel Resizing
// ============================================================================

function setupResizeHandle() {
    const handle = document.getElementById('resize-handle');
    const editorPanel = document.querySelector('.editor-panel');
    const previewPanel = document.querySelector('.preview-panel');

    let isResizing = false;
    let startX = 0;
    let startEditorWidth = 0;

    handle.addEventListener('mousedown', (e) => {
        isResizing = true;
        startX = e.clientX;
        startEditorWidth = editorPanel.offsetWidth;
        handle.classList.add('dragging');
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;

        const dx = e.clientX - startX;
        const containerWidth = editorPanel.parentElement.offsetWidth;
        const newEditorWidth = startEditorWidth + dx;

        // Enforce minimum widths
        const minWidth = 300;
        if (newEditorWidth < minWidth || containerWidth - newEditorWidth - 6 < minWidth) {
            return;
        }

        const editorPercent = (newEditorWidth / containerWidth) * 100;
        editorPanel.style.flex = `0 0 ${editorPercent}%`;
        previewPanel.style.flex = '1';
    });

    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            handle.classList.remove('dragging');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// ============================================================================
// Utilities
// ============================================================================

function setStatus(message, type) {
    const statusBar = document.querySelector('.status-bar');
    const statusMessage = document.getElementById('status-message');

    statusBar.classList.remove('error', 'success');
    if (type) {
        statusBar.classList.add(type);
    }

    statusMessage.textContent = message;
}

function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// ============================================================================
// Initialization
// ============================================================================

async function init() {
    loadSavedTheme();
    setStatus('Loading...', '');

    try {
        await initMonaco();
        setupEventListeners();
        await initWasm();
    } catch (error) {
        console.error('Initialization error:', error);
        setStatus('Failed to initialize', 'error');
    }
}

// Start the app
init();
