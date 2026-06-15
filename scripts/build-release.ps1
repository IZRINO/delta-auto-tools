# build-release.ps1
# 用 Tauri 私钥签名构建桌面发布包
# 必须在项目根目录运行

$ErrorActionPreference = "Stop"
$ProjectRoot = (Get-Location).Path

# 加载私钥
$keyPath = "$env:USERPROFILE\.tauri\delta-auto-tools.key"
if (-not (Test-Path $keyPath)) {
    Write-Host "未找到私钥: $keyPath" -ForegroundColor Red
    Write-Host "请先运行 scripts\setup-update-key.ps1 生成密钥" -ForegroundColor Yellow
    exit 1
}

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

Write-Host "开始 tauri release 构建（带签名）..." -ForegroundColor Cyan
bun run tauri build 2>&1 | Tee-Object -FilePath "$ProjectRoot\build-output.log" | Select-Object -Last 30
if ($LASTEXITCODE -ne 0) {
    Write-Host "构建失败，请查看 build-output.log" -ForegroundColor Red
    exit 1
}
Write-Host "构建完成" -ForegroundColor Green
