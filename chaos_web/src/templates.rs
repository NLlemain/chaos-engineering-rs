//! HTML templates with embedded CSS for the web dashboard.

/// Render the shared page shell.
pub fn base_layout(title: &str, content: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - Chaos Dashboard</title>
    <style>{CSS}</style>
</head>
<body>
    <div class="backdrop backdrop-a"></div>
    <div class="backdrop backdrop-b"></div>
    <div class="shell">
        <header class="topbar">
            <a class="brand" href="/">
                <span class="brand-mark">🦀</span>
                <span class="brand-copy"><strong>Chaos Dashboard</strong><span>Control center for resilience testing</span></span>
            </a>
            <nav class="nav">
                <a href="/" class="nav-link">Dashboard</a>
                <a href="/scenarios" class="nav-link">Scenarios</a>
                <a href="/run" class="nav-link">Run Test</a>
                <a href="/load-test" class="nav-link">Load Test</a>
                <a href="/targets" class="nav-link">Targets</a>
                <a href="/results" class="nav-link">Results</a>
            </nav>
        </header>
        <main class="main">
            <div class="content">{content}</div>
        </main>
        <footer class="footer">Chaos Engineering Framework &copy; 2025</footer>
    </div>
    <script>{JS}</script>
</body>
</html>"##,
        title = title,
        content = content,
        CSS = CSS_STYLES,
        JS = JS_SCRIPTS
    )
}

const CSS_STYLES: &str = r##"
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }

:root {
    --bg: #07111f;
    --panel: rgba(14, 22, 37, 0.86);
    --line: rgba(154, 171, 196, 0.16);
    --text: #f6fbff;
    --muted: #a9bdd6;
    --soft: #7f94b2;
    --blue: #6db7ff;
    --green: #58d59b;
    --red: #ff7d94;
    --yellow: #ffd56b;
    --purple: #b79dff;
    --shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
}

body {
    margin: 0;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    color: var(--text);
    background: var(--bg);
    font-family: "Aptos", "Segoe UI Variable", "Trebuchet MS", sans-serif;
    overflow-x: hidden;
}

body::before, body::after {
    content: '';
    position: fixed;
    inset: 0;
    pointer-events: none;
}

body::before {
    background:
        radial-gradient(circle at top left, rgba(109, 183, 255, 0.18), transparent 30%),
        radial-gradient(circle at top right, rgba(183, 157, 255, 0.16), transparent 28%),
        radial-gradient(circle at bottom center, rgba(88, 213, 155, 0.08), transparent 28%);
    z-index: -3;
}

body::after {
    background-image: linear-gradient(rgba(154, 171, 196, 0.04) 1px, transparent 1px), linear-gradient(90deg, rgba(154, 171, 196, 0.04) 1px, transparent 1px);
    background-size: 48px 48px;
    mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.8), transparent 80%);
    z-index: -2;
}

.backdrop {
    position: fixed;
    border-radius: 999px;
    filter: blur(70px);
    opacity: 0.5;
    z-index: -1;
}

.backdrop-a { width: 320px; height: 320px; top: -100px; left: -60px; background: rgba(109, 183, 255, 0.2); }
.backdrop-b { width: 260px; height: 260px; top: 220px; right: -80px; background: rgba(183, 157, 255, 0.16); }

.shell { flex: 1; display: flex; flex-direction: column; }

.topbar {
    position: sticky; top: 0; z-index: 100;
    display: flex; align-items: center; justify-content: space-between; gap: 1rem;
    padding: 1rem 1.5rem; backdrop-filter: blur(22px);
    background: rgba(7, 17, 31, 0.74); border-bottom: 1px solid rgba(154, 171, 196, 0.12);
}

.brand { display: flex; align-items: center; gap: 0.9rem; text-decoration: none; color: inherit; }
.brand-mark {
    width: 2.5rem; height: 2.5rem; display: grid; place-items: center; flex: 0 0 auto;
    border-radius: 0.95rem; background: linear-gradient(135deg, rgba(109, 183, 255, 0.22), rgba(183, 157, 255, 0.18));
    border: 1px solid rgba(154, 171, 196, 0.15); box-shadow: var(--shadow);
}
.brand-copy { display: flex; flex-direction: column; line-height: 1.1; }
.brand-copy strong { font-size: 1rem; letter-spacing: 0.01em; }
.brand-copy span { font-size: 0.8rem; color: var(--soft); }
.nav { display: flex; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
.nav-link {
    text-decoration: none; color: var(--muted);
    padding: 0.6rem 0.95rem; border-radius: 999px; border: 1px solid transparent;
    transition: 0.18s ease;
}
.nav-link:hover, .nav-link.active { color: var(--text); background: rgba(154, 171, 196, 0.08); border-color: rgba(154, 171, 196, 0.12); }

.main { flex: 1; padding: clamp(1.25rem, 2vw, 2rem); }
.content { max-width: 1440px; margin: 0 auto; display: flex; flex-direction: column; gap: 1.1rem; }

.page-header {
    display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem;
    padding: 1.4rem 1.5rem; border-radius: 24px; border: 1px solid rgba(154, 171, 196, 0.16);
    background: linear-gradient(135deg, rgba(14, 22, 37, 0.96), rgba(19, 30, 49, 0.78)); box-shadow: var(--shadow);
}
.hero-kicker {
    display: inline-flex; align-items: center; gap: 0.45rem; margin-bottom: 0.7rem;
    padding: 0.35rem 0.75rem; border-radius: 999px; font-size: 0.74rem; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase;
    color: var(--text); background: rgba(154, 171, 196, 0.09); border: 1px solid rgba(154, 171, 196, 0.12);
}
.page-title { margin: 0 0 0.5rem; font-size: clamp(1.8rem, 3vw, 2.7rem); line-height: 1.05; letter-spacing: -0.03em; }
.page-subtitle { margin: 0; color: var(--muted); max-width: 68ch; }
.hero-actions { display: flex; gap: 0.75rem; flex-wrap: wrap; }

.card, .scenario-card, .stat-card, .surface, .callout {
    background: linear-gradient(180deg, rgba(14, 22, 37, 0.98), rgba(19, 30, 49, 0.9));
    border: 1px solid rgba(154, 171, 196, 0.14); border-radius: 22px; box-shadow: 0 1px 0 rgba(255,255,255,0.02) inset;
}
.card, .scenario-card, .stat-card { padding: 1.35rem; }
.card, .stat-card { transition: transform 0.2s ease, box-shadow 0.2s ease, border-color 0.2s ease; }
.card:hover, .stat-card:hover, .scenario-card:hover { transform: translateY(-2px); box-shadow: var(--shadow); border-color: rgba(109, 183, 255, 0.26); }
.card-header { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin-bottom: 1rem; padding-bottom: 1rem; border-bottom: 1px solid rgba(154, 171, 196, 0.12); }
.card-title, .section-title { margin: 0; color: var(--text); font-weight: 600; }
.card-title { font-size: 1.05rem; }
.card-body { color: var(--muted); }

.grid { display: grid; gap: 1.1rem; }
.grid-2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.grid-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.grid-4 { grid-template-columns: repeat(4, minmax(0, 1fr)); }

@media (max-width: 1024px) {
    .grid-4, .grid-3 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .page-header, .card-header { align-items: flex-start; flex-direction: column; }
}

@media (max-width: 640px) {
    .grid-2, .grid-3, .grid-4 { grid-template-columns: 1fr; }
    .topbar { flex-direction: column; align-items: flex-start; }
}

.stat-card { position: relative; overflow: hidden; min-height: 122px; display: flex; flex-direction: column; }
.stat-card::before { content: ''; position: absolute; inset: 0 auto auto 0; width: 100%; height: 4px; background: linear-gradient(90deg, var(--blue), var(--purple), #5de3d6); }
.stat-icon { width: 2.35rem; height: 2.35rem; display: grid; place-items: center; border-radius: 0.9rem; background: rgba(154, 171, 196, 0.08); margin-bottom: 0.65rem; }
.stat-value { font-size: 2.1rem; font-weight: 700; color: var(--text); margin-bottom: 0.25rem; letter-spacing: -0.03em; }
.stat-label { color: var(--muted); font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.05em; }

.btn {
    display: inline-flex; align-items: center; justify-content: center; gap: 0.5rem; cursor: pointer; text-decoration: none;
    padding: 0.75rem 1.5rem; border-radius: 999px; border: none; color: var(--text);
    transition: transform 0.18s ease, box-shadow 0.18s ease, background-color 0.18s ease;
}
.btn:hover { transform: translateY(-1px); }
.btn-primary { background: linear-gradient(135deg, var(--blue), #8b8dff); box-shadow: 0 16px 28px rgba(109, 183, 255, 0.18); }
.btn-secondary { background: rgba(154, 171, 196, 0.08); border: 1px solid rgba(154, 171, 196, 0.14); }
.btn-success { background: linear-gradient(135deg, var(--green), #34c6a4); }
.btn-danger { background: linear-gradient(135deg, var(--red), #ff5d71); }
.btn-sm { padding: 0.5rem 0.95rem; font-size: 0.78rem; }
.btn-lg { padding: 1rem 2rem; font-size: 1rem; }

.table-container { overflow-x: auto; border-radius: 20px; border: 1px solid rgba(154, 171, 196, 0.14); box-shadow: var(--shadow); background: rgba(14, 22, 37, 0.88); }
table { width: 100%; border-collapse: collapse; }
th, td { padding: 1rem; text-align: left; border-bottom: 1px solid rgba(154, 171, 196, 0.12); }
th { background: rgba(154, 171, 196, 0.06); color: var(--text); text-transform: uppercase; font-size: 0.75rem; letter-spacing: 0.05em; }
td { color: var(--muted); }
tr:hover td { background: rgba(154, 171, 196, 0.05); }

.badge { display: inline-flex; align-items: center; padding: 0.25rem 0.75rem; border-radius: 999px; font-size: 0.75rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; border: 1px solid transparent; }
.badge-info { background: rgba(109, 183, 255, 0.14); color: var(--blue); border-color: rgba(109, 183, 255, 0.22); }
.badge-success { background: rgba(88, 213, 155, 0.14); color: var(--green); border-color: rgba(88, 213, 155, 0.22); }
.badge-warning { background: rgba(255, 213, 107, 0.14); color: var(--yellow); border-color: rgba(255, 213, 107, 0.22); }
.badge-danger { background: rgba(255, 125, 148, 0.14); color: var(--red); border-color: rgba(255, 125, 148, 0.22); }
.badge-neutral { background: rgba(154, 171, 196, 0.08); color: var(--muted); border-color: rgba(154, 171, 196, 0.12); }

.progress-container { height: 8px; overflow: hidden; border-radius: 999px; background: rgba(154, 171, 196, 0.08); margin: 1rem 0; }
.progress-bar { height: 100%; border-radius: 999px; background: linear-gradient(90deg, var(--blue), var(--purple)); transition: width 0.3s ease; }

.form-group { margin-bottom: 1.25rem; }
.form-label { display: block; margin-bottom: 0.5rem; color: var(--text); font-weight: 500; }
.form-input, .form-select, .form-textarea {
    width: 100%; padding: 0.75rem 1rem; color: var(--text); font-size: 1rem; border-radius: 14px;
    border: 1px solid rgba(154, 171, 196, 0.14); background: rgba(154, 171, 196, 0.06);
}
.form-input:focus, .form-select:focus, .form-textarea:focus { outline: none; border-color: rgba(109, 183, 255, 0.65); box-shadow: 0 0 0 4px rgba(109, 183, 255, 0.12); }
.form-textarea { min-height: 150px; resize: vertical; }

.code-block { background: rgba(6, 11, 20, 0.92); border: 1px solid rgba(154, 171, 196, 0.16); border-radius: 18px; padding: 1rem; overflow-x: auto; font-family: Consolas, Monaco, 'Courier New', monospace; font-size: 0.875rem; }
.scenario-card { position: relative; cursor: pointer; }
.scenario-name { margin: 0 0 0.5rem; font-size: 1.08rem; color: var(--text); }
.scenario-meta { display: flex; gap: 1rem; margin-top: 1rem; color: var(--soft); font-size: 0.875rem; flex-wrap: wrap; }
.scenario-meta-item { display: inline-flex; align-items: center; gap: 0.3rem; }

.footer { padding: 1.25rem 1.5rem 1.5rem; text-align: center; color: var(--soft); border-top: 1px solid rgba(154, 171, 196, 0.12); background: rgba(7, 17, 31, 0.72); backdrop-filter: blur(18px); }
.empty-state { text-align: center; padding: 4.5rem 2rem; color: var(--soft); }
.empty-state-icon { font-size: 4rem; margin-bottom: 1rem; opacity: 0.5; }
.empty-state-title { font-size: 1.25rem; font-weight: 600; color: var(--muted); margin-bottom: 0.5rem; }

.timeline { position: relative; padding-left: 2rem; }
.timeline::before { content: ''; position: absolute; left: 0.5rem; top: 0; bottom: 0; width: 2px; background: linear-gradient(180deg, rgba(154, 171, 196, 0.18), rgba(154, 171, 196, 0.04)); }
.timeline-item { position: relative; padding-bottom: 1.5rem; }
.timeline-item::before { content: ''; position: absolute; left: -1.5rem; top: 0.5rem; width: 12px; height: 12px; border-radius: 50%; background: rgba(154, 171, 196, 0.22); border: 2px solid rgba(154, 171, 196, 0.35); }
.timeline-item.active::before { background: var(--blue); border-color: var(--blue); }
.timeline-item.completed::before { background: var(--green); border-color: var(--green); }
.timeline-title { margin: 0 0 0.25rem; color: var(--text); font-weight: 600; }
.timeline-content { color: var(--muted); font-size: 0.875rem; }

.alert { padding: 1rem 1.25rem; border-radius: 18px; margin-bottom: 1rem; display: flex; gap: 0.75rem; align-items: flex-start; box-shadow: var(--shadow); }
.alert-info { background: rgba(109, 183, 255, 0.1); border: 1px solid rgba(109, 183, 255, 0.28); color: var(--blue); }
.alert-success { background: rgba(88, 213, 155, 0.1); border: 1px solid rgba(88, 213, 155, 0.28); color: var(--green); }
.alert-warning { background: rgba(255, 213, 107, 0.1); border: 1px solid rgba(255, 213, 107, 0.28); color: var(--yellow); }
.alert-danger { background: rgba(255, 125, 148, 0.1); border: 1px solid rgba(255, 125, 148, 0.28); color: var(--red); }

.live-indicator { display: inline-flex; align-items: center; gap: 0.5rem; }
.live-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--green); animation: pulse 2s infinite; }
.spinner { width: 40px; height: 40px; border: 3px solid rgba(154, 171, 196, 0.12); border-top-color: var(--blue); border-radius: 50%; animation: spin 1s linear infinite; }

.hero-strip { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 1rem; align-items: center; padding: 1rem 1.25rem; border-radius: 18px; border: 1px solid rgba(154, 171, 196, 0.12); background: linear-gradient(135deg, rgba(109, 183, 255, 0.08), rgba(183, 157, 255, 0.08)); }
.section-heading { display: flex; justify-content: space-between; gap: 1rem; align-items: flex-end; margin-bottom: 0.9rem; }
.section-caption { color: var(--soft); font-size: 0.9rem; }
.surface { padding: 1rem; }
.callout { padding: 1rem 1.15rem; }

.text-center { text-align: center; }
.text-right { text-align: right; }
.text-primary { color: var(--text); }
.text-secondary { color: var(--muted); }
.text-muted { color: var(--soft); }
.text-success { color: var(--green); }
.text-danger { color: var(--red); }
.text-warning { color: var(--yellow); }
.text-info { color: var(--blue); }

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

@keyframes spin { to { transform: rotate(360deg); } }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }

@media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
        scroll-behavior: auto !important;
    }
}
"##;

/// JavaScript for interactivity
const JS_SCRIPTS: &str = r##"
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
        statusEl.innerHTML = status.is_running
            ? `<span class="live-indicator"><span class="live-dot"></span> Running: ${status.scenario_name || 'Unknown'}</span>`
            : '<span class="badge badge-neutral">Idle</span>';
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
        await fetch('/api/stop', { method: 'POST' });
        checkStatus();
    } catch (error) {
        alert('Failed to stop test: ' + error.message);
    }
}

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
        if (data.is_running) location.reload();
    } catch (error) {
        console.error('Failed to check load test status:', error);
    }
}

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

        if (response.ok) location.reload();
        else alert('Failed to add target');
    } catch (error) {
        alert('Failed to add target: ' + error.message);
    }
}

async function deleteTarget(id) {
    if (!confirm('Delete this target?')) return;
    try {
        const response = await fetch('/api/targets/' + id, { method: 'DELETE' });
        if (response.ok) location.reload();
    } catch (error) {
        alert('Failed to delete: ' + error.message);
    }
}

document.addEventListener('DOMContentLoaded', function() {
    checkStatus();

    if (window.location.pathname === '/load-test') {
        loadTestInterval = setInterval(checkLoadTestStatus, 2000);
    }

    const currentPath = window.location.pathname;
    document.querySelectorAll('.nav-link').forEach(link => {
        if (link.getAttribute('href') === currentPath) link.classList.add('active');
    });
});
"##;

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Scenario info for display.
#[derive(Clone, Debug)]
pub struct ScenarioInfo {
    pub file_name: String,
    pub name: String,
    pub description: Option<String>,
    pub duration: String,
    pub phase_count: usize,
}

/// Phase info for display.
#[derive(Clone, Debug)]
pub struct PhaseInfo {
    pub name: String,
    pub duration: String,
    pub injections: Vec<String>,
}

pub fn dashboard_page(
    total_scenarios: usize,
    total_results: usize,
    recent_results: &[crate::state::ResultSummary],
    status: &crate::state::TestStatus,
) -> String {
    let status_html = if status.is_running {
        format!(
            r#"<div class="card mb-4"><div class="card-header"><h2 class="card-title">🔄 Test In Progress</h2><button class="btn btn-danger btn-sm" onclick="stopTest()">Stop Test</button></div><div class="card-body"><p class="mb-2"><strong>Scenario:</strong> {}</p><p class="mb-2"><strong>Phase:</strong> {}</p><div class="progress-container"><div class="progress-bar" id="progress-bar" style="width: {}%"></div></div><p class="text-secondary" id="progress-text">{:.1}% complete - {} / {} seconds</p></div></div>"#,
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

    let results_html = if recent_results.is_empty() {
        r#"<div class="empty-state"><div class="empty-state-icon">📊</div><p class="empty-state-title">No test results yet</p><p>Run a chaos test to see results here.</p></div>"#.to_string()
    } else {
        let rows: String = recent_results.iter().take(5).map(|r| {
            let success_class = if r.success_rate >= 0.9 { "text-success" } else if r.success_rate >= 0.7 { "text-warning" } else { "text-danger" };
            format!(
                r#"<tr onclick="window.location='/results/{}'"><td>{}</td><td class="{}">{:.1}%</td><td>{}s</td><td>{}</td></tr>"#,
                r.id,
                escape_html(&r.scenario_name),
                success_class,
                r.success_rate * 100.0,
                r.total_duration_secs,
                r.timestamp.format("%Y-%m-%d %H:%M")
            )
        }).collect();

        format!(
            r#"<div class="table-container"><table><thead><tr><th>Scenario</th><th>Success Rate</th><th>Duration</th><th>Time</th></tr></thead><tbody>{}</tbody></table></div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Live chaos control</div><h1 class="page-title">Dashboard</h1><p class="page-subtitle">Monitor live tests, inspect recent outcomes, and launch new experiments from one place.</p></div><div class="hero-actions"><a href="/scenarios" class="btn btn-secondary">📋 Browse Scenarios</a><a href="/run" class="btn btn-primary">▶️ Run New Test</a></div></div>{status_html}<div class="grid grid-4 mb-4"><div class="stat-card"><span class="stat-icon">📋</span><span class="stat-value">{total_scenarios}</span><span class="stat-label">Scenarios</span></div><div class="stat-card"><span class="stat-icon">📊</span><span class="stat-value">{total_results}</span><span class="stat-label">Test Results</span></div><div class="stat-card"><span class="stat-icon">⚡</span><span class="stat-value">7</span><span class="stat-label">Injector Types</span></div><div class="stat-card"><span class="stat-icon" id="test-status">{status_badge}</span><span class="stat-value">&nbsp;</span><span class="stat-label">Status</span></div></div><div class="grid grid-2"><div class="card"><div class="card-header"><h2 class="card-title">Recent Results</h2><a href="/results" class="btn btn-secondary btn-sm">View All</a></div><div class="card-body">{results_html}</div></div><div class="card"><div class="card-header"><h2 class="card-title">Quick Actions</h2></div><div class="card-body"><div class="callout mb-2"><p class="text-primary mb-1">Keep the dashboard open while tests run.</p><p class="text-secondary">The page updates live when chaos experiments are active.</p></div><div class="hero-actions"><a href="/scenarios" class="btn btn-secondary">📋 Browse Scenarios</a><a href="/run" class="btn btn-primary">▶️ Run New Test</a></div></div></div></div>"#,
        status_html = status_html,
        total_scenarios = total_scenarios,
        total_results = total_results,
        status_badge = if status.is_running {
            r#"<span class="live-indicator"><span class="live-dot"></span> Running</span>"#
        } else {
            r#"<span class="badge badge-neutral">Idle</span>"#
        },
        results_html = results_html
    );

    base_layout("Dashboard", &content)
}

pub fn scenarios_page(scenarios: &[ScenarioInfo]) -> String {
    let scenarios_html = if scenarios.is_empty() {
        r#"<div class="empty-state"><div class="empty-state-icon">📋</div><p class="empty-state-title">No scenarios found</p><p>Add YAML scenario files to the scenarios directory.</p></div>"#.to_string()
    } else {
        scenarios.iter().map(|s| {
            format!(
                r#"<div class="scenario-card" onclick="window.location='/scenarios/{}'"><h3 class="scenario-name">{}</h3><p class="text-secondary">{}</p><div class="scenario-meta"><span class="scenario-meta-item">⏱️ {}</span><span class="scenario-meta-item">📊 {} phases</span></div><div class="mt-2"><button class="btn btn-primary btn-sm" onclick="event.stopPropagation(); runScenario('{}')">▶️ Run</button></div></div>"#,
                s.file_name,
                escape_html(&s.name),
                escape_html(s.description.as_deref().unwrap_or("No description")),
                s.duration,
                s.phase_count,
                s.file_name
            )
        }).collect::<Vec<_>>().join("\n")
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Scenario library</div><h1 class="page-title">Scenarios</h1><p class="page-subtitle">Browse and run chaos test scenarios.</p></div></div><div class="grid grid-3">{scenarios_html}</div>"#,
        scenarios_html = scenarios_html
    );
    base_layout("Scenarios", &content)
}

pub fn scenario_detail_page(
    scenario: &ScenarioInfo,
    yaml_content: &str,
    phases: &[PhaseInfo],
) -> String {
    let phases_html: String = phases.iter().map(|p| {
        let injections = p.injections.iter().map(|i| format!(r#"<span class="badge badge-info">{}</span>"#, escape_html(i))).collect::<Vec<_>>().join(" ");
        format!(r#"<div class="timeline-item"><h4 class="timeline-title">{}</h4><p class="timeline-content">Duration: {}</p><div class="mt-1">{}</div></div>"#, escape_html(&p.name), p.duration, injections)
    }).collect();

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Scenario detail</div><h1 class="page-title">{}</h1><p class="page-subtitle">{}</p></div><button class="btn btn-primary btn-lg" onclick="runScenario('{}')">▶️ Run Test</button></div><div class="grid grid-2"><div class="card"><div class="card-header"><h2 class="card-title">Phases</h2></div><div class="card-body"><div class="timeline">{}</div></div></div><div class="card"><div class="card-header"><h2 class="card-title">Scenario Configuration</h2></div><div class="card-body"><div class="code-block"><code><pre>{}</pre></code></div></div></div></div>"#,
        escape_html(&scenario.name),
        escape_html(scenario.description.as_deref().unwrap_or("No description")),
        scenario.file_name,
        phases_html,
        escape_html(yaml_content)
    );
    base_layout(&scenario.name, &content)
}

pub fn run_page(scenarios: &[ScenarioInfo], status: &crate::state::TestStatus) -> String {
    let options = scenarios
        .iter()
        .map(|s| {
            format!(
                r#"<option value="{}">{} ({})</option>"#,
                s.file_name,
                escape_html(&s.name),
                s.duration
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let status_section = if status.is_running {
        format!(
            r#"<div class="alert alert-info mb-4"><span class="live-indicator"><span class="live-dot"></span> Test in progress</span></div><div class="card"><div class="card-header"><h2 class="card-title">Current Test: {}</h2><button class="btn btn-danger" onclick="stopTest()">Stop Test</button></div><div class="card-body"><p class="mb-2"><strong>Phase:</strong> <span id="current-phase">{}</span></p><div class="progress-container"><div class="progress-bar" id="progress-bar" style="width: {}%"></div></div><p class="text-secondary" id="progress-text">{:.1}% complete - {} / {} seconds elapsed</p></div></div>"#,
            escape_html(status.scenario_name.as_deref().unwrap_or("Unknown")),
            escape_html(status.current_phase.as_deref().unwrap_or("Starting...")),
            status.progress_percent,
            status.progress_percent,
            status.elapsed_seconds,
            status.total_seconds
        )
    } else {
        format!(
            r#"<div class="card"><div class="card-header"><h2 class="card-title">Run New Test</h2></div><div class="card-body"><form id="run-form" onsubmit="event.preventDefault(); runScenario(document.getElementById('scenario-select').value);"><div class="form-group"><label class="form-label" for="scenario-select">Select Scenario</label><select id="scenario-select" class="form-select">{}</select></div><button type="submit" class="btn btn-primary btn-lg">▶️ Start Test</button></form></div></div>"#,
            options
        )
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Execution</div><h1 class="page-title">Run Chaos Test</h1><p class="page-subtitle">Execute chaos engineering scenarios.</p></div></div>{}"#,
        status_section
    );
    base_layout("Run Test", &content)
}

pub fn results_page(results: &[crate::state::ResultSummary]) -> String {
    let results_html = if results.is_empty() {
        r#"<div class="empty-state"><div class="empty-state-icon">📊</div><p class="empty-state-title">No test results yet</p><p>Run a chaos test to see results here.</p></div>"#.to_string()
    } else {
        let rows: String = results.iter().map(|r| {
            let success_class = if r.success_rate >= 0.9 { "badge-success" } else if r.success_rate >= 0.7 { "badge-warning" } else { "badge-danger" };
            format!(r#"<tr onclick="window.location='/results/{}'" style="cursor:pointer"><td><strong>{}</strong></td><td><span class="badge {}">{:.1}%</span></td><td>{}s</td><td>{}</td><td><a href="/results/{}" class="btn btn-secondary btn-sm">View</a></td></tr>"#, r.id, escape_html(&r.scenario_name), success_class, r.success_rate * 100.0, r.total_duration_secs, r.timestamp.format("%Y-%m-%d %H:%M:%S"), r.id)
        }).collect();
        format!(
            r#"<div class="table-container"><table><thead><tr><th>Scenario</th><th>Success Rate</th><th>Duration</th><th>Timestamp</th><th>Actions</th></tr></thead><tbody>{}</tbody></table></div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">History</div><h1 class="page-title">Test Results</h1><p class="page-subtitle">View historical chaos test results.</p></div></div>{}"#,
        results_html
    );
    base_layout("Results", &content)
}

pub fn result_detail_page(result: &chaos_scenarios::runner::ScenarioResult) -> String {
    let success_class = if result.success_rate() >= 0.9 {
        "text-success"
    } else if result.success_rate() >= 0.7 {
        "text-warning"
    } else {
        "text-danger"
    };
    let phases_html: String = result.phase_results.iter().map(|p| format!(r#"<div class="timeline-item completed"><h4 class="timeline-title">{}</h4><p class="timeline-content">Duration: {:?} | Injections: {}</p></div>"#, escape_html(&p.name), p.duration, p.injection_count)).collect();
    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Completed</div><h1 class="page-title">{}</h1><p class="page-subtitle">Test completed.</p></div><a href="/results" class="btn btn-secondary">← Back to Results</a></div><div class="grid grid-4 mb-4"><div class="stat-card"><span class="stat-icon">⏱️</span><span class="stat-value">{}s</span><span class="stat-label">Duration</span></div><div class="stat-card"><span class="stat-icon">📊</span><span class="stat-value {}">{:.1}%</span><span class="stat-label">Success Rate</span></div><div class="stat-card"><span class="stat-icon">⚡</span><span class="stat-value">{}</span><span class="stat-label">Injections</span></div><div class="stat-card"><span class="stat-icon">📋</span><span class="stat-value">{}</span><span class="stat-label">Phases</span></div></div><div class="card"><div class="card-header"><h2 class="card-title">Phase Timeline</h2></div><div class="card-body"><div class="timeline">{}</div></div></div>"#,
        escape_html(&result.scenario_name),
        result.total_duration.as_secs(),
        success_class,
        result.success_rate() * 100.0,
        result.total_injections,
        result.phase_results.len(),
        phases_html
    );
    base_layout(&result.scenario_name, &content)
}

pub fn error_page(title: &str, message: &str) -> String {
    let content = format!(
        r#"<div class="empty-state"><div class="empty-state-icon">❌</div><p class="empty-state-title">{}</p><p>{}</p><a href="/" class="btn btn-primary mt-3">← Back to Dashboard</a></div>"#,
        escape_html(title),
        escape_html(message)
    );
    base_layout("Error", &content)
}

pub fn load_test_page(
    targets: &[crate::state::CustomTarget],
    is_running: bool,
    metrics: &crate::load_test::LoadTestMetrics,
) -> String {
    let target_options = targets
        .iter()
        .filter(|target| matches!(target.target_type.as_str(), "http" | "hls"))
        .map(|t| {
            format!(
                r#"<option value="{}">{} ({})</option>"#,
                escape_html(&t.url),
                escape_html(&t.name),
                escape_html(&t.target_type)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let status_section = if is_running {
        format!(
            r#"<div class="alert alert-info mb-4"><span class="live-indicator"><span class="live-dot"></span> Load test in progress</span></div><div class="grid grid-4 mb-4"><div class="stat-card"><span class="stat-icon">📊</span><span class="stat-value">{}</span><span class="stat-label">Total Requests</span></div><div class="stat-card"><span class="stat-icon">✅</span><span class="stat-value text-success">{}</span><span class="stat-label">Successful</span></div><div class="stat-card"><span class="stat-icon">❌</span><span class="stat-value text-danger">{}</span><span class="stat-label">Failed</span></div><div class="stat-card"><span class="stat-icon">⚡</span><span class="stat-value">{:.1}</span><span class="stat-label">Req/sec</span></div></div><div class="grid grid-3 mb-4"><div class="stat-card"><span class="stat-icon">🕐</span><span class="stat-value">{:.1}ms</span><span class="stat-label">Avg Latency</span></div><div class="stat-card"><span class="stat-icon">📈</span><span class="stat-value">{:.1}ms</span><span class="stat-label">P95 Latency</span></div><div class="stat-card"><span class="stat-icon">📉</span><span class="stat-value">{:.1}ms</span><span class="stat-label">P99 Latency</span></div></div><button class="btn btn-danger btn-lg" onclick="stopLoadTest()">⏹️ Stop Test</button>"#,
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
            r#"<div class="card"><div class="card-header"><h2 class="card-title">Configure Load Test</h2></div><div class="card-body"><form id="load-test-form" onsubmit="event.preventDefault(); startLoadTest();"><div class="grid grid-2"><div class="form-group"><label class="form-label">Test Name</label><input type="text" id="test-name" class="form-input" value="My Load Test" required></div><div class="form-group"><label class="form-label">Target Type</label><select id="target-type" class="form-select"><option value="http">HTTP/HTTPS API</option><option value="hls">HLS Manifest</option></select></div></div><div class="form-group"><label class="form-label">Target URL</label><input type="text" id="target-url" class="form-input" placeholder="http://localhost:3000/api/endpoint" required>{}</div><div class="form-group"><label class="form-label">HTTP Method</label><select id="http-method" class="form-select"><option value="GET">GET</option><option value="POST">POST</option><option value="PUT">PUT</option><option value="DELETE">DELETE</option><option value="PATCH">PATCH</option></select></div><div class="form-group"><label class="form-label">Request Body (JSON)</label><textarea id="request-body" class="form-textarea" placeholder='{{"key": "value"}}'></textarea></div><div class="grid grid-4"><div class="form-group"><label class="form-label">Concurrent Users</label><input type="number" id="concurrent-users" class="form-input" value="10" min="1" max="1000"></div><div class="form-group"><label class="form-label">Requests/Second</label><input type="number" id="rps" class="form-input" value="100" min="1" max="10000"></div><div class="form-group"><label class="form-label">Duration (seconds)</label><input type="number" id="duration" class="form-input" value="60" min="1" max="3600"></div><div class="form-group"><label class="form-label">Ramp-up (seconds)</label><input type="number" id="ramp-up" class="form-input" value="10" min="0" max="300"></div></div><button type="submit" class="btn btn-primary btn-lg">🚀 Start Load Test</button></form></div></div>"#,
            if !target_options.is_empty() {
                format!(
                    r#"<p class="text-secondary mt-1">Or select from saved targets:<select id="saved-targets" class="form-select mt-1" onchange="document.getElementById('target-url').value = this.value"><option value="">-- Select saved target --</option>{}</select></p>"#,
                    target_options
                )
            } else {
                String::new()
            }
        )
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Load testing</div><h1 class="page-title">Load Testing</h1><p class="page-subtitle">Generate controlled traffic against HTTP services and HLS manifests.</p></div></div>{status_section}<div class="card mt-4"><div class="card-header"><h2 class="card-title">Supported Target Types</h2></div><div class="card-body"><div class="grid grid-2"><div class="flex gap-2 items-center"><span class="badge badge-info">HTTP/HTTPS</span><span class="text-secondary">REST APIs and web services</span></div><div class="flex gap-2 items-center"><span class="badge badge-success">HLS</span><span class="text-secondary">HTTP Live Streaming manifests</span></div></div></div></div>"#,
        status_section = status_section
    );
    base_layout("Load Testing", &content)
}

pub fn targets_page(targets: &[crate::state::CustomTarget]) -> String {
    let targets_html = if targets.is_empty() {
        r#"<div class="empty-state"><div class="empty-state-icon">🎯</div><p class="empty-state-title">No targets configured</p><p>Add your first target to get started.</p></div>"#.to_string()
    } else {
        let rows: String = targets.iter().map(|t| {
            format!(r#"<tr><td><strong>{}</strong></td><td><span class="badge badge-info">{}</span></td><td><code>{}</code></td><td>{}</td><td><button class="btn btn-danger btn-sm" onclick="deleteTarget('{}')">Delete</button></td></tr>"#, escape_html(&t.name), escape_html(&t.target_type), escape_html(&t.url), escape_html(t.description.as_deref().unwrap_or("-")), t.id)
        }).collect();
        format!(
            r#"<div class="table-container"><table><thead><tr><th>Name</th><th>Type</th><th>URL</th><th>Description</th><th>Actions</th></tr></thead><tbody>{}</tbody></table></div>"#,
            rows
        )
    };

    let content = format!(
        r#"<div class="page-header"><div><div class="hero-kicker">Targets</div><h1 class="page-title">Targets</h1><p class="page-subtitle">Manage your stress testing targets.</p></div></div><div class="card mb-4"><div class="card-header"><h2 class="card-title">Add New Target</h2></div><div class="card-body"><form id="add-target-form" onsubmit="event.preventDefault(); addTarget();"><div class="grid grid-2"><div class="form-group"><label class="form-label">Target Name</label><input type="text" id="target-name" class="form-input" placeholder="My API Server" required></div><div class="form-group"><label class="form-label">Target Type</label><select id="new-target-type" class="form-select"><option value="http">HTTP/HTTPS API</option><option value="hls">HLS Manifest</option></select></div></div><div class="form-group"><label class="form-label">URL/Endpoint</label><input type="text" id="new-target-url" class="form-input" placeholder="http://localhost:3000" required></div><div class="form-group"><label class="form-label">Description (optional)</label><input type="text" id="new-target-desc" class="form-input" placeholder="Production API endpoint"></div><button type="submit" class="btn btn-primary">➕ Add Target</button></form></div></div><div class="card"><div class="card-header"><h2 class="card-title">Saved Targets</h2></div><div class="card-body">{targets_html}</div></div>"#,
        targets_html = targets_html
    );
    base_layout("Targets", &content)
}
