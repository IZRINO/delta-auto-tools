# generate-latest-json.ps1
# 从 tauri build 产物生成 latest.json，用于 Tauri updater 静态端点
# 用法：在 bun run tauri build 成功后运行
# 发布时上传：gh release upload v<version> latest.json --repo IZRINO/delta-auto-tools --clobber

param(
    [string]$Version = "",
    [string]$BundleDir = "src-tauri\target\release\bundle",
    [string]$OutputPath = "src-tauri\target\release\bundle\latest.json"
)

$ErrorActionPreference = "Stop"

# 自动读取版本号
if (-not $Version) {
    $cargo = Get-Content "src-tauri\Cargo.toml" -Raw
    if ($cargo -match 'version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        Write-Host "无法从 Cargo.toml 读取版本号" -ForegroundColor Red
        exit 1
    }
}

Write-Host "版本号: $Version" -ForegroundColor Cyan

# 精确匹配当前版本号的 NSIS 安装包（避免被历史产物干扰）
$nsisExe = Get-ChildItem -Path $BundleDir\nsis -Filter "delta-auto-tools_${Version}_x64-setup.exe" | Select-Object -First 1
if (-not $nsisExe) {
    Write-Host "未找到 $Version 的 NSIS 安装包: $BundleDir\nsis\delta-auto-tools_${Version}_x64-setup.exe" -ForegroundColor Red
    Write-Host "请先运行 bun run tauri build" -ForegroundColor Yellow
    exit 1
}

$nsisSigPath = "$($nsisExe.FullName).sig"
if (-not (Test-Path $nsisSigPath)) {
    Write-Host "未找到 NSIS 签名文件: $nsisSigPath" -ForegroundColor Red
    Write-Host "请确认构建时已设置 `$env:TAURI_SIGNING_PRIVATE_KEY" -ForegroundColor Yellow
    exit 1
}

$nsisSig = (Get-Content $nsisSigPath -Raw).Trim()

# 读取 MSI 签名文件（可选）
$msiFile = Get-ChildItem -Path $BundleDir\msi -Filter "delta-auto-tools_${Version}_x64_en-US.msi" | Select-Object -First 1
$msiSig = ""
$msiUrl = ""
if ($msiFile) {
    $msiSigPath = "$($msiFile.FullName).sig"
    if (Test-Path $msiSigPath) {
        $msiSig = (Get-Content $msiSigPath -Raw).Trim()
        $msiUrl = "https://github.com/IZRINO/delta-auto-tools/releases/latest/download/$($msiFile.Name)"
    }
}

$nsisFileName = $nsisExe.Name
$nsisUrl = "https://github.com/IZRINO/delta-auto-tools/releases/latest/download/$nsisFileName"

# 构建 latest.json
$releaseUrl = "https://github.com/IZRINO/delta-auto-tools/releases/tag/v$Version"

$latest = @{
    version   = $Version
    notes     = "详见 $releaseUrl"
    pub_date  = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    platforms = @{
        "windows-x86_64" = @{
            signature = $nsisSig
            url       = $nsisUrl
        }
    }
}

if ($msiSig -and $msiUrl) {
    # Tauri v2 格式不支持同一平台多个 URL，NSIS 优先
    # 如需 MSI 路径，请在 platforms 中添加额外条目
}

$json = $latest | ConvertTo-Json -Depth 5
$json | Set-Content -Path $OutputPath -Encoding UTF8NoBOM

Write-Host "已生成: $OutputPath" -ForegroundColor Green
Write-Host ""
Write-Host "发布时上传命令:" -ForegroundColor Cyan
Write-Host "gh release upload v$Version $OutputPath --repo IZRINO/delta-auto-tools --clobber" -ForegroundColor White
