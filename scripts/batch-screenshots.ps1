<#
.SYNOPSIS
    Batch-generate screenshots for all .bin games in tmp/GameCollection.

.DESCRIPTION
    Runs the spmp8000-emu binary in screenshot mode (--screenshot) for every
    .bin file found under tmp/GameCollection (recursively).  Output PNGs are
    saved to docs/images/, named after the game file (without extension). When
    no binary is supplied, the latest release binary is built before capture.

.PARAMETER Frames
    Number of frames to emulate before capturing.  Default: 300 (10 seconds at
    30 fps — enough for slower title screens such as GoBang to appear). When
    omitted, known games with shorter title-screen windows use tuned defaults.
    Supplying this parameter applies the requested frame count to every game.

.PARAMETER Binary
    Path to the spmp8000-emu binary.  Default: cargo build output.
#>

param(
    [int]$Frames = 300,
    [string]$Binary = ""
)

$ErrorActionPreference = "Stop"
$framesSpecified = $PSBoundParameters.ContainsKey("Frames")

$repoRoot = Split-Path -Parent $PSScriptRoot
$gameDir  = Join-Path $repoRoot "tmp\GameCollection"
$outDir   = Join-Path $repoRoot "docs\images"

if (-not (Test-Path $gameDir)) {
    Write-Error "Game directory not found: $gameDir"
    exit 1
}

# Ensure output directory exists
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# Resolve binary path
if (-not $Binary) {
    $Binary = Join-Path $repoRoot "target\release\spmp8000-emu.exe"
    Write-Host "Building the latest release binary..." -ForegroundColor Yellow
    try {
        Push-Location $repoRoot
        cargo build --release -p spmp8000-emu
        if ($LASTEXITCODE -ne 0) {
            throw "Release build failed with exit code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
    if (-not (Test-Path $Binary)) {
        Write-Error "Build succeeded but binary was not found at $Binary."
        exit 1
    }
}

function Get-CaptureFrames {
    param(
        [string]$BaseName,
        [int]$DefaultFrames,
        [bool]$UseTitleScreenOverrides
    )

    if (-not $UseTitleScreenOverrides) {
        return $DefaultFrames
    }

    switch -Wildcard ($BaseName) {
        "DeepKiller*" { return 180 }
        "SmartBlocks*" { return 270 }
        default { return $DefaultFrames }
    }
}

Write-Host "Using binary: $Binary"
Write-Host "Game dir:     $gameDir"
Write-Host "Output dir:   $outDir"
if ($framesSpecified) {
    Write-Host "Frames:       $Frames"
} else {
    Write-Host "Frames:       $Frames (with title-screen overrides)"
}
Write-Host ""

# Collect all .bin files recursively
$games = Get-ChildItem -Path $gameDir -Filter "*.bin" -Recurse -File |
         Sort-Object Name

if ($games.Count -eq 0) {
    Write-Warning "No .bin files found under $gameDir"
    exit 0
}

Write-Host "Found $($games.Count) game(s).`n"

$success = 0
$failed  = 0

foreach ($game in $games) {
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($game.Name)
    # Sanitize: replace spaces and special chars with underscores
    $safeName = $baseName -replace '[^A-Za-z0-9_\-\.]', '_'
    $outPath  = Join-Path $outDir "$safeName.png"
    $captureFrames = Get-CaptureFrames $baseName $Frames (-not $framesSpecified)

    Write-Host -NoNewline "  $baseName ($captureFrames frames) ... "

    $prevEA = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Binary $game.FullName --screenshot $outPath --screenshot-frames $captureFrames 2>&1
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            Write-Host "FAILED (exit $exitCode)" -ForegroundColor Red
            $failed++
        } elseif (Test-Path $outPath) {
            $size = (Get-Item $outPath).Length
            Write-Host "OK ($([math]::Round($size/1024)) KB)" -ForegroundColor Green
            $success++
        } else {
            Write-Host "FAILED (no output)" -ForegroundColor Red
            $failed++
        }
    } catch {
        Write-Host "FAILED ($_)" -ForegroundColor Red
        $failed++
    } finally {
        $ErrorActionPreference = $prevEA
    }
}

Write-Host ""
Write-Host "Done: $success succeeded, $failed failed out of $($games.Count) total."
