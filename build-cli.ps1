$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo build --locked --release --bin seex-cli
    if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
    New-Item -ItemType Directory -Path dist -Force | Out-Null
    Copy-Item -LiteralPath target/release/seex-cli.exe -Destination dist/seex-cli.exe
    Copy-Item -LiteralPath LICENSE,README.CLI.md,UPSTREAM.md,TEST_REPORT.md,upstream-archive.sha256 -Destination dist
    Write-Host "Built: $PSScriptRoot/dist/seex-cli.exe"
} finally { Pop-Location }
