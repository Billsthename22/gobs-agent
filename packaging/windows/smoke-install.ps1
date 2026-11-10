[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $InstallerPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# This test creates and removes a real machine service. Only run it on the
# disposable Windows runner, never on a developer's existing installation.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'The installer smoke test requires a disposable GitHub Actions Windows runner.'
}
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The installer smoke test requires an elevated runner.'
}

$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$installDirectory = Join-Path $env:ProgramFiles 'GBOS'
$agentExecutable = Join-Path $installDirectory 'GBOS-Agent.exe'
$uninstaller = Join-Path $installDirectory 'uninstall.exe'
$identityDirectory = Join-Path $env:ProgramData 'GBOS'
$identityPath = Join-Path $identityDirectory 'device-token'
$serviceKey = 'HKLM:\SYSTEM\CurrentControlSet\Services\GBOSAgent'
$firewallRule = 'GBOS-Installer-Smoke-' + [Guid]::NewGuid().ToString('N')

if ((Get-Service -Name GBOSAgent -ErrorAction SilentlyContinue) -or
    (Test-Path -LiteralPath $installDirectory) -or
    (Test-Path -LiteralPath $identityDirectory)) {
    throw 'The installer smoke test requires a clean runner without a GBOS installation.'
}
foreach ($name in @('GBOS_DEVICE_TOKEN', 'GBOS_DEVICE_TOKEN_FILE')) {
    if ([Environment]::GetEnvironmentVariable($name, 'Process') -or
        [Environment]::GetEnvironmentVariable($name, 'Machine')) {
        throw 'The smoke test must create its own machine identity.'
    }
}

$listener = $null
$installerProcess = $null
$firewallProfiles = @()
$firewallCreated = $false
$originalApiUrl = $env:GBOS_API_URL
$failure = $null
$cleanupFailed = $false

try {
    # Reserve an available loopback port before constructing HttpListener.
    $portProbe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $portProbe.Start()
    $port = $portProbe.LocalEndpoint.Port
    $portProbe.Stop()
    $apiUrl = 'http://127.0.0.1:' + $port
    $listener = [Net.HttpListener]::new()
    $listener.Prefixes.Add($apiUrl + '/')
    $listener.Start()

    # Defense in depth: even if a future change loses the service environment
    # override, this executable cannot reach the public production backend.
    # Use the supported Internet keyword rather than wildcard IP ranges, which
    # Windows rejects. Keep this guard until every agent process has stopped.
    $firewallProfiles = @(Get-NetFirewallProfile)
    Set-NetFirewallProfile -Profile @('Domain', 'Private', 'Public') -Enabled True
    New-NetFirewallRule -Name $firewallRule -DisplayName $firewallRule `
        -Direction Outbound -Action Block -Program $agentExecutable -Profile Any `
        -RemoteAddress Internet `
        -Enabled True | Out-Null
    $firewallCreated = $true
    if (@(Get-NetFirewallProfile | Where-Object { -not $_.Enabled }).Count -ne 0) {
        throw 'Could not establish network isolation for the installer smoke test.'
    }

    # Register only a stopped placeholder. Setup will install the executable
    # and reconfigure this service while retaining its loopback environment.
    $placeholder = New-Service -Name GBOSAgent -BinaryPathName ('"' + $agentExecutable + '"') -StartupType Manual
    $placeholder.Dispose()
    New-ItemProperty -Path $serviceKey -Name Environment -PropertyType MultiString `
        -Value @('GBOS_API_URL=' + $apiUrl) -Force | Out-Null
    $env:GBOS_API_URL = $apiUrl

    $pendingRequest = $listener.GetContextAsync()
    $installerProcess = Start-Process -FilePath $installer -ArgumentList '/S' -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    $registered = $false

    while ([DateTime]::UtcNow -lt $deadline) {
        if ($pendingRequest.IsCompleted) {
            $context = $pendingRequest.GetAwaiter().GetResult()
            try {
                if ($context.Request.HttpMethod -eq 'POST' -and
                    $context.Request.Url.AbsolutePath -eq '/agents/register') {
                    $reader = [IO.StreamReader]::new($context.Request.InputStream)
                    try { $payload = $reader.ReadToEnd() | ConvertFrom-Json }
                    finally { $reader.Dispose() }
                    if (-not (Test-Path -LiteralPath $identityPath -PathType Leaf)) {
                        throw 'The service attempted enrollment without saving its identity.'
                    }
                    $savedIdentity = [IO.File]::ReadAllText($identityPath).Trim()
                    if ($savedIdentity -notmatch '^[0-9a-f]{64}$') {
                        throw 'Setup did not create the expected secure machine identity.'
                    }
                    if (-not [string]::Equals(
                        $context.Request.Headers['Authorization'],
                        'Bearer ' + $savedIdentity,
                        [StringComparison]::Ordinal
                    )) {
                        throw 'Enrollment did not authenticate with the saved machine identity.'
                    }
                    if ($payload.auto_enroll -isnot [bool] -or -not $payload.auto_enroll -or
                        $payload.hostname -ne $env:COMPUTERNAME -or
                        $payload.device_name -ne $env:COMPUTERNAME -or
                        [string]::IsNullOrWhiteSpace($payload.operating_system)) {
                        throw 'Enrollment did not supply automatic enrollment and the actual machine hostname.'
                    }

                    $body = @{
                        device_id = 1
                        device_name = $payload.device_name
                        hostname = $payload.hostname
                        status = 'online'
                        websocket_url = $apiUrl + '/ws/devices/1'
                    } | ConvertTo-Json -Compress
                    $bytes = [Text.Encoding]::UTF8.GetBytes($body)
                    $context.Response.StatusCode = 200
                    $context.Response.ContentType = 'application/json'
                    $context.Response.ContentLength64 = $bytes.Length
                    $context.Response.OutputStream.Write($bytes, 0, $bytes.Length)
                    $registered = $true
                    # Never include the credential or Authorization header in
                    # console output, assertions, or artifacts.
                    $savedIdentity = $null
                } else {
                    # Do not start a monitoring session on the CI runner.
                    $context.Response.StatusCode = 404
                }
            } finally {
                $context.Response.Close()
            }
            $pendingRequest = $listener.GetContextAsync()
        }

        $installerProcess.Refresh()
        if ($installerProcess.HasExited) {
            if ($installerProcess.ExitCode -ne 0) {
                throw ('The installer failed with exit code ' + $installerProcess.ExitCode + '.')
            }
            $service = Get-Service -Name GBOSAgent -ErrorAction SilentlyContinue
            if ($registered -and $service -and $service.Status -eq 'Running') {
                $service.Dispose()
                Write-Host 'Windows setup passed: service running, identity saved, automatic enrollment authenticated.'
                break
            }
            if ($service) { $service.Dispose() }
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $registered -or -not $installerProcess.HasExited) {
        throw 'Windows setup did not complete and enroll automatically within 90 seconds.'
    }
    $running = Get-Service -Name GBOSAgent
    try {
        if ($running.Status -ne 'Running') { throw 'The installed service is not running.' }
    } finally { $running.Dispose() }
} catch {
    $failure = $_
} finally {
    if ($installerProcess -and -not $installerProcess.HasExited) {
        $installerProcess.Kill()
        $installerProcess.WaitForExit()
    }
    if ($listener) { $listener.Close() }

    # Always exercise the real uninstaller. _?= avoids its temporary child
    # process so WaitForExit observes completion of the actual uninstall.
    try {
        if (Test-Path -LiteralPath $uninstaller) {
            $removal = Start-Process -FilePath $uninstaller -ArgumentList ('/S _?=' + $installDirectory) -PassThru
            if (-not $removal.WaitForExit(30000)) {
                $removal.Kill()
                throw 'The GBOS uninstaller timed out.'
            }
            if ($removal.ExitCode -ne 0) { throw 'The GBOS uninstaller failed.' }
            $removal.Dispose()
        }
    } catch {
        $cleanupFailed = $true
        Write-Warning $_.Exception.Message
    }

    try {
        $remainingService = Get-Service -Name GBOSAgent -ErrorAction SilentlyContinue
        if ($remainingService) {
            if ($remainingService.Status -ne 'Stopped') {
                Stop-Service -InputObject $remainingService -Force
                $remainingService.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(20))
            }
            $remainingService.Dispose()
            & sc.exe delete GBOSAgent | Out-Null
        }
        Get-Process -Name GBOS-Agent -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $agentExecutable } | Stop-Process -Force
        if (Test-Path -LiteralPath $installDirectory) {
            Remove-Item -LiteralPath $installDirectory -Recurse -Force
        }
        if (Test-Path -LiteralPath $identityDirectory) {
            Remove-Item -LiteralPath $identityDirectory -Recurse -Force
        }
        if ($firewallCreated) { Remove-NetFirewallRule -Name $firewallRule }
        foreach ($profile in $firewallProfiles) {
            Set-NetFirewallProfile -Profile $profile.Name -Enabled $profile.Enabled
        }
    } catch {
        # Retain network isolation if an agent could still be running. The
        # disposable runner will be destroyed after the workflow finishes.
        $cleanupFailed = $true
        Write-Warning 'Smoke test cleanup failed; network isolation remains in place.'
    }
    $env:GBOS_API_URL = $originalApiUrl
    if ($installerProcess) { $installerProcess.Dispose() }
}

if ($failure) { throw $failure }
if ($cleanupFailed) { throw 'The Windows installer smoke test could not finish cleanup.' }
