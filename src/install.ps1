# AgentTrace Cluster Node Auto-Deployer
$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " 🌐 AGENTTRACE CLUSTER NODE AUTO-DEPLOYER" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Determine Lead Gateway URL (defaults to server that hosted this script)
$LeadHost = "192.168.1.101"
$LeadPort = "8080"
$LeadUrl = "http://$($LeadHost):$($LeadPort)"
Write-Host "[+] Connecting to Lead Gateway: $LeadUrl" -ForegroundColor Yellow

# 2. Setup target install directory
$InstallDir = "C:\cada\agent_trace_gw"
if (-not (Test-Path $InstallDir)) {
    New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null
}

# 3. Download the Gateway executable from Lead Node
$BinPath = Join-Path $InstallDir "agent_trace_gw.exe"
Write-Host "[*] Downloading latest agent_trace_gw.exe from Lead..." -ForegroundColor Cyan
try {
    # Stop existing instance if running
    Get-Process -Name "agent_trace_gw" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
} catch {}

Invoke-WebRequest -Uri "$LeadUrl/dist/agent_trace_gw.exe" -OutFile $BinPath -UseBasicParsing
Write-Host "[+] Download complete: $BinPath" -ForegroundColor Green

# 4. Detect local LLM service (Hermes / Ollama)
$LocalPort = 8642
$HermesCheck = Test-NetConnection -ComputerName 127.0.0.1 -Port 8642 -WarningAction SilentlyContinue
if ($HermesCheck.TcpTestSucceeded) {
    Write-Host "[+] Detected local hermes-agent on :8642 (NVIDIA CUDA)" -ForegroundColor Green
    $LocalPort = 8642
} else {
    $OllamaCheck = Test-NetConnection -ComputerName 127.0.0.1 -Port 11434 -WarningAction SilentlyContinue
    if ($OllamaCheck.TcpTestSucceeded) {
        Write-Host "[+] Detected local Ollama on :11434" -ForegroundColor Green
        $LocalPort = 11434
    } else {
        Write-Host "[!] No local LLM on :8642 or :11434, defaulting to :8642" -ForegroundColor Yellow
    }
}
$LocalUpstream = "http://127.0.0.1:$LocalPort"

# 5. Register Heartbeat with Lead Node
$HostName = $env:COMPUTERNAME
$Ip = (Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias -notlike "*Loopback*" -and $_.IPAddress -like "192.168.*" }).IPAddress | Select-Object -First 1
if (-not $Ip) { $Ip = "192.168.1.102" }

$NodeId = if ($Ip -eq "192.168.1.102" -or $HostName -match "NODE-02") { "PC-2" } else { "Worker-$HostName" }

$Heartbeat = @{
    node_id = $NodeId
    hostname = $HostName
    ip_address = $Ip
    role = "Worker Node (Edge Proxy -> :$LocalPort)"
    status = "ACTIVE"
    last_seen = (Get-Date).ToUniversalTime().ToString("o")
    public_key = $null
} | ConvertTo-Json

Write-Host "[*] Registering node $NodeId ($Ip) with Lead Cluster..." -ForegroundColor Cyan
Invoke-RestMethod -Uri "$LeadUrl/api/v1/heartbeat" -Method Post -Body $Heartbeat -ContentType "application/json" | Out-Null
Write-Host "[+] Node registered in cluster topology!" -ForegroundColor Green

# 6. Launch Gateway in Worker Mode
Write-Host "[+] Launching local edge gateway on port 8080..." -ForegroundColor Green
$procInfo = New-Object System.Diagnostics.ProcessStartInfo
$procInfo.FileName = $BinPath
$procInfo.WorkingDirectory = $InstallDir
$procInfo.Arguments = ""
$procInfo.EnvironmentVariables["PORT"] = "8080"
$procInfo.EnvironmentVariables["UPSTREAM_URL"] = $LocalUpstream
$procInfo.UseShellExecute = $false
$procInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden

[System.Diagnostics.Process]::Start($procInfo) | Out-Null

Write-Host "==========================================================" -ForegroundColor Green
Write-Host " 🎉 SUCCESS: Node $NodeId is deployed and active in the mesh!" -ForegroundColor Green
Write-Host " Local proxy active: http://localhost:8080" -ForegroundColor Green
Write-Host " Upstream target:    $LocalUpstream" -ForegroundColor Green
Write-Host " Cluster Lead:       $LeadUrl" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
