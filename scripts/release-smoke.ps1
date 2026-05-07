param(
    [string]$SmokeRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($SmokeRoot)) {
    $stamp = Get-Date -Format 'yyyy-MM-dd-HHmmss-fff'
    $SmokeRoot = Join-Path $repoRoot ".tmp/release-smoke/$stamp-ps-$PID"
}
elseif (-not [System.IO.Path]::IsPathRooted($SmokeRoot)) {
    $SmokeRoot = Join-Path $repoRoot $SmokeRoot
}

function Resolve-SmokeRoot {
    param([string]$Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $allowedRoots = @(
        [System.IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp/release-smoke')),
        [System.IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp/usmoke'))
    )

    foreach ($allowedRoot in $allowedRoots) {
        $prefix = $allowedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
        if ($fullPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $fullPath
        }
    }

    throw "SmokeRoot must resolve under repo-local .tmp/release-smoke/ or .tmp/usmoke/: $fullPath"
}

$smokeRoot = Resolve-SmokeRoot $SmokeRoot
# Keep cargo package verification out of the workspace target dir; Windows can keep
# target/package files locked between package runs. The per-run root keeps parallel
# reviewer evidence from deleting another smoke process's artifacts.
$smokeTargetRoot = Join-Path $smokeRoot 'target'

function Invoke-NativeChecked {
    param(
        [string]$Command,
        [scriptblock]$Action
    )

    $global:LASTEXITCODE = 0
    & $Action
    $exitCode = if ($LASTEXITCODE -is [int]) { $LASTEXITCODE } else { 0 }
    if ($exitCode -ne 0) {
        throw "$Command failed with exit code $exitCode"
    }
}

Push-Location $repoRoot
try {
    if (Test-Path -LiteralPath $smokeRoot) {
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }

    Write-Host 'Release smoke: package official artifacts, verify CLI entry point, and generate minimal bindings.'
    Write-Host 'This smoke does not prove runtime, discovery, routing, provider hosting, or client SDK behavior.'
    Write-Host "Release smoke artifact root: $smokeRoot"

    Invoke-NativeChecked 'cargo package -p latrix-laic --allow-dirty' { cargo package -p latrix-laic --allow-dirty --target-dir $smokeTargetRoot }
    Invoke-NativeChecked 'cargo package -p laicc --allow-dirty' { cargo package -p laicc --allow-dirty --target-dir $smokeTargetRoot }
    Invoke-NativeChecked 'cargo run -p laicc -- --help' { cargo run --target-dir $smokeTargetRoot -p laicc -- --help }
    Invoke-NativeChecked 'cargo run -p laicc -- echo rust binding generation' { cargo run --target-dir $smokeTargetRoot -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang rust -o (Join-Path $smokeRoot 'rust') }
    Invoke-NativeChecked 'cargo run -p laicc -- echo python binding generation' { cargo run --target-dir $smokeTargetRoot -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang python -o (Join-Path $smokeRoot 'python') }
    Invoke-NativeChecked 'cargo run -p laicc -- echo typescript binding generation' { cargo run --target-dir $smokeTargetRoot -p laicc -- crates/laicc/tests/fixtures/echo.laic --lang typescript -o (Join-Path $smokeRoot 'typescript') }

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
