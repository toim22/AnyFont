<#
.SYNOPSIS
  Writes THIRD-PARTY-NOTICES.txt: the open-source components that go into anyfont.exe, with their licences.

.DESCRIPTION
  Reads the dependency graph from `cargo metadata` (the Windows x64 target), walks every crate anyfont.exe links
  (normal dependencies only: build tools, dev-dependencies and test crates are left out) and lists each one with its
  version, licence and repository. The workspace's own crates (anyfont, anyfont-*) are left out.

  The same list as AnyVector's `cargo run -p xtask -- licenses`, as a script so it needs no xtask subcommand.
  build.bat runs it before every release build; the installer copies the file to {app}\legal and the program shows
  it in About ▸ Legal Notices.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\licenses.ps1

.EXAMPLE
  # also refresh the copy the program embeds (crates\anyfont\assets\legal\third-party-licenses.txt)
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\licenses.ps1 -AssetCopy
#>
param(
  # the workspace root (the folder with the root Cargo.toml); defaults to this script's parent folder
  [string]$Root = (Split-Path -Parent $PSScriptRoot),
  # the binary crate whose dependencies are listed
  [string]$App = 'anyfont',
  # where the list goes; defaults to <Root>\THIRD-PARTY-NOTICES.txt
  [string]$OutFile = '',
  # pass --locked to cargo (CI: fail instead of updating Cargo.lock)
  [switch]$Locked,
  # also write crates\<App>\assets\legal\third-party-licenses.txt
  [switch]$AssetCopy
)

$ErrorActionPreference = 'Stop'
if (-not $OutFile) { $OutFile = Join-Path $Root 'THIRD-PARTY-NOTICES.txt' }

# Code from other projects kept inside AnyFont's own crates: name, licence, where it comes from. None yet.
$Bundled = @()

$cargoArgs = @('metadata', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc',
               '--manifest-path', (Join-Path $Root 'Cargo.toml'))
if ($Locked) { $cargoArgs += '--locked' }
$json = & cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed (exit code $LASTEXITCODE)" }
$meta = ($json -join "`n") | ConvertFrom-Json

$packages = @{}
foreach ($p in $meta.packages) { $packages[$p.id] = $p }
$nodes = @{}
foreach ($n in $meta.resolve.nodes) { $nodes[$n.id] = $n }
$appPkg = $meta.packages | Where-Object { $_.name -eq $App } | Select-Object -First 1
if (-not $appPkg) { throw "$App not found in the workspace" }

# everything the program links: normal dependencies only (dep_kinds with kind = null)
$seen = @{}
$stack = New-Object System.Collections.Stack
$stack.Push($appPkg.id)
while ($stack.Count -gt 0) {
  $id = $stack.Pop()
  if ($seen.ContainsKey($id)) { continue }
  $seen[$id] = $true
  $node = $nodes[$id]
  if (-not $node) { continue }
  foreach ($d in $node.deps) {
    $normal = $false
    foreach ($k in $d.dep_kinds) { if ($null -eq $k.kind) { $normal = $true } }
    if ($normal) { $stack.Push($d.pkg) }
  }
}

$byName = New-Object 'System.Collections.Generic.SortedDictionary[string,object]' ([StringComparer]::Ordinal)
foreach ($id in $seen.Keys) {
  $p = $packages[$id]
  # the workspace's own crates
  if ($p.name -eq $App -or $p.name -like "$App-*") { continue }
  $licence = if ($p.license) { $p.license } elseif ($p.license_file) { "see $($p.license_file)" } else { 'unspecified' }
  $repo = if ($p.repository) { $p.repository } elseif ($p.homepage) { $p.homepage } else { '' }
  $key = "$($p.name) $($p.version)"
  $byName[$key.ToLowerInvariant()] = @($key, $licence, $repo)
}
foreach ($b in $Bundled) { $byName[$b[0].ToLowerInvariant()] = $b }

$counts = New-Object 'System.Collections.Generic.SortedDictionary[string,int]' ([StringComparer]::Ordinal)
foreach ($v in $byName.Values) {
  if ($counts.ContainsKey($v[1])) { $counts[$v[1]]++ } else { $counts[$v[1]] = 1 }
}

$sb = New-Object System.Text.StringBuilder
[void]$sb.Append("AnyFont is built with the open-source components listed below.`n")
[void]$sb.Append("Each one is used under the license shown next to it. The license texts`n")
[void]$sb.Append("are published with each component at the address given.`n`n")
[void]$sb.Append("$($byName.Count) components. Licenses used:`n")
foreach ($c in $counts.GetEnumerator()) { [void]$sb.Append("  $($c.Key): $($c.Value)`n") }
[void]$sb.Append("`n")
foreach ($v in $byName.Values) {
  [void]$sb.Append("$($v[0])`n    License: $($v[1])`n")
  if ($v[2]) { [void]$sb.Append("    $($v[2])`n") }
}

$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($OutFile, $sb.ToString(), $utf8)
Write-Host "wrote $OutFile ($($byName.Count) components)"
if ($AssetCopy) {
  $dir = Join-Path $Root "crates\$App\assets\legal"
  New-Item -ItemType Directory -Force $dir | Out-Null
  $copy = Join-Path $dir 'third-party-licenses.txt'
  [IO.File]::WriteAllText($copy, $sb.ToString(), $utf8)
  Write-Host "wrote $copy"
}
