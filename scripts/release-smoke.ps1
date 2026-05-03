Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$smokeRoot = Join-Path $repoRoot '.tmp/release-smoke'

Push-Location $repoRoot
try {
    if (Test-Path -LiteralPath $smokeRoot) {
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }

    Write-Host 'Release smoke: package official artifacts, verify CLI entry point, and generate minimal bindings.'
    Write-Host 'This smoke does not prove runtime, discovery, routing, provider hosting, or client SDK behavior.'

    cargo package -p latrix-laic --allow-dirty
    cargo package -p laicc --allow-dirty
    cargo run -p laicc -- --help
    cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang rust -o (Join-Path $smokeRoot 'rust')
    cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang python -o (Join-Path $smokeRoot 'python')
    cargo run -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang typescript -o (Join-Path $smokeRoot 'typescript')

    $expectedOutputs = @(
        (Join-Path $smokeRoot 'rust/echo_laic.rs'),
        (Join-Path $smokeRoot 'python/echo_laic.py'),
        (Join-Path $smokeRoot 'typescript/echo_laic.ts')
    )

    foreach ($path in $expectedOutputs) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "missing expected release smoke output: $path"
        }
    }

    Write-Host 'Release smoke passed.'
}
finally {
    Pop-Location
}
