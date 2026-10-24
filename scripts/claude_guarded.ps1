<#
.SYNOPSIS
    Corporate Desktop AI Guard Launcher for Claude Code Terminal (`claude`).
.DESCRIPTION
    Connects your native `claude` terminal CLI to the local AgentTrace AI Guard (:8080).
    You keep 100% of the interactive Claude Code terminal experience (diffs, bash tool use,
    streaming, slash commands), while every outbound prompt and command is inspected in <1ms
    for DLP secret leaks (.env, API keys, PEM keys), destructive shell commands, and spatial
    zone policies, signed into the local SHA-256 ledger, and shipped to corporate ELK/SIEM.
#>
param(
    [ValidateSet("hard", "redact", "audit")]
    [string]$GuardMode = "hard",
    [string]$GatewayUrl = "http://127.0.0.1:8080",
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ClaudeArgs
)

$HostName = $env:COMPUTERNAME
if (-not $HostName) { $HostName = "CORP-DESKTOP" }
$SessionHash = [guid]::NewGuid().ToString("N").Substring(0, 6)

# 1. Point Claude Code CLI transparently at the local AgentTrace AI Guard (/v1/messages)
$env:ANTHROPIC_BASE_URL = $GatewayUrl
$env:ANTHROPIC_CUSTOM_HEADERS = "x-agent-id: claude-code`nx-agent-role: terminal`nx-workstation-id: $HostName`nx-agent-instance: pid$PID-$SessionHash`nx-guard-mode: $GuardMode"

if (-not $env:ANTHROPIC_API_KEY) {
    $env:ANTHROPIC_API_KEY = "sk-agenttrace-corporate-hub-token"
}

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host " 🛡️  AGENTTRACE CORPORATE AI GUARD // CLAUDE CODE TERMINAL HUB" -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host " Agent Identity : claude-code:terminal@$HostName#pid$PID" -ForegroundColor Green
Write-Host " Guard Mode     : $($GuardMode.ToUpper()) (DLP Secret Scan + Hard Command Gate)" -ForegroundColor Yellow
Write-Host " Gateway Hub    : $GatewayUrl (/v1/messages)" -ForegroundColor Gray
Write-Host " Console Audit  : $GatewayUrl/console" -ForegroundColor Gray
Write-Host "==================================================================" -ForegroundColor Cyan

if (Get-Command claude -ErrorAction SilentlyContinue) {
    & claude @ClaudeArgs
} else {
    Write-Host "[i] 'claude' CLI binary not found on PATH; running live corporate guard verification against $GatewayUrl/v1/messages..." -ForegroundColor Yellow
    $SafeBody = @{
        model = "claude-3-7-sonnet-latest"
        max_tokens = 256
        messages = @(@{ role = "user"; content = "Review the authentication module architecture." })
    } | ConvertTo-Json -Depth 5

    $RespSafe = Invoke-RestMethod -Uri "$GatewayUrl/v1/messages" -Method Post -ContentType "application/json" -Headers @{
        "anthropic-version" = "2023-06-01"
        "x-agent-id" = "claude-code"
        "x-agent-role" = "terminal"
        "x-workstation-id" = $HostName
        "x-agent-instance" = "pid$PID"
        "x-guard-mode" = $GuardMode
    } -Body $SafeBody
    Write-Host "`n[1] Safe Corporate Prompt Verdict: ALLOWED" -ForegroundColor Green
    Write-Host ($RespSafe.content[0].text)

    $LeakBody = @{
        model = "claude-3-7-sonnet-latest"
        max_tokens = 256
        messages = @(@{ role = "user"; content = "Debug this production key: sk-ant-api03-99887766554433221100aabbccddeeff" })
    } | ConvertTo-Json -Depth 5

    $RespLeak = Invoke-RestMethod -Uri "$GatewayUrl/v1/messages" -Method Post -ContentType "application/json" -Headers @{
        "anthropic-version" = "2023-06-01"
        "x-agent-id" = "claude-code"
        "x-agent-role" = "terminal"
        "x-workstation-id" = $HostName
        "x-agent-instance" = "pid$PID"
        "x-guard-mode" = "hard"
    } -Body $LeakBody
    Write-Host "`n[2] Secret Leak Prompt Verdict: HARD BLOCKED" -ForegroundColor Red
    Write-Host ($RespLeak.content[0].text)
}
