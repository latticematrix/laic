param(
    [string]$DemoRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ([string]::IsNullOrWhiteSpace($DemoRoot)) {
    $DemoRoot = Join-Path $repoRoot '.tmp/mechanism-only-demo'
}
elseif (-not [System.IO.Path]::IsPathRooted($DemoRoot)) {
    $DemoRoot = Join-Path $repoRoot $DemoRoot
}

$demoRootFull = [System.IO.Path]::GetFullPath($DemoRoot)
$allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot '.tmp'))
$allowedPrefix = $allowedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $demoRootFull.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "DemoRoot must resolve under repo-local .tmp/: $demoRootFull"
}

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
    if (Test-Path -LiteralPath $demoRootFull) {
        Remove-Item -LiteralPath $demoRootFull -Recurse -Force
    }

    $contract = Join-Path $PSScriptRoot 'echo_contract.laic'
    $targetRoot = Join-Path $demoRootFull 'target'

    Write-Host 'LAIC mechanism-only demo: compile one .laic contract into Rust, Python, and TypeScript bindings.'
    Write-Host 'This demo does not prove runtime, routing, provider hosting, workflow, marketplace, or multi-agent behavior.'
    Write-Host "Demo output root: $demoRootFull"

    Invoke-NativeChecked 'laicc rust generation' { cargo run --target-dir $targetRoot -p laicc -- $contract --lang rust -o (Join-Path $demoRootFull 'rust') }
    Invoke-NativeChecked 'laicc python generation' { cargo run --target-dir $targetRoot -p laicc -- $contract --lang python -o (Join-Path $demoRootFull 'python') }
    Invoke-NativeChecked 'laicc typescript generation' { cargo run --target-dir $targetRoot -p laicc -- $contract --lang typescript -o (Join-Path $demoRootFull 'typescript') }

    $expectedOutputs = @(
        (Join-Path $demoRootFull 'rust/echo_contract_laic.rs'),
        (Join-Path $demoRootFull 'python/echo_contract_laic.py'),
        (Join-Path $demoRootFull 'typescript/echo_contract_laic.ts')
    )

    foreach ($path in $expectedOutputs) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "missing expected demo output: $path"
        }
    }

    Write-Host 'LAIC mechanism-only demo passed.'
}
finally {
    Pop-Location
}
