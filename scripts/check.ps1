$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-Step {
    param(
        [Parameter(Mandatory)]
        [string]$Name,
        [Parameter(Mandatory)]
        [scriptblock]$Command
    )

    Write-Host "`n==> $Name"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    Invoke-Step "TypeScript" { bun node_modules/typescript/bin/tsc --noEmit --pretty false }
    Invoke-Step "Frontend tests" { node node_modules/vitest/vitest.mjs run --reporter=dot }
    Invoke-Step "Frontend coverage" { node node_modules/vitest/vitest.mjs run --coverage --reporter=dot }
    Invoke-Step "Rust format" { cargo fmt --manifest-path src-tauri/Cargo.toml -- --check }
    Invoke-Step "Rust Clippy" { cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings }
    Invoke-Step "Rust tests" { cargo test --manifest-path src-tauri/Cargo.toml }
}
finally {
    Pop-Location
}
