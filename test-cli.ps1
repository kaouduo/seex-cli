$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo test --locked --lib project_scan
    if ($LASTEXITCODE -ne 0) { throw 'Scanner tests failed' }
    cargo test --locked --lib cli::tests
    if ($LASTEXITCODE -ne 0) { throw 'CLI tests failed' }
} finally { Pop-Location }
