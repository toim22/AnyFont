# Prints the processes that hold a file open (Restart Manager), one per line: "<pid> <name> (<service>)".
# Used by `anyfont.exe /SelfTest:` inside Windows Sandbox to explain an "in use" font.
param([Parameter(Mandatory = $true)][string]$Path)
$ErrorActionPreference = "Stop"
Add-Type -Namespace Holders -Name Rm -MemberDefinition @'
[StructLayout(LayoutKind.Sequential)] public struct RM_UNIQUE_PROCESS { public int dwProcessId; public System.Runtime.InteropServices.ComTypes.FILETIME ProcessStartTime; }
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] public struct RM_PROCESS_INFO {
    public RM_UNIQUE_PROCESS Process;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string strAppName;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string strServiceShortName;
    public int ApplicationType; public uint AppStatus; public uint TSSessionId; [MarshalAs(UnmanagedType.Bool)] public bool bRestartable; }
[DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)] public static extern int RmStartSession(out uint pSessionHandle, int dwSessionFlags, string strSessionKey);
[DllImport("rstrtmgr.dll")] public static extern int RmEndSession(uint pSessionHandle);
[DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode)] public static extern int RmRegisterResources(uint pSessionHandle, uint nFiles, string[] rgsFilenames, uint nApplications, RM_UNIQUE_PROCESS[] rgApplications, uint nServices, string[] rgsServiceNames);
[DllImport("rstrtmgr.dll")] public static extern int RmGetList(uint dwSessionHandle, out uint pnProcInfoNeeded, ref uint pnProcInfo, [In, Out] RM_PROCESS_INFO[] rgAffectedApps, ref uint lpdwRebootReasons);
'@
$h = 0
$key = [Guid]::NewGuid().ToString()
if ([Holders.Rm]::RmStartSession([ref]$h, 0, $key) -ne 0) { "RmStartSession failed"; exit 1 }
try {
    [void][Holders.Rm]::RmRegisterResources($h, 1, @($Path), 0, $null, 0, $null)
    $needed = 0; $n = 0; $reasons = 0
    $r = [Holders.Rm]::RmGetList($h, [ref]$needed, [ref]$n, $null, [ref]$reasons)
    if ($needed -eq 0) { "nobody"; exit 0 }
    $n = $needed
    $info = New-Object 'Holders.Rm+RM_PROCESS_INFO[]' $n
    $r = [Holders.Rm]::RmGetList($h, [ref]$needed, [ref]$n, $info, [ref]$reasons)
    if ($r -ne 0) { "RmGetList failed: $r"; exit 1 }
    for ($i = 0; $i -lt $n; $i++) {
        $p = $info[$i]
        $svc = if ($p.strServiceShortName) { " (service $($p.strServiceShortName))" } else { "" }
        "$($p.Process.dwProcessId) $($p.strAppName)$svc"
    }
} finally { [void][Holders.Rm]::RmEndSession($h) }
