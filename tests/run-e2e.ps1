# Runs every automatic test of AnyFont and prints one summary. Nothing here takes the mouse, the keyboard or the
# clipboard, opens a window over your work, or changes this computer's fonts or your AnyFont profile (see
# docs/TESTING.md), so you can keep working while it runs.
#
#   tests\run-e2e.ps1            unit + headless end-to-end tests, translation check, issue-relay tests
#   tests\run-e2e.ps1 -Ignored   also run the tests marked #[ignore] (the known bugs of docs/QA-BUGS.md)
#
# The real installer and font system are tested on GitHub Actions only (.github/workflows/e2e-real.yml): nothing
# local ever installs fonts or opens a window.
# Exit code 0 when every step passed (missing translations are reported, not counted as a failure).
param(
    [switch]$Ignored
)
$ErrorActionPreference = "Continue"   # PowerShell 5.1 turns a native program's stderr into an error under "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$steps = New-Object System.Collections.ArrayList
$logDir = Join-Path $repo "target\e2e-logs"
New-Item -ItemType Directory -Force $logDir | Out-Null

function Step($name, [scriptblock]$body, [switch]$ReportOnly) {
    Write-Host ""
    Write-Host "=== $name" -ForegroundColor Cyan
    $t0 = Get-Date
    $global:LASTEXITCODE = 0
    $detail = & $body
    $code = $global:LASTEXITCODE
    $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds)
    $status = if ($code -eq 0) { "PASS" } elseif ($ReportOnly) { "REPORT" } else { "FAIL" }
    [void]$steps.Add([pscustomobject]@{ Step = $name; Status = $status; Seconds = $secs; Detail = "$detail" })
}

Push-Location $repo
try {
    # 1. cargo test --workspace: the unit tests of the three crates and the headless e2e suite (egui_kittest)
    Step "cargo test --workspace" {
        $log = Join-Path $logDir "cargo-test.log"
        $extra = @()
        if ($Ignored) { $extra = @("--", "--include-ignored") }
        & cargo test --workspace @extra 2>&1 | Tee-Object -FilePath $log | ForEach-Object { "$_" } |
            Where-Object { $_ -match "^(test result|     Running|error|failures:|    [a-z_:0-9]+$|test .* FAILED)" } | Write-Host
        $code = $LASTEXITCODE
        $sum = Select-String -Path $log -Pattern "^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored" |
            ForEach-Object { $_.Matches[0] } |
            ForEach-Object { [pscustomobject]@{ p = [int]$_.Groups[1].Value; f = [int]$_.Groups[2].Value; i = [int]$_.Groups[3].Value } }
        $p = ($sum | Measure-Object p -Sum).Sum; $f = ($sum | Measure-Object f -Sum).Sum; $i = ($sum | Measure-Object i -Sum).Sum
        $global:LASTEXITCODE = $code
        "$p passed, $f failed, $i ignored (log: $log)"
    }

    # 2. translations: missing keys are reported, not failed (yet)
    Step "i18n check (report only)" {
        $py = (Get-Command python -ErrorAction SilentlyContinue)
        if (-not $py) { $py = (Get-Command py -ErrorAction SilentlyContinue) }
        if (-not $py) { $global:LASTEXITCODE = 1; return "python not found" }
        $log = Join-Path $logDir "i18n-check.log"
        & $py.Source -I scripts\i18n_check.py 2>&1 | Tee-Object -FilePath $log | Select-Object -Last 20 | ForEach-Object { Write-Host "$_" }
        $n = @(Get-Content $log | Where-Object { $_ -match "missing" }).Count
        "$n line(s) mention missing keys (log: $log)"
    } -ReportOnly

    # 3. the issue relay's own tests (Node)
    Step "issue relay tests (npm test)" {
        $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
        if (-not $npm) { $global:LASTEXITCODE = 1; return "npm.cmd not found" }
        Push-Location services\issue-relay
        try {
            $log = Join-Path $logDir "issue-relay.log"
            & $npm.Source test 2>&1 | Tee-Object -FilePath $log | Select-Object -Last 15 | ForEach-Object { Write-Host "$_" }
            "log: $log"
        } finally { Pop-Location }
    }
} finally { Pop-Location }

Write-Host ""
Write-Host "=== Summary" -ForegroundColor Cyan
$steps | Format-Table -AutoSize Step, Status, Seconds, Detail | Out-String -Width 200 | Write-Host
$failed = @($steps | Where-Object { $_.Status -eq "FAIL" }).Count
if ($failed -gt 0) { Write-Host "$failed step(s) failed" -ForegroundColor Red; exit 1 }
Write-Host "All steps passed" -ForegroundColor Green
exit 0
