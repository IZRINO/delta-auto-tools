# setup-update-key.ps1
# 一次性脚本：生成 Tauri 更新签名密钥对，并将公钥写入 tauri.conf.json
# 私钥保存到 $HOME/.tauri/delta-auto-tools.key（不入库）

param(
    [string]$KeyPath = "$env:USERPROFILE\.tauri\delta-auto-tools.key",
    [string]$PubKeyPath = "$env:USERPROFILE\.tauri\delta-auto-tools.pub"
)

$ErrorActionPreference = "Stop"

# 确保目录存在
$dir = Split-Path $KeyPath -Parent
if (-not (Test-Path $dir)) {
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
}

# 检查密钥是否已存在
if (Test-Path $KeyPath) {
    Write-Host "私钥已存在: $KeyPath" -ForegroundColor Yellow
    Write-Host "跳过密钥生成。如需重新生成，请先删除旧密钥文件。" -ForegroundColor Yellow
} else {
    Write-Host "正在生成签名密钥对..." -ForegroundColor Cyan
    bunx --offline tauri signer generate -w $KeyPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "密钥生成失败。请确认已安装 @tauri-apps/cli" -ForegroundColor Red
        exit 1
    }
    Write-Host "密钥生成成功" -ForegroundColor Green
}

# 读取公钥
if (Test-Path $PubKeyPath) {
    $pubkey = Get-Content $PubKeyPath -Raw
} elseif (Test-Path "$KeyPath.pub") {
    $pubkey = Get-Content "$KeyPath.pub" -Raw
} else {
    Write-Host "未找到公钥文件（$PubKeyPath 或 $KeyPath.pub），跳过 tauri.conf.json 更新" -ForegroundColor Yellow
    Write-Host "请手动将公钥内容填入 src-tauri/tauri.conf.json 的 plugins.updater.pubkey 字段" -ForegroundColor Yellow
    exit 0
}

$pubkey = $pubkey.Trim()

# 更新 tauri.conf.json
$confPath = "src-tauri/tauri.conf.json"
if (Test-Path $confPath) {
    $conf = Get-Content $confPath -Raw | ConvertFrom-Json
    if (-not $conf.plugins) {
        $conf | Add-Member -NotePropertyName "plugins" -NotePropertyValue ([PSCustomObject]@{}) -Force
    }
    if (-not $conf.plugins.updater) {
        $conf.plugins | Add-Member -NotePropertyName "updater" -NotePropertyValue ([PSCustomObject]@{}) -Force
    }
    $conf.plugins.updater.pubkey = $pubkey
    $conf | ConvertTo-Json -Depth 10 | Set-Content $confPath -Encoding UTF8
    Write-Host "已更新 $confPath 中的公钥" -ForegroundColor Green
} else {
    Write-Host "未找到 $confPath，跳过配置更新" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "==== 重要提醒 ====" -ForegroundColor Cyan
Write-Host "1. 私钥位于: $KeyPath — 请妥善保管，绝不要提交到版本控制" -ForegroundColor Yellow
Write-Host "2. 构建发布版时需设置环境变量:" -ForegroundColor Yellow
Write-Host "   `$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content '$KeyPath' -Raw" -ForegroundColor White
Write-Host "3. 如设置了密码，也需设置:" -ForegroundColor Yellow
Write-Host "   `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = 'your-password'" -ForegroundColor White
