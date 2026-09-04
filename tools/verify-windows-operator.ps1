# SPITFIRE NG
# Preservation-driven modern cross-platform reimplementation of
# Buffalo Creek Software's SPITFIRE Bulletin Board System
#
# Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
# Licensed under MIT OR Apache-2.0
#
# This file is part of the SPITFIRE NG project.
# See the repository documentation for architecture, provenance,
# compatibility research, security, and contribution guidelines.

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Spitfire {
    param(
        [Parameter(Mandatory = $true)] [string] $Executable,
        [Parameter(Mandatory = $true)] [string[]] $Arguments
    )

    $output = & $Executable @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "spitfire command failed: $($Arguments -join ' '): $($output -join [Environment]::NewLine)"
    }
    return $output
}

function Start-BoardDaemon {
    param(
        [Parameter(Mandatory = $true)] [string] $Executable,
        [Parameter(Mandatory = $true)] [string] $Configuration,
        [Parameter(Mandatory = $true)] [string] $OutputPath,
        [Parameter(Mandatory = $true)] [string] $ErrorPath
    )

    return Start-Process -FilePath $Executable `
        -ArgumentList @("run", $Configuration) `
        -RedirectStandardOutput $OutputPath `
        -RedirectStandardError $ErrorPath `
        -PassThru
}

function Wait-ForOperator {
    param(
        [Parameter(Mandatory = $true)] [string] $Executable,
        [Parameter(Mandatory = $true)] [string] $Configuration,
        [Parameter(Mandatory = $true)] [System.Diagnostics.Process] $Daemon
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($Daemon.HasExited) {
            throw "board daemon exited before the operator endpoint became ready"
        }
        $output = & $Executable operator status $Configuration 2>&1
        if ($LASTEXITCODE -eq 0) {
            return $output
        }
        Start-Sleep -Milliseconds 100
    }
    throw "operator endpoint did not become ready within 30 seconds"
}

function Stop-TestProcess {
    param([System.Diagnostics.Process] $Process)

    if ($null -ne $Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit()
    }
}

$repository = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $repository "target\debug\spitfire.exe"
$testRoot = Join-Path $env:PUBLIC ("spitfire-ng-b021aw-" + [Guid]::NewGuid().ToString("N"))
$boardRoot = Join-Path $testRoot "board"
$configuration = Join-Path $boardRoot "spitfire.toml"
$daemonOutput = Join-Path $testRoot "daemon.out"
$daemonError = Join-Path $testRoot "daemon.err"
$daemon = $null
$callerOne = $null
$callerTwo = $null
$watchOne = $null
$watchTwo = $null
$testUser = "sfng" + [Guid]::NewGuid().ToString("N").Substring(0, 12)
$testUserCreated = $false

try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    cargo build --locked -p sf-bbs
    if ($LASTEXITCODE -ne 0) {
        throw "Windows sf-bbs build failed"
    }
    Invoke-Spitfire $executable @("init-fixture", $boardRoot) | Out-Null

    $configurationText = [IO.File]::ReadAllText($configuration)
    $configurationText = $configurationText.Replace("count = 1", "count = 2")
    [IO.File]::WriteAllText($configuration, $configurationText)
    $currentSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if ($configurationText -notmatch [Regex]::Escape($currentSid)) {
        throw "setup did not bootstrap the creating Windows SID"
    }

    $daemon = Start-BoardDaemon $executable $configuration $daemonOutput $daemonError
    $status = Wait-ForOperator $executable $configuration $daemon
    if (($status -join "`n") -notmatch "Schema: 19") {
        throw "operator status did not report schema 19"
    }

    foreach ($action in @("nodes", "events", "notifications", "statistics", "callers", "maintenance")) {
        Invoke-Spitfire $executable @("operator", $action, $configuration) | Out-Null
    }

    $callerOne = [Net.Sockets.TcpClient]::new("127.0.0.1", 2323)
    $callerTwo = [Net.Sockets.TcpClient]::new("127.0.0.1", 2324)
    Start-Sleep -Milliseconds 500
    $nodes = Invoke-Spitfire $executable @("operator", "nodes", $configuration)
    if ((($nodes -join "`n") | Select-String -Pattern "Node " -AllMatches).Matches.Count -lt 2) {
        throw "two real caller connections were not visible in the node projection"
    }

    $callerTwo.Dispose()
    $callerTwo = $null
    Start-Sleep -Milliseconds 250
    $watchOneOutput = Join-Path $testRoot "watch-one.out"
    $watchOneError = Join-Path $testRoot "watch-one.err"
    $watchTwoOutput = Join-Path $testRoot "watch-two.out"
    $watchTwoError = Join-Path $testRoot "watch-two.err"
    $watchOne = Start-Process -FilePath $executable `
        -ArgumentList @("operator", "watch-events", $configuration) `
        -RedirectStandardOutput $watchOneOutput -RedirectStandardError $watchOneError -PassThru
    $watchTwo = Start-Process -FilePath $executable `
        -ArgumentList @("operator", "watch-events", $configuration) `
        -RedirectStandardOutput $watchTwoOutput -RedirectStandardError $watchTwoError -PassThru
    Start-Sleep -Milliseconds 1000
    $callerTwo = [Net.Sockets.TcpClient]::new("127.0.0.1", 2324)
    $activity = [Text.Encoding]::ASCII.GetBytes(
        "Y`r`nIncrement Two Demo`r`ntest-only-demo-password`r`n" +
        "test-only-demo-password`r`nG`r`n"
    )
    $callerTwo.GetStream().Write($activity, 0, $activity.Length)
    $callerTwo.GetStream().Flush()
    $watchOne.WaitForExit(10000) | Out-Null
    $watchTwo.WaitForExit(10000) | Out-Null
    if (-not $watchOne.HasExited -or -not $watchTwo.HasExited -or
        $watchOne.ExitCode -ne 0 -or $watchTwo.ExitCode -ne 0) {
        throw "concurrent operator event subscriptions did not complete independently"
    }
    if (([IO.File]::ReadAllText($watchOneOutput) -notmatch "Event ") -or
        ([IO.File]::ReadAllText($watchTwoOutput) -notmatch "Event ")) {
        throw "live operator subscriptions did not receive generated caller activity"
    }

    $abruptOutput = Join-Path $testRoot "abrupt.out"
    $abruptError = Join-Path $testRoot "abrupt.err"
    $abrupt = Start-Process -FilePath $executable `
        -ArgumentList @("operator", "watch-events", $configuration) `
        -RedirectStandardOutput $abruptOutput -RedirectStandardError $abruptError -PassThru
    Start-Sleep -Milliseconds 100
    Stop-TestProcess $abrupt
    Invoke-Spitfire $executable @("operator", "status", $configuration) | Out-Null

    $passwordPlain = "Sfng!" + [Guid]::NewGuid().ToString("N") + "9"
    $password = ConvertTo-SecureString $passwordPlain -AsPlainText -Force
    New-LocalUser -Name $testUser -Password $password -PasswordNeverExpires | Out-Null
    $testUserCreated = $true
    $credential = [Management.Automation.PSCredential]::new(".\$testUser", $password)
    $deniedOutput = Join-Path $testRoot "denied.out"
    $deniedError = Join-Path $testRoot "denied.err"
    $denied = Start-Process -FilePath $executable `
        -ArgumentList @("operator", "status", $configuration) `
        -Credential $credential `
        -RedirectStandardOutput $deniedOutput `
        -RedirectStandardError $deniedError `
        -Wait -PassThru
    if ($denied.ExitCode -eq 0) {
        throw "an unlisted Windows principal attached to the protected operator pipe"
    }
    $denialText = ([IO.File]::ReadAllText($deniedOutput) + "`n" +
        [IO.File]::ReadAllText($deniedError))
    if ($denialText -notmatch "not authorized to operate this board") {
        throw "the unlisted-principal process failed for an unexpected reason"
    }

    $callerOne.Dispose()
    $callerOne = $null
    $callerTwo.Dispose()
    $callerTwo = $null
    Stop-TestProcess $daemon
    $daemon = Start-BoardDaemon $executable $configuration $daemonOutput $daemonError
    Wait-ForOperator $executable $configuration $daemon | Out-Null

    Write-Output "B021-AW Windows acceptance: PASS"
    Write-Output "setup SID bootstrap: PASS"
    Write-Output "protected named-pipe attach and read projections: PASS"
    Write-Output "two callers and concurrent operator subscriptions: PASS"
    Write-Output "unauthorized Windows principal ACL denial: PASS"
    Write-Output "daemon restart and reattach: PASS"
}
finally {
    if ($null -ne $callerOne) { $callerOne.Dispose() }
    if ($null -ne $callerTwo) { $callerTwo.Dispose() }
    Stop-TestProcess $watchOne
    Stop-TestProcess $watchTwo
    Stop-TestProcess $daemon
    if ($testUserCreated) {
        Remove-LocalUser -Name $testUser -ErrorAction SilentlyContinue
    }
    if (Test-Path $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
