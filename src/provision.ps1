# Cluster Node Full Provisioning Script (Automated Gemma 4 + Gateway)
$ErrorActionPreference = 'Stop'
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " 🚀 PROVISIONING CLUSTER WORKER NODE" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Install & Setup AgentTrace Gateway
$LeadUrl = "http://127.0.0.1:8080"
$InstallDir = Join-Path $env:LOCALAPPDATA "AgentTrace"
if (-not (Test-Path $InstallDir)) { New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null }
$BinPath = Join-Path $InstallDir "agent_trace_gw.exe"

Write-Host "[*] Fetching agent_trace_gw.exe from Lead..." -ForegroundColor Cyan
Invoke-WebRequest -Uri "$LeadUrl/dist/agent_trace_gw.exe" -OutFile $BinPath -UseBasicParsing

# 2. Check & Install Ollama Gemma 4 if missing
$OllamaInstalled = Get-Command "ollama" -ErrorAction SilentlyContinue
if (-not $OllamaInstalled) {
    Write-Host "[*] Downloading Ollama setup..." -ForegroundColor Yellow
    $OllamaInstaller = "$env:TEMP\OllamaSetup.exe"
    Invoke-WebRequest -Uri "https://ollama.com/download/OllamaSetup.exe" -OutFile $OllamaInstaller -UseBasicParsing
    Start-Process -FilePath $OllamaInstaller -ArgumentList "/silent" -Wait
    Write-Host "[+] Ollama installed successfully!" -ForegroundColor Green
}

# Pull Gemma 4 model
Write-Host "[*] Verifying/Pulling gemma4:latest in Ollama..." -ForegroundColor Cyan
Start-Process -FilePath "ollama" -ArgumentList "pull gemma4:latest" -Wait

# 3. Register Heartbeat with Lead Gateway
$MyIp = (Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -like "192.168.*" }).IPAddress | Select-Object -First 1
if (-not $MyIp) { $MyIp = "127.0.0.1" }

$Heartbeat = @{
    node_id = "PC-Worker-$($env:COMPUTERNAME)"
    hostname = $env:COMPUTERNAME
    ip_address = $MyIp
    role = "Worker Node (Gemma 4 + Edge Gateway)"
    status = "ACTIVE"
    last_seen = (Get-Date).ToUniversalTime().ToString("o")
    public_key = $null
} | ConvertTo-Json

Invoke-RestMethod -Uri "$LeadUrl/api/v1/heartbeat" -Method Post -Body $Heartbeat -ContentType "application/json" | Out-Null

# 4. Start Gateway
$Env:PORT = "8080"
$Env:UPSTREAM_URL = "http://127.0.0.1:11434"
Start-Process -FilePath $BinPath -WindowStyle Hidden

Write-Host "==========================================================" -ForegroundColor Green
Write-Host " 🎉 Worker Node Fully Provisioned and Active!" -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
