# Real-system end-to-end test of AnyFont: the real setup, the real Windows font system (registry, Windows\Fonts,
# the per-user fonts folder, GDI, the Recycle Bin, the elevated helper) and the real uninstaller.
#
# It changes the fonts and the registry of the Windows it runs on, so it runs ONLY on a disposable machine: a
# GitHub-hosted Actions runner (.github/workflows/e2e-real.yml). It refuses to run anywhere else. Never run it on
# your PC (there is no local mode; the headless suite, `cargo test`, covers the program locally).
#
#   tests\real-system\run-real.ps1 -Setup dist\AnyFont-<version>-setup.exe -Results <folder>
#
# Writes <folder>\summary.json (every check), selftest.json (anyfont.exe /SelfTest: steps), setup and uninstall
# logs, AnyFont's log and a screenshot. Exit code 0 when every check passed. Checks marked "soft" (the window on the
# runner's desktop) are reported but do not fail the run.
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Results
)
$ErrorActionPreference = "Continue"
if ($env:GITHUB_ACTIONS -ne "true" -or $env:RUNNER_ENVIRONMENT -ne "github-hosted") {
    [Console]::Error.WriteLine("run-real.ps1 refused: it installs and uninstalls fonts of this Windows; it runs only on a GitHub-hosted Actions runner.")
    exit 2
}
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
New-Item -ItemType Directory -Force $Results | Out-Null
$Results = (Resolve-Path $Results).Path
$log = Join-Path $Results "run-real.log"
$checks = New-Object System.Collections.ArrayList
$appId = "{80A727BB-0AF4-4B46-A41E-269BD512170B}_is1"
$fixtures = Join-Path $repo "crates\anyfont-core\tests\fixtures"

function Say($text) {
    $line = "{0:HH:mm:ss} {1}" -f (Get-Date), $text
    Write-Host $line
    Add-Content -Path $log -Value $line -Encoding utf8
}
function Check($name, $ok, $detail, [switch]$Soft) {
    [void]$checks.Add([ordered]@{ name = $name; ok = [bool]$ok; soft = [bool]$Soft; detail = "$detail" })
    $mark = if ($ok) { "PASS" } elseif ($Soft) { "WARN" } else { "FAIL" }
    Say "$mark $name - $detail"
    if (-not $ok -and -not $Soft) { Write-Host "::error::$name - $detail" }
}
function Shot($name) {
    try {
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
        $bmp.Save((Join-Path $Results "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
    } catch { Say "screenshot $name failed: $_" }
}
function UninstallKey() {
    foreach ($root in "HKCU:", "HKLM:") {
        $k = "$root\Software\Microsoft\Windows\CurrentVersion\Uninstall\$appId"
        if (Test-Path $k) { return $k }
    }
    return $null
}
function Finish($code) {
    $hard = @($checks | Where-Object { -not $_.soft })
    $summary = [ordered]@{
        passed = @($checks | Where-Object { $_.ok }).Count
        failed = @($hard | Where-Object { -not $_.ok }).Count
        warnings = @($checks | Where-Object { $_.soft -and -not $_.ok }).Count
        checks = $checks
    }
    $summary | ConvertTo-Json -Depth 5 | Set-Content -Path (Join-Path $Results "summary.json") -Encoding utf8
    if ($summary.failed -gt 0 -and $code -eq 0) { $code = 1 }
    Say "done: $($summary.passed) passed, $($summary.failed) failed, $($summary.warnings) warnings"
    exit $code
}

try {
    Say "user $env:USERNAME on $([Environment]::OSVersion.VersionString)"
    $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    Say "running as administrator: $admin"

    # ---------------------------------------------------------------- 1. silent install
    $Setup = (Resolve-Path $Setup).Path
    Say "installing $(Split-Path $Setup -Leaf) silently (per user)"
    $p = Start-Process $Setup -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/LOG=`"$Results\setup.log`"" -Wait -PassThru
    Check "setup exit code" ($p.ExitCode -eq 0) "exit code $($p.ExitCode)"
    $key = UninstallKey
    Check "uninstall entry registered" ($null -ne $key) "$key"
    $dir = $null
    if ($key) { $dir = (Get-ItemProperty $key).InstallLocation }
    if (-not $dir) { $dir = Join-Path $env:LOCALAPPDATA "Programs\AnyFont" }
    $dir = $dir.TrimEnd('\')
    $exe = Join-Path $dir "anyfont.exe"
    Check "anyfont.exe installed" (Test-Path $exe) $exe
    Check "legal notices installed" (Test-Path (Join-Path $dir "legal\THIRD-PARTY-NOTICES.txt")) "$dir\legal"
    $lnk = Join-Path ([Environment]::GetFolderPath("Programs")) "AnyFont.lnk"
    Check "Start menu shortcut" (Test-Path $lnk) $lnk
    if (-not (Test-Path $exe)) { Finish 2 }
    $ver = (Get-Item $exe).VersionInfo
    Say "installed version $($ver.ProductVersion) ($($ver.FileDescription))"

    # BUG-401: the runner has the Visual C++ runtime installed, a fresh Windows does not. The program must not need
    # it, or the setup must bring it.
    $text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($exe))
    $needs = @([regex]::Matches($text, '(?i)(vcruntime140(_1)?|msvcp140)\.dll') | ForEach-Object { $_.Value.ToLower() } | Sort-Object -Unique)
    $missing = @($needs | Where-Object { -not (Test-Path (Join-Path $dir $_)) })
    Check "runs on a Windows without the Visual C++ runtime (static CRT or runtime shipped)" ($missing.Count -eq 0) "imports $($needs -join ', '); not in the program folder: $($missing -join ', ')"

    # ---------------------------------------------------------------- 2. the program on the real font system
    $appLog = Join-Path $env:LOCALAPPDATA "AnyFont\logs\anyfont.log"
    function LogHas($pattern) { (Test-Path $appLog) -and (Select-String -Path $appLog -Pattern $pattern -Quiet) }
    Say "starting AnyFont /NoSplash"
    $app = Start-Process $exe -ArgumentList "/NoSplash", "/SomeUnknownSwitch" -PassThru
    $deadline = (Get-Date).AddSeconds(90)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 500
        $app.Refresh()
        if ($app.HasExited) { break }
        if ($app.MainWindowHandle -ne 0 -and (LogHas "scan finished")) { break }
    }
    $app.Refresh()
    $state = if ($app.HasExited) { "exited with code $($app.ExitCode) (0x{0:X8})" -f $app.ExitCode } else { "running" }
    Check "AnyFont window opened" ((-not $app.HasExited) -and $app.MainWindowHandle -ne 0) "title '$($app.MainWindowTitle)', $state" -Soft
    $last = if (Test-Path $appLog) { Select-String -Path $appLog -Pattern "scan finished" | Select-Object -Last 1 } else { "no $appLog" }
    Check "first scan of the installed fonts finished" (LogHas "scan finished") "$last" -Soft
    Check "unknown switch ignored" (LogHas "ignoring the switch /SomeUnknownSwitch") "log line"
    Start-Sleep -Seconds 2
    Shot "anyfont-main"
    if (-not $app.HasExited) {
        # a second instance forwards its font file to the first one and exits
        $fwd = Join-Path $env:RUNNER_TEMP "anyfont-forward"
        New-Item -ItemType Directory -Force $fwd | Out-Null
        Copy-Item (Join-Path $fixtures "Tuffy.ttf") $fwd -Force
        $second = Start-Process $exe -ArgumentList "`"$(Join-Path $fwd 'Tuffy.ttf')`"" -PassThru
        $exited = $second.WaitForExit(20000)
        Check "second instance forwards and exits" ($exited -and (LogHas "arguments forwarded")) "exited $exited"
        $deadline = (Get-Date).AddSeconds(20)
        while ((Get-Date) -lt $deadline -and -not (LogHas "message from another instance")) { Start-Sleep -Milliseconds 500 }
        Check "first instance received the file" (LogHas "message from another instance: 1 file") "log line"
        Start-Sleep -Seconds 2
        Shot "anyfont-forwarded-file"
        $null = $app.CloseMainWindow()
        $closed = $app.WaitForExit(20000)
        if (-not $closed) { Stop-Process -Id $app.Id -Force }
        Check "AnyFont closes on WM_CLOSE" $closed "" -Soft
        Check "settings saved on exit" (Test-Path (Join-Path $env:APPDATA "AnyFont\settings.json")) "$env:APPDATA\AnyFont\settings.json" -Soft
    }
    Check "AnyFont log has no panic" ((Test-Path $appLog) -and -not (LogHas "panicked")) $appLog
    $crash = Join-Path $env:LOCALAPPDATA "AnyFont\crash-reports"
    Check "no crash report" (-not (Test-Path $crash) -or -not (Get-ChildItem $crash -Filter "crash-*.txt" -ErrorAction SilentlyContinue)) $crash

    # ---------------------------------------------------------------- 3. scripted font operations (/SelfTest:)
    $script = Get-Content (Join-Path $PSScriptRoot "selftest.json") -Raw -Encoding utf8 | ConvertFrom-Json
    $script.results = Join-Path $Results "selftest.json"
    $script.fixtures = $fixtures
    $script.work = Join-Path $env:RUNNER_TEMP "anyfont-selftest"
    $script.holders_script = Join-Path $PSScriptRoot "holders.ps1"
    $scriptFile = Join-Path $env:RUNNER_TEMP "anyfont-selftest.json"
    # (no BOM: serde_json does not accept one)
    [IO.File]::WriteAllText($scriptFile, ($script | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding $false))
    Say "running anyfont.exe /SelfTest:$scriptFile"
    $st = Start-Process $exe -ArgumentList "/SelfTest:$scriptFile" -Wait -PassThru
    $stFile = Join-Path $Results "selftest.json"
    $r = $null
    if (Test-Path $stFile) {
        $r = Get-Content $stFile -Raw -Encoding utf8 | ConvertFrom-Json
        foreach ($s in $r.steps) { Check ("selftest " + $s.op) $s.ok $s.detail }
        # the deleted copy is in the Recycle Bin
        $bin = (New-Object -ComObject Shell.Application).NameSpace(10)
        foreach ($d in $r.deleted) {
            $leaf = [IO.Path]::GetFileNameWithoutExtension($d)
            $hit = @($bin.Items() | Where-Object { $_.Name -like "$leaf*" -or $_.Path -like "*$leaf*" })
            Check "deleted copy is in the Recycle Bin" ($hit.Count -gt 0) "$d (bin: $((@($bin.Items() | ForEach-Object { $_.Name })) -join ', '))"
        }
    } else {
        Check "selftest results written" $false "exit code $($st.ExitCode), no $stFile"
    }
    Check "selftest exit code" ($st.ExitCode -eq 0) "exit code $($st.ExitCode)"
    # nothing of the test fonts is left in Windows (files in use answered with "Reboot later" must be queued)
    $pendingReboot = @()
    if ($r -and $r.pending_reboot) { $pendingReboot = @($r.pending_reboot) }
    $leftFiles = @(Get-ChildItem "$env:WINDIR\Fonts", (Join-Path $env:LOCALAPPDATA "Microsoft\Windows\Fonts") -ErrorAction SilentlyContinue |
        Where-Object { ($_.Name -like "Tuffy*" -or $_.Name -like "Hack-*") -and $pendingReboot -notcontains $_.FullName })
    Check "no test font left in the fonts folders (except those queued for the restart)" ($leftFiles.Count -eq 0) (($leftFiles | ForEach-Object FullName) -join ", ")
    if ($pendingReboot.Count -gt 0) {
        $pfro = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager" -Name PendingFileRenameOperations -ErrorAction SilentlyContinue).PendingFileRenameOperations
        foreach ($p in $pendingReboot) { Check "queued for deletion at the restart" ((-not (Test-Path $p)) -or (@($pfro) -like "*$p")) $p }
    }
    $leftReg = @()
    foreach ($k in "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts", "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts") {
        if (Test-Path $k) {
            $props = (Get-ItemProperty $k).PSObject.Properties | Where-Object { $_.Name -like "Tuffy*" -or $_.Name -like "Hack*" }
            $leftReg += @($props | ForEach-Object { "$k\$($_.Name)" })
        }
    }
    Check "no test font left in the registry" ($leftReg.Count -eq 0) ($leftReg -join ", ")

    # ---------------------------------------------------------------- 4. silent uninstall and leftovers
    Copy-Item $appLog (Join-Path $Results "anyfont.log") -ErrorAction SilentlyContinue
    $unins = Join-Path $dir "unins000.exe"
    Check "uninstaller present" (Test-Path $unins) $unins
    if (Test-Path $unins) {
        Say "uninstalling silently"
        $u = Start-Process $unins -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$Results\uninstall.log`"" -Wait -PassThru
        # the uninstaller runs from a temporary copy: wait until it has removed the program
        $deadline = (Get-Date).AddSeconds(90)
        while ((Get-Date) -lt $deadline -and ((Test-Path $exe) -or (UninstallKey))) { Start-Sleep -Milliseconds 500 }
        Check "uninstaller exit code" ($u.ExitCode -eq 0) "exit code $($u.ExitCode)"
    }
    Check "program removed" (-not (Test-Path $exe)) $exe
    $rest = @(Get-ChildItem $dir -Recurse -Force -ErrorAction SilentlyContinue)
    Check "install folder removed" ($rest.Count -eq 0) (($rest | ForEach-Object FullName) -join ", ")
    Check "uninstall entry removed" ($null -eq (UninstallKey)) ""
    Check "Start menu shortcut removed" (-not (Test-Path $lnk)) $lnk
    Check "file association key removed" (-not (Test-Path "HKCU:\Software\Classes\Applications\anyfont.exe")) "HKCU\Software\Classes\Applications\anyfont.exe"
    Check "Software\AnyFont key removed (empty)" (-not (Test-Path "HKCU:\Software\AnyFont")) "HKCU\Software\AnyFont"
    # kept by design ([UninstallDelete] is empty): the settings, the database, the logs, the Content folder
    $kept = @((Join-Path $env:APPDATA "AnyFont"), (Join-Path $env:LOCALAPPDATA "AnyFont")) | Where-Object { Test-Path $_ }
    Say "kept by design: $($kept -join ', ')"
    Copy-Item (Join-Path $env:LOCALAPPDATA "AnyFont\logs\*.log") $Results -ErrorAction SilentlyContinue
    Finish 0
} catch {
    Check "run-real.ps1 ran to the end" $false "$_ at $($_.InvocationInfo.PositionMessage)"
    Finish 2
}
