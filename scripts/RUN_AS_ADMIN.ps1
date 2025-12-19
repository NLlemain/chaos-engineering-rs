# Run Stress Test as Administrator
# This script will elevate privileges and run the full stress test

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Running Chaos Test as Administrator" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if already running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "⚠️  Not running as Administrator!" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To run with full network chaos capabilities:" -ForegroundColor Yellow
    Write-Host "  1. Right-click on PowerShell" -ForegroundColor White
    Write-Host "  2. Select 'Run as Administrator'" -ForegroundColor White
    Write-Host "  3. Navigate to: $PSScriptRoot" -ForegroundColor White
    Write-Host "  4. Run: .\scripts\stress_test.ps1" -ForegroundColor White
    Write-Host ""
    Write-Host "Press any key to exit..."
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    exit 1
}

Write-Host "✓ Running as Administrator" -ForegroundColor Green
Write-Host ""

# Change to script directory
Set-Location $PSScriptRoot

# Run the stress test
Write-Host "Starting comprehensive stress test..." -ForegroundColor Yellow
Write-Host ""

& .\scripts\stress_test.ps1

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Test Complete!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press any key to exit..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
