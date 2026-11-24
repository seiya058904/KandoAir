# SPDX-FileCopyrightText: KandoAir contributors
# SPDX-License-Identifier: MIT

<#
.SYNOPSIS
  Samples the memory and CPU usage of a process tree (root process + all descendants).

.DESCRIPTION
  Finds the root process(es) by name, collects all descendant processes (for KandoAir
  these are the msedgewebview2.exe processes, for Kando these are the Electron helper
  processes) and samples their private working set and private bytes. CPU usage is the
  total processor time consumed by the tree during the sampling window, relative to all
  logical processors.

  Uses Win32_PerfFormattedData_PerfProc_Process because its class and property names are
  not localized (Get-Counter paths are localized on non-English Windows).

.EXAMPLE
  ./scripts/measure.ps1 -Name KandoAir -Seconds 60
#>
param(
  [Parameter(Mandatory = $true)][string]$Name,
  [int]$Seconds = 60,
  [int]$Interval = 5,
  [switch]$Json
)

$ErrorActionPreference = 'Stop'

function Get-ProcessTree([string]$rootName) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
  # A root is a matching process whose parent is not a matching process.
  $matching = @($all | Where-Object { $_.Name -ieq "$rootName.exe" })
  $matchingIds = [System.Collections.Generic.HashSet[int]]::new()
  foreach ($m in $matching) { [void]$matchingIds.Add([int]$m.ProcessId) }
  $roots = @($matching | Where-Object { -not $matchingIds.Contains([int]$_.ParentProcessId) })
  if ($roots.Count -eq 0) { throw "No running process named '$rootName.exe' found." }

  $set = [System.Collections.Generic.HashSet[int]]::new()
  foreach ($r in $roots) { [void]$set.Add([int]$r.ProcessId) }
  $changed = $true
  while ($changed) {
    $changed = $false
    foreach ($p in $all) {
      if ($set.Contains([int]$p.ParentProcessId) -and -not $set.Contains([int]$p.ProcessId)) {
        [void]$set.Add([int]$p.ProcessId)
        $changed = $true
      }
    }
  }
  return @($all | Where-Object { $set.Contains([int]$_.ProcessId) })
}

function Get-TreeSample([int[]]$ids) {
  $idSet = [System.Collections.Generic.HashSet[int]]::new()
  foreach ($i in $ids) { [void]$idSet.Add($i) }
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $idSet.Contains([int]$_.IDProcess) }
  [pscustomobject]@{
    PrivateWorkingSetMB = [math]::Round((($perf | Measure-Object WorkingSetPrivate -Sum).Sum) / 1MB, 1)
    PrivateBytesMB      = [math]::Round((($perf | Measure-Object PrivateBytes -Sum).Sum) / 1MB, 1)
    Processes           = $perf
  }
}

$tree = Get-ProcessTree $Name
$ids = @($tree | ForEach-Object { [int]$_.ProcessId })

$cpuStart = 0.0
foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($p) { $cpuStart += $p.TotalProcessorTime.TotalMilliseconds }
}
$watch = [System.Diagnostics.Stopwatch]::StartNew()

$samples = @()
do {
  $samples += Get-TreeSample $ids
  if ($watch.Elapsed.TotalSeconds + $Interval -gt $Seconds) { break }
  Start-Sleep -Seconds $Interval
} while ($true)
$remaining = $Seconds - $watch.Elapsed.TotalSeconds
if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int]($remaining * 1000)) }

$cpuEnd = 0.0
foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if ($p) { $cpuEnd += $p.TotalProcessorTime.TotalMilliseconds }
}
$elapsedMs = $watch.Elapsed.TotalMilliseconds
$cpuPercent = [math]::Round((($cpuEnd - $cpuStart) / ($elapsedMs * [Environment]::ProcessorCount)) * 100, 3)

$last = $samples[-1]
$result = [pscustomobject]@{
  Name                   = $Name
  ProcessCount           = $ids.Count
  SampleCount            = $samples.Count
  AvgPrivateWorkingSetMB = [math]::Round(($samples | Measure-Object PrivateWorkingSetMB -Average).Average, 1)
  AvgPrivateBytesMB      = [math]::Round(($samples | Measure-Object PrivateBytesMB -Average).Average, 1)
  LastPrivateWorkingSetMB = $last.PrivateWorkingSetMB
  CpuPercentOfAllCores   = $cpuPercent
  CpuMsTotal             = [math]::Round($cpuEnd - $cpuStart, 1)
  Seconds                = [math]::Round($elapsedMs / 1000, 1)
}

if ($Json) {
  $result | ConvertTo-Json
} else {
  $result | Format-List
  $last.Processes | Sort-Object WorkingSetPrivate -Descending |
    Select-Object IDProcess, Name,
      @{ n = 'PrivateWS_MB'; e = { [math]::Round($_.WorkingSetPrivate / 1MB, 1) } },
      @{ n = 'PrivateBytes_MB'; e = { [math]::Round($_.PrivateBytes / 1MB, 1) } } |
    Format-Table -AutoSize
}
