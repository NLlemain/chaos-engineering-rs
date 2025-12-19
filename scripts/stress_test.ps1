# Comprehensive 15-minute stress test  
# Works on Windows, macOS (pwsh), and Linux (pwsh)
# Network chaos now supported # Results
Write-Host ""Write-Host "What happened:" -ForegroundColor Yellow
Write-Host "  - Tested service under progressively heavier load"
Write-Host "  - Injected CPU, memory, and network chaos"
Write-Host "  - Verified recovery back to baseline"
Write-Host "  - Service should have stayed functional throughout"
Write-Host ""e-Host "==========================================" -ForegroundColor Cyan
if ($exitCode -eq 0) {
    Write-Host " Test Complete! OK" -ForegroundColor Green
} else {
    Write-Host " Test Finished (with errors)" -ForegroundColor Yellow
}
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""atforms!

Write-Host ""
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " Comprehensive Stress Test (15 minutes)" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

# Detect platform
# PowerShell 5.1 doesn't have $IsWindows, so we detect it
$IsWindowsPlatform = if ($PSVersionTable.PSVersion.Major -ge 6) { $IsWindows } else { $true }
$IsLinux = $PSVersionTable.PSVersion.Major -ge 6 -and $PSVersionTable.Platform -eq "Unix" -and -not $IsMacOS
$IsMac = $PSVersionTable.PSVersion.Major -ge 6 -and $IsMacOS

# Paths
$chaosExe = if ($IsWindowsPlatform) { ".\target\release\chaos.exe" } else { "./target/release/chaos" }
$serviceExe = if ($IsWindowsPlatform) { ".\target\release\axum_http_service.exe" } else { "./target/release/axum_http_service" }
$scenarioFile = if ($IsWindowsPlatform) { ".\scenarios\stress_test.yaml" } else { "./scenarios/stress_test.yaml" }

# Platform info
if ($IsLinux) {
    Write-Host "Platform: Linux (full network chaos with tc/netem)" -ForegroundColor Green
} elseif ($IsMac) {
    Write-Host "Platform: macOS (network chaos with dnctl/pfctl)" -ForegroundColor Green
} else {
    Write-Host "Platform: Windows (network chaos simulation)" -ForegroundColor Green
}
Write-Host ""

# Check binaries
if (!(Test-Path $chaosExe)) {
    Write-Host "ERROR: chaos binary not found. Run 'cargo build --release' first." -ForegroundColor Red
    exit 1
}

# Step 1: Service check/start
Write-Host "[1/5] Checking service..." -ForegroundColor Yellow
$serviceProcess = Get-Process -Name "axum_http_service" -ErrorAction SilentlyContinue

if ($serviceProcess) {
    Write-Host "      Service running (PID: $($serviceProcess.Id))" -ForegroundColor Green
} else {
    Write-Host "      Starting service..." -ForegroundColor Yellow
    if ($IsWindowsPlatform) {
        Start-Process -FilePath $serviceExe -WindowStyle Hidden
    } else {
        Start-Process -FilePath $serviceExe
    }
    Start-Sleep -Seconds 2
    $serviceProcess = Get-Process -Name "axum_http_service" -ErrorAction SilentlyContinue
    if (!$serviceProcess) {
        Write-Host "      ERROR: Could not start service" -ForegroundColor Red
        exit 1
    }
    Write-Host "      Service started (PID: $($serviceProcess.Id))" -ForegroundColor Green
}

# Step 2: Health check
Write-Host "`n[2/5] Health check..." -ForegroundColor Yellow
try {
    $health = Invoke-RestMethod -Uri "http://localhost:8080/health" -TimeoutSec 5
    Write-Host "      Status: $($health.status)" -ForegroundColor Green
} catch {
    Write-Host "      WARNING: Health check failed" -ForegroundColor Yellow
}

# Step 3: Prepare scenario
Write-Host "`n[3/5] Preparing scenario..." -ForegroundColor Yellow
$servicePid = $serviceProcess.Id
Write-Host "      Target PID: $servicePid" -ForegroundColor Gray

# Update scenario with PID
$scenarioContent = Get-Content $scenarioFile -Raw
$scenarioContent = $scenarioContent -replace 'process_name:\s*"axum_http_service"', "pid: $servicePid"

$tempScenario = Join-Path $env:TEMP "stress_test_$servicePid.yaml"
$scenarioContent | Set-Content $tempScenario

# Step 4: Show what's about to happen
Write-Host "`n[4/5] Test plan:" -ForegroundColor Yellow
Write-Host "      Phase 1: baseline          (2 min)  - Normal operation"
Write-Host "      Phase 2: light_network     (2 min)  - 20ms latency"
Write-Host "      Phase 3: moderate_combined (2 min)  - 60% CPU + 50ms latency"
Write-Host "      Phase 4: memory_stress     (2 min)  - 75% memory usage"
Write-Host "      Phase 5: heavy_chaos       (3 min)  - 80% CPU + 85% mem + 20% packet loss"
Write-Host "      Phase 6: recovery          (4 min)  - Back to normal"
Write-Host ""
Write-Host "      Total: 15 minutes" -ForegroundColor Cyan
Write-Host ""

# Confirmation
$response = Read-Host "Ready to start? (y/n)"
if ($response -ne 'y') {
    Write-Host "Cancelled." -ForegroundColor Yellow
    Remove-Item $tempScenario -ErrorAction SilentlyContinue
    exit 0
}

# Step 5: Run test
Write-Host ""
Write-Host "[5/5] Running test..." -ForegroundColor Yellow
Write-Host ""

# Create results directory
$resultsDir = "test_results"
if (!(Test-Path $resultsDir)) {
    New-Item -ItemType Directory -Path $resultsDir | Out-Null
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$jsonFile = Join-Path $resultsDir "stress_test_$timestamp.json"
$mdFile = Join-Path $resultsDir "stress_test_$timestamp.md"

# Run chaos with reports
& $chaosExe run $tempScenario --verbose --output-json $jsonFile --output-markdown $mdFile

$exitCode = $LASTEXITCODE

# Cleanup
Remove-Item $tempScenario -ErrorAction SilentlyContinue

# Results
Write-Host "`n==========================================" -ForegroundColor Cyan
if ($exitCode -eq 0) {
    Write-Host " Test Complete! ✓" -ForegroundColor Green
} else {
    Write-Host " Test Finished (with errors)" -ForegroundColor Yellow
}
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Results saved:" -ForegroundColor Yellow
Write-Host "  JSON:     $jsonFile"
Write-Host "  Markdown: $mdFile"
Write-Host ""

Write-Host "What happened:" -ForegroundColor Yellow
Write-Host "  • Tested service under progressively heavier load"
Write-Host "  • Injected CPU, memory, and network chaos"
Write-Host "  • Verified recovery back to baseline"
Write-Host "  • Service should have stayed functional throughout"
Write-Host ""

if (Test-Path $mdFile) {
    Write-Host "View the report:" -ForegroundColor Yellow
    if ($IsWindowsPlatform) {
        Write-Host "  notepad $mdFile"
    } else {
        Write-Host "  cat $mdFile"
    }
    Write-Host ""
}

# Cleanup prompt
$response = Read-Host "Stop the test service? (y/n)"
if ($response -eq 'y') {
    Write-Host "Stopping service..." -ForegroundColor Yellow
    Stop-Process -Name "axum_http_service" -ErrorAction SilentlyContinue
    Write-Host "Service stopped." -ForegroundColor Green
}

Write-Host ""
Write-Host "Done!"
Write-Host ""

