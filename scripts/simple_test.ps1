# Simple 3-minute chaos test
# Works on Windows, macOS (pwsh), and Linux (pwsh)

Write-Host "`n=================================" -ForegroundColor Cyan
Write-Host " Simple Chaos Test (3 minutes)" -ForegroundColor Cyan
Write-Host "=================================`n" -ForegroundColor Cyan

# Check if we're on Windows, macOS, or Linux
# PowerShell 5.1 doesn't have $IsWindows, so we detect it
$IsWindowsPlatform = if ($PSVersionTable.PSVersion.Major -ge 6) { $IsWindows } else { $true }
$IsLinux = $PSVersionTable.PSVersion.Major -ge 6 -and $PSVersionTable.Platform -eq "Unix" -and -not $IsMacOS
$IsMac = $PSVersionTable.PSVersion.Major -ge 6 -and $IsMacOS

# Paths
$chaosExe = if ($IsWindowsPlatform) { ".\target\release\chaos.exe" } else { "./target/release/chaos" }
$serviceExe = if ($IsWindowsPlatform) { ".\target\release\axum_http_service.exe" } else { "./target/release/axum_http_service" }
$scenarioFile = if ($IsWindowsPlatform) { ".\scenarios\quick_test.yaml" } else { "./scenarios/quick_test.yaml" }

# Check if binaries exist
if (!(Test-Path $chaosExe)) {
    Write-Host "ERROR: chaos binary not found. Run 'cargo build --release' first." -ForegroundColor Red
    exit 1
}

# Step 1: Check if service is already running
Write-Host "[1/4] Checking for running service..." -ForegroundColor Yellow
$serviceProcess = Get-Process -Name "axum_http_service" -ErrorAction SilentlyContinue

if ($serviceProcess) {
    Write-Host "      Service already running (PID: $($serviceProcess.Id))" -ForegroundColor Green
} else {
    Write-Host "      Starting service..." -ForegroundColor Yellow
    if ($IsWindowsPlatform) {
        Start-Process -FilePath $serviceExe -WindowStyle Hidden
    } else {
        Start-Process -FilePath $serviceExe
    }
    Start-Sleep -Seconds 2
    $serviceProcess = Get-Process -Name "axum_http_service" -ErrorAction SilentlyContinue
    if ($serviceProcess) {
        Write-Host "      Service started (PID: $($serviceProcess.Id))" -ForegroundColor Green
    } else {
        Write-Host "      ERROR: Could not start service" -ForegroundColor Red
        exit 1
    }
}

# Step 2: Quick health check
Write-Host "`n[2/4] Testing service health..." -ForegroundColor Yellow
try {
    $health = Invoke-RestMethod -Uri "http://localhost:8080/health" -TimeoutSec 5
    Write-Host "      Status: $($health.status)" -ForegroundColor Green
} catch {
    Write-Host "      WARNING: Health check failed, but continuing..." -ForegroundColor Yellow
}

# Step 3: Update scenario file with actual PID
Write-Host "`n[3/4] Preparing test scenario..." -ForegroundColor Yellow
$servicePid = $serviceProcess.Id
Write-Host "      Target PID: $servicePid" -ForegroundColor Gray

# Read scenario and replace process_name with actual PID
$scenarioContent = Get-Content $scenarioFile -Raw
$scenarioContent = $scenarioContent -replace 'process_name:\s*"axum_http_service"', "pid: $servicePid"
$tempScenario = Join-Path $env:TEMP "quick_test_$servicePid.yaml"
$scenarioContent | Set-Content $tempScenario

# Step 4: Run the test
Write-Host "`n[4/4] Running chaos test (3 minutes)...`n" -ForegroundColor Yellow
Write-Host "      Phases: baseline (1m) -> cpu_stress (1m) -> recovery (1m)" -ForegroundColor Gray
Write-Host ""

# Run chaos
& $chaosExe run $tempScenario --verbose

# Cleanup
Remove-Item $tempScenario -ErrorAction SilentlyContinue

# Done!
Write-Host "`n=================================" -ForegroundColor Cyan
Write-Host " Test Complete!" -ForegroundColor Green
Write-Host "=================================`n" -ForegroundColor Cyan

Write-Host "What happened:" -ForegroundColor Yellow
Write-Host "  1. Measured baseline performance (1 min)"
Write-Host "  2. Added CPU load to stress the service (1 min)"  
Write-Host "  3. Verified recovery back to normal (1 min)`n"

Write-Host "The service should have stayed responsive throughout."
Write-Host "Check the logs above for any errors or warnings.`n"

# Ask if they want to stop the service
$response = Read-Host "Stop the test service? (y/n)"
if ($response -eq 'y') {
    Write-Host "Stopping service..." -ForegroundColor Yellow
    Stop-Process -Name "axum_http_service" -ErrorAction SilentlyContinue
    Write-Host "Service stopped." -ForegroundColor Green
}

Write-Host "`nDone!`n"
