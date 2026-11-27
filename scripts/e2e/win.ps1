# Lmux 창을 최소화(6)·복원(9)하거나 닫는다. 사용: win.ps1 [-Show <코드>] [-Close] [-CloseDialog]
param([int]$Show = -1, [switch]$Close, [switch]$CloseDialog)
Add-Type -Namespace W -Name U -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool ShowWindowAsync(System.IntPtr h, int c);
[DllImport("user32.dll")] public static extern bool IsIconic(System.IntPtr h);
[DllImport("user32.dll")] public static extern bool PostMessage(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
public delegate bool Callback(System.IntPtr h, System.IntPtr p);
[DllImport("user32.dll")] public static extern bool EnumWindows(Callback cb, System.IntPtr p);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(System.IntPtr h);
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(System.IntPtr h, System.Text.StringBuilder s, int n);
'@
$p = Get-Process Lmux
# 최소화하면 MainWindowHandle이 다른 창을 가리키므로 처음 값을 보관한다.
$dir = if ($env:OUT) { $env:OUT } else { $env:TEMP }
$cache = Join-Path $dir "hwnd-$($p.Id).txt"
if (-not (Test-Path $cache)) { "$($p.MainWindowHandle)" | Set-Content $cache }
$h = [IntPtr][int64](Get-Content $cache)
$was = [W.U]::IsIconic($h)
if ($Show -ge 0) { [W.U]::ShowWindowAsync($h, $Show) | Out-Null; Start-Sleep -Milliseconds 300 }
"cached=$h current=$($p.MainWindowHandle) was_iconic=$was iconic=$([W.U]::IsIconic($h))"
if ($CloseDialog) {
    $script:dialogs = @()
    [void][W.U]::EnumWindows({
            param($w, $unused)
            $owner = 0
            [void][W.U]::GetWindowThreadProcessId($w, [ref]$owner)
            $class = New-Object Text.StringBuilder 64
            [void][W.U]::GetClassName($w, $class, 64)
            if ($owner -eq $p.Id -and "$class" -eq '#32770' -and [W.U]::IsWindowVisible($w)) { $script:dialogs += $w }
            $true
        }, [IntPtr]::Zero)
    foreach ($w in $script:dialogs) { [W.U]::PostMessage($w, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }
    "dialog closed: $($script:dialogs.Count)"
}
if ($Close) {
    $kids = @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($p.Id)")
    "children: " + (($kids | Where-Object Name -ne msedgewebview2.exe | ForEach-Object Name) -join ",")
    "mem MB: " + [math]::Round($p.WorkingSet64 / 1MB, 1)
    # CloseMainWindow()는 최소화 중에 다른 창을 닫으려 하므로 보관한 창에 WM_CLOSE를 보낸다.
    [W.U]::PostMessage($h, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    "exited: " + $p.WaitForExit(10000)
    Start-Sleep 2
    "survivors: " + @($kids | Where-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue }).Count
}
