Add-Type -AssemblyName UIAutomationClient
$uia = [System.Windows.Automation.AutomationElement]
$root = $uia::RootElement
$win = $null
foreach ($name in @("RustService")) {
    $c = New-Object System.Windows.Automation.PropertyCondition ($uia::NameProperty, $name)
    $win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $c)
    if ($win) { break }
}
if (-not $win) {
    Write-Output "FAIL no window"
    exit 1
}

$tabs = @(
    "Service", "Agent", "System Info", "Component Test", "Shortcuts",
    "Programs", "Scripts", "Network Diagnostics", "Startup Manager",
    "Event Log", "Bluescreen", "Reports", "Settings"
)
$ok = @()
$fail = @()
foreach ($tab in $tabs) {
    $nameCond = New-Object System.Windows.Automation.PropertyCondition ($uia::NameProperty, $tab)
    $el = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $nameCond)
    if (-not $el) {
        $fail += $tab
        continue
    }
    $inv = $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    if ($inv) {
        $inv.Invoke()
        Start-Sleep -Milliseconds 400
        $ok += $tab
    }
    else {
        $sel = $el.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
        if ($sel) {
            $sel.Select()
            Start-Sleep -Milliseconds 400
            $ok += $tab
        }
        else {
            $fail += "$tab (no invoke)"
        }
    }
}
Write-Output "OK $($ok -join ', ')"
if ($fail.Count) { Write-Output "FAIL $($fail -join ', ')" }
exit $(if ($fail.Count) { 1 } else { 0 })
