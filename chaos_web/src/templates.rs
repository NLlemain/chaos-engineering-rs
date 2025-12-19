//! HTML templates with embedded CSS
//! Professional dark theme inspired by ChatGPT/enterprise applications

/// Base HTML layout with dark theme CSS
pub fn base_layout(title: &str, content: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - Chaos Dashboard</title>
    <style>
        {CSS}
    </style>
</head>
<body>
    <nav class="navbar">
        <div class="nav-brand">
            <span class="logo">🦀</span>
            <span class="brand-text">Chaos Dashboard</span>
        </div>
        <div class="nav-links">
            <a href="/" class="nav-link">Dashboard</a>
            <a href="/scenarios" class="nav-link">Scenarios</a>
            <a href="/run" class="nav-link">Run Test</a>
            <a href="/load-test" class="nav-link">Load Test</a>
            <a href="/targets" class="nav-link">Targets</a>
            <a href="/results" class="nav-link">Results</a>
        </div>
    </nav>
    <main class="main-content">
        {content}
    </main>
    <footer class="footer">
        <p>Chaos Engineering Framework &copy; 2025</p>
    </footer>
    <script>
        {JS}
    </script>
</body>
</html>"##,
        title = title,
        content = content,
        CSS = CSS_STYLES,
        JS = JS_SCRIPTS
    )
}

/// Dark theme CSS styles
const CSS_STYLES: &str = r##"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

:root {
    --bg-primary: #0d0d0d;
    --bg-secondary: #1a1a1a;
    --bg-tertiary: #2d2d2d;
    --bg-hover: #3d3d3d;
    --text-primary: #ffffff;
    --text-secondary: #a0a0a0;
    --text-muted: #6b6b6b;
    --accent-blue: #3b82f6;
    --accent-green: #22c55e;
    --accent-red: #ef4444;
    --accent-yellow: #eab308;
    --accent-purple: #a855f7;
    --border-color: #333333;
    --shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.3);
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    line-height: 1.6;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
}

/* Navigation */
.navbar {
    background-color: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    padding: 0 2rem;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    position: sticky;
    top: 0;
    z-index: 100;
}

.nav-brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}

.logo {
    font-size: 1.5rem;
}

.brand-text {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-primary);
}

.nav-links {
    display: flex;
    gap: 0.5rem;
}

.nav-link {
    color: var(--text-secondary);
    text-decoration: none;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    transition: all 0.2s ease;
    font-weight: 500;
}

.nav-link:hover {
    color: var(--text-primary);
    background-color: var(--bg-tertiary);
}

.nav-link.active {
    color: var(--accent-blue);
    background-color: rgba(59, 130, 246, 0.1);
}

/* Main Content */
.main-content {
    flex: 1;
    padding: 2rem;
    max-width: 1400px;
    margin: 0 auto;
    width: 100%;
}

/* Page Headers */
.page-header {
    margin-bottom: 2rem;
}

.page-title {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 0.5rem;
}

.page-subtitle {
    color: var(--text-secondary);
    font-size: 1rem;
}

/* Cards */
.card {
    background-color: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    transition: all 0.2s ease;
}

.card:hover {
    border-color: var(--bg-hover);
    box-shadow: var(--shadow);
}

.card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border-color);
}

.card-title {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-primary);
}

.card-body {
    color: var(--text-secondary);
}

/* Grid System */
.grid {
    display: grid;
    gap: 1.5rem;
}

.grid-2 {
    grid-template-columns: repeat(2, 1fr);
}

.grid-3 {
    grid-template-columns: repeat(3, 1fr);
}

.grid-4 {
    grid-template-columns: repeat(4, 1fr);
}

@media (max-width: 1024px) {
    .grid-4 { grid-template-columns: repeat(2, 1fr); }
    .grid-3 { grid-template-columns: repeat(2, 1fr); }
}

@media (max-width: 640px) {
    .grid-4, .grid-3, .grid-2 { grid-template-columns: 1fr; }
}

/* Stats Cards */
.stat-card {
    background-color: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
}

.stat-icon {
    font-size: 2rem;
    margin-bottom: 0.5rem;
}

.stat-value {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 0.25rem;
}

.stat-label {
    color: var(--text-secondary);
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

/* Buttons */
.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s ease;
    border: none;
    text-decoration: none;
}

.btn-primary {
    background-color: var(--accent-blue);
    color: white;
}

.btn-primary:hover {
    background-color: #2563eb;
}

.btn-success {
    background-color: var(--accent-green);
    color: white;
}

.btn-success:hover {
    background-color: #16a34a;
}

.btn-danger {
    background-color: var(--accent-red);
    color: white;
}

.btn-danger:hover {
    background-color: #dc2626;
}

.btn-secondary {
    background-color: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border-color);
}

.btn-secondary:hover {
    background-color: var(--bg-hover);
}

.btn-sm {
    padding: 0.5rem 1rem;
    font-size: 0.75rem;
}

.btn-lg {
    padding: 1rem 2rem;
    font-size: 1rem;
}

/* Tables */
.table-container {
    overflow-x: auto;
    border-radius: 12px;
    border: 1px solid var(--border-color);
}

table {
    width: 100%;
    border-collapse: collapse;
    background-color: var(--bg-secondary);
}

th, td {
    padding: 1rem;
    text-align: left;
    border-bottom: 1px solid var(--border-color);
}

th {
    background-color: var(--bg-tertiary);
    font-weight: 600;
    color: var(--text-primary);
    text-transform: uppercase;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
}

td {
    color: var(--text-secondary);
}

tr:hover td {
    background-color: var(--bg-tertiary);
}

tr:last-child td {
    border-bottom: none;
}

/* Status Badges */
.badge {
    display: inline-flex;
    align-items: center;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}

.badge-success {
    background-color: rgba(34, 197, 94, 0.15);
    color: var(--accent-green);
}

.badge-warning {
    background-color: rgba(234, 179, 8, 0.15);
    color: var(--accent-yellow);
}

.badge-danger {
    background-color: rgba(239, 68, 68, 0.15);
    color: var(--accent-red);
}

.badge-info {
    background-color: rgba(59, 130, 246, 0.15);
    color: var(--accent-blue);
}

.badge-neutral {
    background-color: var(--bg-tertiary);
    color: var(--text-secondary);
}

/* Progress Bar */
.progress-container {
    background-color: var(--bg-tertiary);
    border-radius: 9999px;
    height: 8px;
    overflow: hidden;
    margin: 1rem 0;
}

.progress-bar {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-blue), var(--accent-purple));
    border-radius: 9999px;
    transition: width 0.3s ease;
}

/* Forms */
.form-group {
    margin-bottom: 1.5rem;
}

.form-label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
    color: var(--text-primary);
}

.form-input, .form-select, .form-textarea {
    width: 100%;
    padding: 0.75rem 1rem;
    background-color: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    color: var(--text-primary);
    font-size: 1rem;
    transition: all 0.2s ease;
}

.form-input:focus, .form-select:focus, .form-textarea:focus {
    outline: none;
    border-color: var(--accent-blue);
    box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.form-textarea {
    min-height: 150px;
    resize: vertical;
}

/* Code Block */
.code-block {
    background-color: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-x: auto;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 0.875rem;
    line-height: 1.5;
    color: var(--text-secondary);
}

.code-block code {
    color: var(--text-primary);
}

/* Scenario Card */
.scenario-card {
    background-color: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 1.5rem;
    transition: all 0.2s ease;
    cursor: pointer;
}

.scenario-card:hover {
    border-color: var(--accent-blue);
    transform: translateY(-2px);
    box-shadow: var(--shadow);
}

.scenario-name {
    font-size: 1.125rem;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 0.5rem;
}

.scenario-meta {
    display: flex;
    gap: 1rem;
    color: var(--text-muted);
    font-size: 0.875rem;
    margin-top: 1rem;
}

.scenario-meta-item {
    display: flex;
    align-items: center;
    gap: 0.25rem;
}

/* Footer */
.footer {
    background-color: var(--bg-secondary);
    border-top: 1px solid var(--border-color);
    padding: 1.5rem 2rem;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.875rem;
}

/* Utilities */
.text-center { text-align: center; }
.text-right { text-align: right; }
.text-primary { color: var(--text-primary); }
.text-secondary { color: var(--text-secondary); }
.text-muted { color: var(--text-muted); }
.text-success { color: var(--accent-green); }
.text-danger { color: var(--accent-red); }
.text-warning { color: var(--accent-yellow); }
.text-info { color: var(--accent-blue); }

.mb-1 { margin-bottom: 0.5rem; }
.mb-2 { margin-bottom: 1rem; }
.mb-3 { margin-bottom: 1.5rem; }
.mb-4 { margin-bottom: 2rem; }
.mt-1 { margin-top: 0.5rem; }
.mt-2 { margin-top: 1rem; }
.mt-3 { margin-top: 1.5rem; }
.mt-4 { margin-top: 2rem; }

.flex { display: flex; }
.flex-col { flex-direction: column; }
.items-center { align-items: center; }
.justify-between { justify-content: space-between; }
.gap-1 { gap: 0.5rem; }
.gap-2 { gap: 1rem; }
.gap-3 { gap: 1.5rem; }

/* Loading Spinner */
.spinner {
    width: 40px;
    height: 40px;
    border: 3px solid var(--bg-tertiary);
    border-top-color: var(--accent-blue);
    border-radius: 50%;
    animation: spin 1s linear infinite;
}

@keyframes spin {
    to { transform: rotate(360deg); }
}

/* Empty State */
.empty-state {
    text-align: center;
    padding: 4rem 2rem;
    color: var(--text-muted);
}

.empty-state-icon {
    font-size: 4rem;
    margin-bottom: 1rem;
    opacity: 0.5;
}

.empty-state-title {
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-secondary);
    margin-bottom: 0.5rem;
}

/* Phase Timeline */
.timeline {
    position: relative;
    padding-left: 2rem;
}

.timeline::before {
    content: '';
    position: absolute;
    left: 0.5rem;
    top: 0;
    bottom: 0;
    width: 2px;
    background-color: var(--border-color);
}

.timeline-item {
    position: relative;
    padding-bottom: 1.5rem;
}

.timeline-item::before {
    content: '';
    position: absolute;
    left: -1.5rem;
    top: 0.5rem;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background-color: var(--bg-tertiary);
    border: 2px solid var(--border-color);
}

.timeline-item.active::before {
    background-color: var(--accent-blue);
    border-color: var(--accent-blue);
}

.timeline-item.completed::before {
    background-color: var(--accent-green);
    border-color: var(--accent-green);
}

.timeline-title {
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 0.25rem;
}

.timeline-content {
    color: var(--text-secondary);
    font-size: 0.875rem;
}

/* Alert */
.alert {
    padding: 1rem 1.25rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
}

.alert-info {
    background-color: rgba(59, 130, 246, 0.1);
    border: 1px solid rgba(59, 130, 246, 0.3);
    color: var(--accent-blue);
}

.alert-success {
    background-color: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.3);
    color: var(--accent-green);
}

.alert-warning {
    background-color: rgba(234, 179, 8, 0.1);
    border: 1px solid rgba(234, 179, 8, 0.3);
    color: var(--accent-yellow);
}

.alert-danger {
    background-color: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--accent-red);
}

/* Live Status */
.live-indicator {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
}

.live-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: var(--accent-green);
    animation: pulse 2s infinite;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
}
"##;

/// JavaScript for interactivity
const JS_SCRIPTS: &str = r##"
// Auto-refresh status when test is running
let statusInterval = null;
let loadTestInterval = null;

async function checkStatus() {
    try {
        const response = await fetch('/api/status');
        const status = await response.json();
        updateStatusUI(status);
        
        if (status.is_running && !statusInterval) {
            statusInterval = setInterval(checkStatus, 1000);
        } else if (!status.is_running && statusInterval) {
            clearInterval(statusInterval);
            statusInterval = null;
        }
    } catch (error) {
        console.error('Failed to fetch status:', error);
    }
}

function updateStatusUI(status) {
    const statusEl = document.getElementById('test-status');
    const progressEl = document.getElementById('progress-bar');
    const progressText = document.getElementById('progress-text');
    
    if (statusEl) {
        if (status.is_running) {
            statusEl.innerHTML = `
                <span class="live-indicator">
                    <span class="live-dot"></span>
                    Running: ${status.scenario_name || 'Unknown'}
                </span>
            `;
        } else {
            statusEl.innerHTML = '<span class="badge badge-neutral">Idle</span>';
        }
    }
    
    if (progressEl && status.is_running) {
        progressEl.style.width = `${status.progress_percent}%`;
    }
    
    if (progressText && status.is_running) {
        progressText.textContent = `${Math.round(status.progress_percent)}% - Phase: ${status.current_phase || 'Starting...'}`;
    }
}

async function runScenario(scenarioName) {
    try {
        const response = await fetch('/api/run', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ scenario: scenarioName })
        });
        
        if (response.ok) {
            checkStatus();
            window.location.href = '/run';
        } else {
            const error = await response.json();
            alert('Failed to start test: ' + (error.message || 'Unknown error'));
        }
    } catch (error) {
        alert('Failed to start test: ' + error.message);
    }
}

async function stopTest() {
    if (!confirm('Are you sure you want to stop the current test?')) return;
    
    try {
        const response = await fetch('/api/stop', { method: 'POST' });
        if (response.ok) {
            checkStatus();
        }
    } catch (error) {
        alert('Failed to stop test: ' + error.message);
    }
}

// Load Test Functions
async function startLoadTest() {
    const config = {
        name: document.getElementById('test-name')?.value || 'Load Test',
        target_type: document.getElementById('target-type')?.value || 'http',
        url: document.getElementById('target-url')?.value,
        method: document.getElementById('http-method')?.value || 'GET',
        body: document.getElementById('request-body')?.value || null,
        concurrent_users: parseInt(document.getElementById('concurrent-users')?.value) || 10,
        requests_per_second: parseInt(document.getElementById('rps')?.value) || 100,
        duration_secs: parseInt(document.getElementById('duration')?.value) || 60,
        timeout_ms: 5000,
        ramp_up_secs: parseInt(document.getElementById('ramp-up')?.value) || 10
    };

    if (!config.url) {
        alert('Please enter a target URL');
        return;
    }

    try {
        const response = await fetch('/api/load-test/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(config)
        });
        
        if (response.ok) {
            location.reload();
        } else {
            const error = await response.json();
            alert('Failed to start load test: ' + (error.message || 'Unknown error'));
        }
    } catch (error) {
        alert('Failed to start load test: ' + error.message);
    }
}

async function stopLoadTest() {
    if (!confirm('Stop the load test?')) return;
    
    try {
        await fetch('/api/load-test/stop', { method: 'POST' });
        location.reload();
    } catch (error) {
        alert('Failed to stop: ' + error.message);
    }
}

async function checkLoadTestStatus() {
    try {
        const response = await fetch('/api/load-test/status');
        const data = await response.json();
        if (data.is_running) {
            location.reload();
        }
    } catch (error) {
        console.error('Failed to check load test status:', error);
    }
}

// Target Management Functions
async function addTarget() {
    const target = {
        name: document.getElementById('target-name')?.value,
        target_type: document.getElementById('new-target-type')?.value,
        url: document.getElementById('new-target-url')?.value,
        description: document.getElementById('new-target-desc')?.value || null
    };

    if (!target.name || !target.url) {
        alert('Please fill in name and URL');
        return;
    }

    try {
        const response = await fetch('/api/targets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(target)
        });
        
        if (response.ok) {
            location.reload();
        } else {
            alert('Failed to add target');
        }
    } catch (error) {
        alert('Failed to add target: ' + error.message);
    }
}

async function deleteTarget(id) {
    if (!confirm('Delete this target?')) return;
    
    try {
        const response = await fetch('/api/targets/' + id, { method: 'DELETE' });
        if (response.ok) {
            location.reload();
        }
    } catch (error) {
        alert('Failed to delete: ' + error.message);
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', function() {
    checkStatus();
    
    // Check load test status on load test page
    if (window.location.pathname === '/load-test') {
        loadTestInterval = setInterval(checkLoadTestStatus, 2000);
    }
    
    // Highlight current nav link
    const currentPath = window.location.pathname;
    document.querySelectorAll('.nav-link').forEach(link => {
        if (link.getAttribute('href') === currentPath) {
            link.classList.add('active');
        }
    });
});
"##;

/// Generate the dashboard page
pub fn dashboard_page(
    total_scenarios: usize,
    total_results: usize,
    recent_results: &[crate::state::ResultSummary],
    status: &crate::state::TestStatus,
) -> String {
    let status_html = if status.is_running {
        format!(
            r#"<div class="card">
                <div class="card-header">
                    <h2 class="card-title">🔄 Test In Progress</h2>
                    <button class="btn btn-danger btn-sm" onclick="stopTest()">Stop Test</button>
                </div>
                <div class="card-body">
                    <p class="mb-2"><strong>Scenario:</strong> {}</p>
                    <p class="mb-2"><strong>Phase:</strong> {}</p>
                    <div class="progress-container">
                        <div class="progress-bar" id="progress-bar" style="width: {}%"></div>
                    </div>
                    <p class="text-secondary" id="progress-text">{:.1}% complete - {} / {} seconds</p>
                </div>
            </div>"#,
            status.scenario_name.as_deref().unwrap_or("Unknown"),
            status.current_phase.as_deref().unwrap_or("Starting..."),
            status.progress_percent,
            status.progress_percent,
            status.elapsed_seconds,
            status.total_seconds
        )
    } else {
        String::new()
    };

    let recent_results_html = if recent_results.is_empty() {
        r#"<div class="empty-state">
            <div class="empty-state-icon">📊</div>
            <p class="empty-state-title">No test results yet</p>
            <p>Run a chaos test to see results here</p>
        </div>"#
            .to_string()
    } else {
        let rows: String = recent_results
            .iter()
            .take(5)
            .map(|r| {
                let success_class = if r.success_rate >= 0.9 {
                    "text-success"
                } else if r.success_rate >= 0.7 {
                    "text-warning"
                } else {
                    "text-danger"
                };
                format!(
                    r#"<tr onclick="window.location='/results/{}'">
                    <td>{}</td>
                    <td class="{}">{:.1}%</td>
                    <td>{}s</td>
                    <td>{}</td>
                </tr>"#,
                    r.id,
                    r.scenario_name,
                    success_class,
                    r.success_rate * 100.0,
                    r.total_duration_secs,
                    r.timestamp.format("%Y-%m-%d %H:%M")
                )
            })
            .collect();

        format!(
            r#"<div class="table-container">
                <table>
                    <thead>
                        <tr>
                            <th>Scenario</th>
                            <th>Success Rate</th>
                            <th>Duration</th>
                            <th>Time</th>
                        </tr>
                    </thead>
                    <tbody>{}</tbody>
                </table>
            </div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Dashboard</h1>
            <p class="page-subtitle">Monitor and manage your chaos engineering tests</p>
        </div>
        
        {status_html}
        
        <div class="grid grid-4 mb-4">
            <div class="stat-card">
                <span class="stat-icon">📋</span>
                <span class="stat-value">{total_scenarios}</span>
                <span class="stat-label">Scenarios</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon">📊</span>
                <span class="stat-value">{total_results}</span>
                <span class="stat-label">Test Results</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon">⚡</span>
                <span class="stat-value">7</span>
                <span class="stat-label">Injector Types</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon" id="test-status">{status_badge}</span>
                <span class="stat-value">&nbsp;</span>
                <span class="stat-label">Status</span>
            </div>
        </div>
        
        <div class="card">
            <div class="card-header">
                <h2 class="card-title">Recent Results</h2>
                <a href="/results" class="btn btn-secondary btn-sm">View All</a>
            </div>
            <div class="card-body">
                {recent_results_html}
            </div>
        </div>
        
        <div class="grid grid-2">
            <div class="card">
                <div class="card-header">
                    <h2 class="card-title">Quick Actions</h2>
                </div>
                <div class="card-body flex flex-col gap-2">
                    <a href="/scenarios" class="btn btn-secondary">📋 Browse Scenarios</a>
                    <a href="/run" class="btn btn-primary">▶️ Run New Test</a>
                </div>
            </div>
            <div class="card">
                <div class="card-header">
                    <h2 class="card-title">Available Injectors</h2>
                </div>
                <div class="card-body">
                    <div class="flex gap-1" style="flex-wrap: wrap;">
                        <span class="badge badge-info">network_latency</span>
                        <span class="badge badge-info">packet_loss</span>
                        <span class="badge badge-info">tcp_reset</span>
                        <span class="badge badge-info">cpu_starvation</span>
                        <span class="badge badge-info">memory_pressure</span>
                        <span class="badge badge-info">disk_slow</span>
                        <span class="badge badge-info">process_kill</span>
                    </div>
                </div>
            </div>
        </div>"#,
        status_html = status_html,
        total_scenarios = total_scenarios,
        total_results = total_results,
        status_badge = if status.is_running {
            r#"<span class="live-indicator"><span class="live-dot"></span> Running</span>"#
        } else {
            r#"<span class="badge badge-neutral">Idle</span>"#
        },
        recent_results_html = recent_results_html
    );

    base_layout("Dashboard", &content)
}

/// Generate the scenarios list page
pub fn scenarios_page(scenarios: &[ScenarioInfo]) -> String {
    let scenarios_html = if scenarios.is_empty() {
        r#"<div class="empty-state">
            <div class="empty-state-icon">📋</div>
            <p class="empty-state-title">No scenarios found</p>
            <p>Add YAML scenario files to the scenarios directory</p>
        </div>"#
            .to_string()
    } else {
        scenarios.iter().map(|s| {
            format!(
                r#"<div class="scenario-card" onclick="window.location='/scenarios/{}'">
                    <h3 class="scenario-name">{}</h3>
                    <p class="text-secondary">{}</p>
                    <div class="scenario-meta">
                        <span class="scenario-meta-item">⏱️ {}</span>
                        <span class="scenario-meta-item">📊 {} phases</span>
                    </div>
                    <div class="mt-2">
                        <button class="btn btn-primary btn-sm" onclick="event.stopPropagation(); runScenario('{}')">
                            ▶️ Run
                        </button>
                    </div>
                </div>"#,
                s.file_name,
                s.name,
                s.description.as_deref().unwrap_or("No description"),
                s.duration,
                s.phase_count,
                s.file_name
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Scenarios</h1>
            <p class="page-subtitle">Browse and run chaos test scenarios</p>
        </div>
        
        <div class="grid grid-3">
            {scenarios_html}
        </div>"#,
        scenarios_html = scenarios_html
    );

    base_layout("Scenarios", &content)
}

/// Scenario info for display
#[derive(Clone, Debug)]
pub struct ScenarioInfo {
    pub file_name: String,
    pub name: String,
    pub description: Option<String>,
    pub duration: String,
    pub phase_count: usize,
}

/// Generate the scenario detail page
pub fn scenario_detail_page(
    scenario: &ScenarioInfo,
    yaml_content: &str,
    phases: &[PhaseInfo],
) -> String {
    let phases_html: String = phases
        .iter()
        .map(|p| {
            let injections: String = p
                .injections
                .iter()
                .map(|i| format!(r#"<span class="badge badge-info">{}</span>"#, i))
                .collect::<Vec<_>>()
                .join(" ");

            format!(
                r#"<div class="timeline-item">
                <h4 class="timeline-title">{}</h4>
                <p class="timeline-content">Duration: {}</p>
                <div class="mt-1">{}</div>
            </div>"#,
                p.name, p.duration, injections
            )
        })
        .collect();

    let content = format!(
        r#"<div class="page-header">
            <div class="flex justify-between items-center">
                <div>
                    <h1 class="page-title">{name}</h1>
                    <p class="page-subtitle">{description}</p>
                </div>
                <button class="btn btn-primary btn-lg" onclick="runScenario('{file_name}')">
                    ▶️ Run Test
                </button>
            </div>
        </div>
        
        <div class="grid grid-2">
            <div class="card">
                <div class="card-header">
                    <h2 class="card-title">Phases</h2>
                </div>
                <div class="card-body">
                    <div class="timeline">
                        {phases_html}
                    </div>
                </div>
            </div>
            
            <div class="card">
                <div class="card-header">
                    <h2 class="card-title">Scenario Configuration</h2>
                </div>
                <div class="card-body">
                    <div class="code-block">
                        <code><pre>{yaml_content}</pre></code>
                    </div>
                </div>
            </div>
        </div>"#,
        name = scenario.name,
        description = scenario.description.as_deref().unwrap_or("No description"),
        file_name = scenario.file_name,
        phases_html = phases_html,
        yaml_content = yaml_content.replace('<', "&lt;").replace('>', "&gt;")
    );

    base_layout(&scenario.name, &content)
}

/// Phase info for display
#[derive(Clone, Debug)]
pub struct PhaseInfo {
    pub name: String,
    pub duration: String,
    pub injections: Vec<String>,
}

/// Generate the run test page
pub fn run_page(scenarios: &[ScenarioInfo], status: &crate::state::TestStatus) -> String {
    let options: String = scenarios
        .iter()
        .map(|s| {
            format!(
                r#"<option value="{}">{} ({})</option>"#,
                s.file_name, s.name, s.duration
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let status_section = if status.is_running {
        format!(
            r#"<div class="alert alert-info mb-4">
                <span class="live-indicator">
                    <span class="live-dot"></span>
                    Test in progress
                </span>
            </div>
            
            <div class="card">
                <div class="card-header">
                    <h2 class="card-title">Current Test: {}</h2>
                    <button class="btn btn-danger" onclick="stopTest()">Stop Test</button>
                </div>
                <div class="card-body">
                    <p class="mb-2"><strong>Phase:</strong> <span id="current-phase">{}</span></p>
                    <div class="progress-container">
                        <div class="progress-bar" id="progress-bar" style="width: {}%"></div>
                    </div>
                    <p class="text-secondary" id="progress-text">
                        {:.1}% complete - {} / {} seconds elapsed
                    </p>
                </div>
            </div>"#,
            status.scenario_name.as_deref().unwrap_or("Unknown"),
            status.current_phase.as_deref().unwrap_or("Starting..."),
            status.progress_percent,
            status.progress_percent,
            status.elapsed_seconds,
            status.total_seconds
        )
    } else {
        format!(
            r#"<div class="card">
                <div class="card-header">
                    <h2 class="card-title">Run New Test</h2>
                </div>
                <div class="card-body">
                    <form id="run-form" onsubmit="event.preventDefault(); runScenario(document.getElementById('scenario-select').value);">
                        <div class="form-group">
                            <label class="form-label" for="scenario-select">Select Scenario</label>
                            <select id="scenario-select" class="form-select">
                                {options}
                            </select>
                        </div>
                        <button type="submit" class="btn btn-primary btn-lg">
                            ▶️ Start Test
                        </button>
                    </form>
                </div>
            </div>"#,
            options = options
        )
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Run Chaos Test</h1>
            <p class="page-subtitle">Execute chaos engineering scenarios</p>
        </div>
        
        {status_section}"#,
        status_section = status_section
    );

    base_layout("Run Test", &content)
}

/// Generate the results list page
pub fn results_page(results: &[crate::state::ResultSummary]) -> String {
    let results_html = if results.is_empty() {
        r#"<div class="empty-state">
            <div class="empty-state-icon">📊</div>
            <p class="empty-state-title">No test results yet</p>
            <p>Run a chaos test to see results here</p>
        </div>"#
            .to_string()
    } else {
        let rows: String = results
            .iter()
            .map(|r| {
                let success_class = if r.success_rate >= 0.9 {
                    "badge-success"
                } else if r.success_rate >= 0.7 {
                    "badge-warning"
                } else {
                    "badge-danger"
                };
                format!(
                    r#"<tr onclick="window.location='/results/{}'" style="cursor: pointer;">
                    <td><strong>{}</strong></td>
                    <td><span class="badge {}">{:.1}%</span></td>
                    <td>{}s</td>
                    <td>{}</td>
                    <td>
                        <a href="/results/{}" class="btn btn-secondary btn-sm">View</a>
                    </td>
                </tr>"#,
                    r.id,
                    r.scenario_name,
                    success_class,
                    r.success_rate * 100.0,
                    r.total_duration_secs,
                    r.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    r.id
                )
            })
            .collect();

        format!(
            r#"<div class="table-container">
                <table>
                    <thead>
                        <tr>
                            <th>Scenario</th>
                            <th>Success Rate</th>
                            <th>Duration</th>
                            <th>Timestamp</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>{}</tbody>
                </table>
            </div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Test Results</h1>
            <p class="page-subtitle">View historical chaos test results</p>
        </div>
        
        {results_html}"#,
        results_html = results_html
    );

    base_layout("Results", &content)
}

/// Generate the result detail page
pub fn result_detail_page(result: &chaos_scenarios::runner::ScenarioResult) -> String {
    let success_class = if result.success_rate() >= 0.9 {
        "text-success"
    } else if result.success_rate() >= 0.7 {
        "text-warning"
    } else {
        "text-danger"
    };

    let phases_html: String = result
        .phase_results
        .iter()
        .map(|p| {
            format!(
                r#"<div class="timeline-item completed">
                <h4 class="timeline-title">{}</h4>
                <p class="timeline-content">Duration: {:?} | Injections: {}</p>
            </div>"#,
                p.name, p.duration, p.injection_count
            )
        })
        .collect();

    let content = format!(
        r#"<div class="page-header">
            <div class="flex justify-between items-center">
                <div>
                    <h1 class="page-title">{scenario_name}</h1>
                    <p class="page-subtitle">Test completed</p>
                </div>
                <a href="/results" class="btn btn-secondary">← Back to Results</a>
            </div>
        </div>
        
        <div class="grid grid-4 mb-4">
            <div class="stat-card">
                <span class="stat-icon">⏱️</span>
                <span class="stat-value">{duration}s</span>
                <span class="stat-label">Duration</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon">📊</span>
                <span class="stat-value {success_class}">{success_rate:.1}%</span>
                <span class="stat-label">Success Rate</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon">⚡</span>
                <span class="stat-value">{injections}</span>
                <span class="stat-label">Injections</span>
            </div>
            <div class="stat-card">
                <span class="stat-icon">📋</span>
                <span class="stat-value">{phases}</span>
                <span class="stat-label">Phases</span>
            </div>
        </div>
        
        <div class="card">
            <div class="card-header">
                <h2 class="card-title">Phase Timeline</h2>
            </div>
            <div class="card-body">
                <div class="timeline">
                    {phases_html}
                </div>
            </div>
        </div>"#,
        scenario_name = result.scenario_name,
        duration = result.total_duration.as_secs(),
        success_class = success_class,
        success_rate = result.success_rate() * 100.0,
        injections = result.total_injections,
        phases = result.phase_results.len(),
        phases_html = phases_html
    );

    base_layout(&result.scenario_name, &content)
}

/// Generate error page
pub fn error_page(title: &str, message: &str) -> String {
    let content = format!(
        r#"<div class="empty-state">
            <div class="empty-state-icon">❌</div>
            <p class="empty-state-title">{}</p>
            <p>{}</p>
            <a href="/" class="btn btn-primary mt-3">← Back to Dashboard</a>
        </div>"#,
        title, message
    );

    base_layout("Error", &content)
}

/// Generate load test page
pub fn load_test_page(
    targets: &[crate::state::CustomTarget],
    is_running: bool,
    metrics: &crate::load_test::LoadTestMetrics,
) -> String {
    let target_options: String = targets
        .iter()
        .map(|t| {
            format!(
                r#"<option value="{}">{} ({})</option>"#,
                t.url, t.name, t.target_type
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let status_section = if is_running {
        format!(
            r#"<div class="alert alert-info mb-4">
                <span class="live-indicator">
                    <span class="live-dot"></span>
                    Load test in progress
                </span>
            </div>
            
            <div class="grid grid-4 mb-4">
                <div class="stat-card">
                    <span class="stat-icon">📊</span>
                    <span class="stat-value">{}</span>
                    <span class="stat-label">Total Requests</span>
                </div>
                <div class="stat-card">
                    <span class="stat-icon">✅</span>
                    <span class="stat-value text-success">{}</span>
                    <span class="stat-label">Successful</span>
                </div>
                <div class="stat-card">
                    <span class="stat-icon">❌</span>
                    <span class="stat-value text-danger">{}</span>
                    <span class="stat-label">Failed</span>
                </div>
                <div class="stat-card">
                    <span class="stat-icon">⚡</span>
                    <span class="stat-value">{:.1}</span>
                    <span class="stat-label">Req/sec</span>
                </div>
            </div>
            
            <div class="grid grid-3 mb-4">
                <div class="stat-card">
                    <span class="stat-icon">🕐</span>
                    <span class="stat-value">{:.1}ms</span>
                    <span class="stat-label">Avg Latency</span>
                </div>
                <div class="stat-card">
                    <span class="stat-icon">📈</span>
                    <span class="stat-value">{:.1}ms</span>
                    <span class="stat-label">P95 Latency</span>
                </div>
                <div class="stat-card">
                    <span class="stat-icon">📉</span>
                    <span class="stat-value">{:.1}ms</span>
                    <span class="stat-label">P99 Latency</span>
                </div>
            </div>
            
            <button class="btn btn-danger btn-lg" onclick="stopLoadTest()">⏹️ Stop Test</button>"#,
            metrics.total_requests,
            metrics.successful_requests,
            metrics.failed_requests,
            metrics.requests_per_second,
            metrics.avg_latency_ms,
            metrics.p95_latency_ms,
            metrics.p99_latency_ms
        )
    } else {
        format!(
            r#"<div class="card">
                <div class="card-header">
                    <h2 class="card-title">Configure Load Test</h2>
                </div>
                <div class="card-body">
                    <form id="load-test-form" onsubmit="event.preventDefault(); startLoadTest();">
                        <div class="grid grid-2">
                            <div class="form-group">
                                <label class="form-label">Test Name</label>
                                <input type="text" id="test-name" class="form-input" value="My Load Test" required>
                            </div>
                            <div class="form-group">
                                <label class="form-label">Target Type</label>
                                <select id="target-type" class="form-select">
                                    <option value="http">HTTP/HTTPS API</option>
                                    <option value="websocket">WebSocket</option>
                                    <option value="tcp">TCP Socket</option>
                                    <option value="grpc">gRPC</option>
                                    <option value="hls">HLS Stream</option>
                                    <option value="rtmp">RTMP Stream</option>
                                </select>
                            </div>
                        </div>
                        
                        <div class="form-group">
                            <label class="form-label">Target URL</label>
                            <input type="text" id="target-url" class="form-input" placeholder="http://localhost:3000/api/endpoint" required>
                            {target_select}
                        </div>
                        
                        <div class="form-group">
                            <label class="form-label">HTTP Method</label>
                            <select id="http-method" class="form-select">
                                <option value="GET">GET</option>
                                <option value="POST">POST</option>
                                <option value="PUT">PUT</option>
                                <option value="DELETE">DELETE</option>
                                <option value="PATCH">PATCH</option>
                            </select>
                        </div>
                        
                        <div class="form-group">
                            <label class="form-label">Request Body (JSON)</label>
                            <textarea id="request-body" class="form-textarea" placeholder='{{"key": "value"}}'></textarea>
                        </div>
                        
                        <div class="grid grid-4">
                            <div class="form-group">
                                <label class="form-label">Concurrent Users</label>
                                <input type="number" id="concurrent-users" class="form-input" value="10" min="1" max="1000">
                            </div>
                            <div class="form-group">
                                <label class="form-label">Requests/Second</label>
                                <input type="number" id="rps" class="form-input" value="100" min="1" max="10000">
                            </div>
                            <div class="form-group">
                                <label class="form-label">Duration (seconds)</label>
                                <input type="number" id="duration" class="form-input" value="60" min="1" max="3600">
                            </div>
                            <div class="form-group">
                                <label class="form-label">Ramp-up (seconds)</label>
                                <input type="number" id="ramp-up" class="form-input" value="10" min="0" max="300">
                            </div>
                        </div>
                        
                        <button type="submit" class="btn btn-primary btn-lg">🚀 Start Load Test</button>
                    </form>
                </div>
            </div>"#,
            target_select = if !target_options.is_empty() {
                format!(
                    r#"<p class="text-secondary mt-1">Or select from saved targets:
                    <select id="saved-targets" class="form-select mt-1" onchange="document.getElementById('target-url').value = this.value">
                        <option value="">-- Select saved target --</option>
                        {}
                    </select>
                </p>"#,
                    target_options
                )
            } else {
                String::new()
            }
        )
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Load Testing</h1>
            <p class="page-subtitle">Stress test your APIs, services, and video streams</p>
        </div>
        
        {status_section}
        
        <div class="card mt-4">
            <div class="card-header">
                <h2 class="card-title">Supported Target Types</h2>
            </div>
            <div class="card-body">
                <div class="grid grid-3">
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-info">HTTP/HTTPS</span>
                        <span class="text-secondary">REST APIs, Web Services</span>
                    </div>
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-info">WebSocket</span>
                        <span class="text-secondary">Real-time connections</span>
                    </div>
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-info">TCP</span>
                        <span class="text-secondary">Raw socket connections</span>
                    </div>
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-info">gRPC</span>
                        <span class="text-secondary">Protocol buffer services</span>
                    </div>
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-success">HLS</span>
                        <span class="text-secondary">Video streaming (HTTP Live)</span>
                    </div>
                    <div class="flex gap-2 items-center">
                        <span class="badge badge-success">RTMP</span>
                        <span class="text-secondary">Live video streams</span>
                    </div>
                </div>
            </div>
        </div>"#,
        status_section = status_section
    );

    base_layout("Load Testing", &content)
}

/// Generate targets management page
pub fn targets_page(targets: &[crate::state::CustomTarget]) -> String {
    let targets_html = if targets.is_empty() {
        r#"<div class="empty-state">
            <div class="empty-state-icon">🎯</div>
            <p class="empty-state-title">No targets configured</p>
            <p>Add your first target to get started</p>
        </div>"#
            .to_string()
    } else {
        let rows: String = targets.iter().map(|t| {
            format!(
                r#"<tr>
                    <td><strong>{}</strong></td>
                    <td><span class="badge badge-info">{}</span></td>
                    <td><code>{}</code></td>
                    <td>{}</td>
                    <td>
                        <button class="btn btn-danger btn-sm" onclick="deleteTarget('{}')">Delete</button>
                    </td>
                </tr>"#,
                t.name,
                t.target_type,
                t.url,
                t.description.as_deref().unwrap_or("-"),
                t.id
            )
        }).collect();

        format!(
            r#"<div class="table-container">
                <table>
                    <thead>
                        <tr>
                            <th>Name</th>
                            <th>Type</th>
                            <th>URL</th>
                            <th>Description</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>{}</tbody>
                </table>
            </div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header">
            <h1 class="page-title">Targets</h1>
            <p class="page-subtitle">Manage your stress testing targets</p>
        </div>
        
        <div class="card mb-4">
            <div class="card-header">
                <h2 class="card-title">Add New Target</h2>
            </div>
            <div class="card-body">
                <form id="add-target-form" onsubmit="event.preventDefault(); addTarget();">
                    <div class="grid grid-2">
                        <div class="form-group">
                            <label class="form-label">Target Name</label>
                            <input type="text" id="target-name" class="form-input" placeholder="My API Server" required>
                        </div>
                        <div class="form-group">
                            <label class="form-label">Target Type</label>
                            <select id="new-target-type" class="form-select">
                                <option value="http">HTTP/HTTPS API</option>
                                <option value="websocket">WebSocket</option>
                                <option value="tcp">TCP Socket</option>
                                <option value="grpc">gRPC Service</option>
                                <option value="hls">HLS Video Stream</option>
                                <option value="rtmp">RTMP Stream</option>
                                <option value="pipeline">Data Pipeline</option>
                            </select>
                        </div>
                    </div>
                    <div class="form-group">
                        <label class="form-label">URL/Endpoint</label>
                        <input type="text" id="new-target-url" class="form-input" placeholder="http://localhost:3000" required>
                    </div>
                    <div class="form-group">
                        <label class="form-label">Description (optional)</label>
                        <input type="text" id="new-target-desc" class="form-input" placeholder="Production API endpoint">
                    </div>
                    <button type="submit" class="btn btn-primary">➕ Add Target</button>
                </form>
            </div>
        </div>
        
        <div class="card">
            <div class="card-header">
                <h2 class="card-title">Saved Targets</h2>
            </div>
            <div class="card-body">
                {targets_html}
            </div>
        </div>"#,
        targets_html = targets_html
    );

    base_layout("Targets", &content)
}
