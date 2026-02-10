<#
.SYNOPSIS
    Benchmark streaming vs batch STT for providers that support both modes.

.DESCRIPTION
    For each configured provider, runs:
      - Batch: pipeline transcribe (sends full audio after recording, HTTP POST)
      - Stream: pipeline stream --speed 1 (sends chunks via WebSocket at real-time pace)

    The key metric is "wait after stop" — how long a user waits once they finish talking:
      - Batch: entire stt_duration_ms (upload + transcribe happens after recording ends)
      - Stream: tail_latency_ms (time between last audio chunk sent and final result)

    Providers without a configured API key are automatically skipped.

.PARAMETER WavFile
    Path to a WAV file to use for benchmarking (16-bit PCM).

.PARAMETER Repeat
    Number of measured runs per test case (default: 1). Uses the --repeat flag
    for batch and runs stream N times.

.PARAMETER Providers
    Comma-separated list of providers to test. Default: all.
    Available: speechmatics, fireworks, elevenlabs, openai, assemblyai, deepgram

.EXAMPLE
    .\scripts\benchmark-streaming-vs-batch.ps1 -WavFile .\test.wav
    .\scripts\benchmark-streaming-vs-batch.ps1 -WavFile .\test.wav -Repeat 3
    .\scripts\benchmark-streaming-vs-batch.ps1 -WavFile .\test.wav -Providers fireworks,speechmatics
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$WavFile,

    [int]$Repeat = 1,

    [string]$Providers = ""
)

$ErrorActionPreference = "Stop"

# ── Locate the kolboo binary ────────────────────────────────────────────────
$RepoRoot = Split-Path -Parent $PSScriptRoot
$DebugBin = Join-Path $RepoRoot "app\src-tauri\target\debug\kolboo.exe"

if (-not (Test-Path $DebugBin)) {
    Write-Host "Binary not found at $DebugBin" -ForegroundColor Red
    Write-Host "Build first: pnpm -C app tauri dev  (or cargo build from app/src-tauri)"
    exit 1
}

if (-not (Test-Path $WavFile)) {
    Write-Host "WAV file not found: $WavFile" -ForegroundColor Red
    exit 1
}

$WavFile = (Resolve-Path $WavFile).Path

# ── Provider test matrix ────────────────────────────────────────────────────
# Each entry: provider, batch model, stream model, language hint (optional)
#
# Notes:
#   - Fireworks streaming models (asr-v2, asr-large) can't do batch; we compare
#     against their fastest batch model (whisper-v3-turbo).
#   - AssemblyAI streaming models are separate from batch models.
#   - OpenAI realtime models require streaming; batch uses gpt-4o-transcribe.
#   - ElevenLabs scribe_v2 uses WS for both batch and stream internally.
#   - Speechmatics uses the same model for both modes (true apples-to-apples).

$TestMatrix = @(
    @{
        Provider     = "speechmatics"
        BatchModel   = "enhanced"
        StreamModel  = "enhanced"
        Language     = "en"
        Notes        = "Same model for both modes (true comparison)"
    },
    @{
        Provider     = "fireworks"
        BatchModel   = "whisper-v3-turbo"
        StreamModel  = "fireworks-asr-v2"
        Language     = "en"
        Notes        = "Different models: batch=whisper-v3-turbo, stream=asr-v2"
    },
    @{
        Provider     = "elevenlabs"
        BatchModel   = "scribe_v2"
        StreamModel  = "scribe_v2"
        Language     = "en"
        Notes        = "scribe_v2 uses WS internally for both"
    },
    @{
        Provider     = "openai"
        BatchModel   = "gpt-4o-transcribe"
        StreamModel  = "gpt-4o-realtime-transcribe"
        Language     = "en"
        Notes        = "Different APIs: batch=HTTP, stream=Realtime WS"
    },
    @{
        Provider     = "assemblyai"
        BatchModel   = "universal"
        StreamModel  = "universal-streaming-english"
        Language     = "en"
        Notes        = "Different models: batch=universal, stream=universal-streaming-english"
    },
    @{
        Provider     = "deepgram"
        BatchModel   = "nova-3"
        StreamModel  = "nova-3"
        Language     = "en"
        Notes        = "Same model for both modes (true comparison)"
    }
)

# Filter by --Providers if specified
if ($Providers -ne "") {
    $allowed = $Providers -split "," | ForEach-Object { $_.Trim().ToLower() }
    $TestMatrix = $TestMatrix | Where-Object { $allowed -contains $_.Provider }
}

if ($TestMatrix.Count -eq 0) {
    Write-Host "No providers matched the filter." -ForegroundColor Yellow
    exit 0
}

# ── Helpers ─────────────────────────────────────────────────────────────────

function Run-Batch {
    param(
        [string]$Provider,
        [string]$Model,
        [string]$File,
        [int]$Repeat
    )

    $args = @(
        "pipeline", "transcribe",
        "--file", $File,
        "--stt_provider", $Provider,
        "--stt_model", $Model,
        "-o", "json"
    )
    if ($Repeat -gt 1) {
        $args += @("--repeat", $Repeat.ToString())
    }

    $proc = Start-Process -FilePath $DebugBin -ArgumentList $args `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput "$env:TEMP\kolboo-bench-batch-stdout.txt" `
        -RedirectStandardError "$env:TEMP\kolboo-bench-batch-stderr.txt"

    $stdout = Get-Content "$env:TEMP\kolboo-bench-batch-stdout.txt" -Raw -ErrorAction SilentlyContinue
    $stderr = Get-Content "$env:TEMP\kolboo-bench-batch-stderr.txt" -Raw -ErrorAction SilentlyContinue

    if ($proc.ExitCode -ne 0) {
        return @{ Error = "exit code $($proc.ExitCode): $stderr"; Json = $null }
    }

    try {
        # The CLI wraps output in a result envelope; extract the data payload.
        $json = $stdout | ConvertFrom-Json
        if ($json.data) {
            return @{ Error = $null; Json = $json.data }
        }
        return @{ Error = $null; Json = $json }
    }
    catch {
        return @{ Error = "Failed to parse JSON: $_`nstdout=$stdout"; Json = $null }
    }
}

function Run-Stream {
    param(
        [string]$Provider,
        [string]$Model,
        [string]$Language,
        [string]$File
    )

    $args = @(
        "pipeline", "stream",
        "--file", $File,
        "--stt_provider", $Provider,
        "--stt_model", $Model,
        "--speed", "1",
        "-o", "json"
    )
    if ($Language) {
        $args += @("--language", $Language)
    }

    $proc = Start-Process -FilePath $DebugBin -ArgumentList $args `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput "$env:TEMP\kolboo-bench-stream-stdout.txt" `
        -RedirectStandardError "$env:TEMP\kolboo-bench-stream-stderr.txt"

    $stdout = Get-Content "$env:TEMP\kolboo-bench-stream-stdout.txt" -Raw -ErrorAction SilentlyContinue
    $stderr = Get-Content "$env:TEMP\kolboo-bench-stream-stderr.txt" -Raw -ErrorAction SilentlyContinue

    if ($proc.ExitCode -ne 0) {
        return @{ Error = "exit code $($proc.ExitCode): $stderr"; Json = $null }
    }

    try {
        $json = $stdout | ConvertFrom-Json
        if ($json.data) {
            return @{ Error = $null; Json = $json.data }
        }
        return @{ Error = $null; Json = $json }
    }
    catch {
        return @{ Error = "Failed to parse JSON: $_`nstdout=$stdout"; Json = $null }
    }
}

# ── Main ────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  STT Benchmark: Streaming vs Batch" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  WAV file : $WavFile"
Write-Host "  Repeat   : $Repeat"
Write-Host "  Binary   : $DebugBin"
Write-Host "  Providers: $($TestMatrix | ForEach-Object { $_.Provider } | Join-String -Separator ', ')"
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

$results = @()

foreach ($entry in $TestMatrix) {
    $provider = $entry.Provider
    $batchModel = $entry.BatchModel
    $streamModel = $entry.StreamModel
    $language = $entry.Language
    $notes = $entry.Notes

    Write-Host "──────────────────────────────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "  Provider: $provider" -ForegroundColor Yellow
    Write-Host "  $notes" -ForegroundColor DarkGray
    Write-Host "──────────────────────────────────────────────────────────" -ForegroundColor DarkGray

    # ── Batch ───────────────────────────────────────────────────────
    Write-Host "  [batch]  model=$batchModel ..." -NoNewline -ForegroundColor White
    $batchResult = Run-Batch -Provider $provider -Model $batchModel -File $WavFile -Repeat $Repeat

    $batchMs = $null
    $batchText = ""
    if ($batchResult.Error) {
        Write-Host " FAILED" -ForegroundColor Red
        Write-Host "         $($batchResult.Error)" -ForegroundColor DarkRed
    }
    else {
        $json = $batchResult.Json
        if ($Repeat -gt 1 -and $json.summary) {
            $batchMs = $json.summary.stt_duration_ms.avg
        }
        else {
            $batchMs = $json.stt_duration_ms
        }
        $batchText = if ($json.stt_text) { $json.stt_text } else { $json.final_text }
        Write-Host " ${batchMs}ms" -ForegroundColor Green
    }

    # ── Stream ──────────────────────────────────────────────────────
    Write-Host "  [stream] model=$streamModel ..." -NoNewline -ForegroundColor White

    $tailLatencyValues = @()
    $streamSessionValues = @()
    $streamText = ""
    $streamError = $null

    for ($i = 0; $i -lt $Repeat; $i++) {
        $streamResult = Run-Stream -Provider $provider -Model $streamModel -Language $language -File $WavFile

        if ($streamResult.Error) {
            $streamError = $streamResult.Error
            break
        }

        $json = $streamResult.Json
        $tailLatencyValues += $json.tail_latency_ms
        $streamSessionValues += $json.session_ms
        if (-not $streamText -and $json.final_text) {
            $streamText = $json.final_text
        }
    }

    $tailMs = $null
    $streamMs = $null
    if ($streamError) {
        Write-Host " FAILED" -ForegroundColor Red
        Write-Host "         $streamError" -ForegroundColor DarkRed
    }
    else {
        if ($tailLatencyValues.Count -gt 1) {
            $tailMs = [math]::Round(($tailLatencyValues | Measure-Object -Average).Average)
            $streamMs = [math]::Round(($streamSessionValues | Measure-Object -Average).Average)
        }
        else {
            $tailMs = $tailLatencyValues[0]
            $streamMs = $streamSessionValues[0]
        }
        Write-Host " tail=${tailMs}ms (session=${streamMs}ms)" -ForegroundColor Green
    }

    # ── Compare ─────────────────────────────────────────────────────
    # The meaningful comparison: how long you wait after you stop talking.
    #   Batch:  full stt_duration_ms (upload + transcribe, all after stop)
    #   Stream: tail_latency_ms (time from feed done to final result)
    if ($batchMs -and $null -ne $tailMs) {
        $diff = $tailMs - $batchMs
        $pct = [math]::Round(($diff / $batchMs) * 100, 1)
        $winner = if ($diff -lt 0) { "STREAM" } else { "BATCH" }
        $emoji  = if ($diff -lt 0) { "<<" } else { ">>" }
        $color  = if ($diff -lt 0) { "Green" } else { "Yellow" }

        Write-Host "  Wait-after-stop: batch=${batchMs}ms  vs  stream-tail=${tailMs}ms  $emoji $winner wins by $([math]::Abs($pct))%" -ForegroundColor $color
    }
    else {
        Write-Host "  Result: comparison not available (one mode failed)" -ForegroundColor DarkYellow
    }

    $results += [PSCustomObject]@{
        Provider    = $provider
        BatchModel  = $batchModel
        StreamModel = $streamModel
        BatchMs     = $batchMs
        TailMs      = $tailMs
        SessionMs   = $streamMs
        Notes       = $notes
    }

    Write-Host ""
}

# ── Summary table ───────────────────────────────────────────────────────────

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  Summary" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

Write-Host "  'Wait after stop' is what the user experiences:" -ForegroundColor DarkGray
Write-Host "    Batch  = full stt_duration_ms (upload + transcribe after recording ends)" -ForegroundColor DarkGray
Write-Host "    Stream = tail_latency_ms (time from last audio chunk to final result)" -ForegroundColor DarkGray
Write-Host ""

$header = "{0,-15} {1,-22} {2,-30} {3,>12} {4,>12} {5,>10}" -f "Provider", "Batch Model", "Stream Model", "Batch Wait", "Stream Tail", "Diff(%)"
Write-Host $header -ForegroundColor White
Write-Host ("-" * 105) -ForegroundColor DarkGray

foreach ($r in $results) {
    $diffStr = ""
    if ($r.BatchMs -and $null -ne $r.TailMs) {
        $pct = [math]::Round((($r.TailMs - $r.BatchMs) / $r.BatchMs) * 100, 1)
        $diffStr = if ($pct -lt 0) { "${pct}%" } else { "+${pct}%" }
    }
    $bStr = if ($r.BatchMs) { "$($r.BatchMs)ms" } else { "FAIL" }
    $tStr = if ($null -ne $r.TailMs) { "$($r.TailMs)ms" } else { "FAIL" }

    $line = "{0,-15} {1,-22} {2,-30} {3,>12} {4,>12} {5,>10}" -f $r.Provider, $r.BatchModel, $r.StreamModel, $bStr, $tStr, $diffStr
    Write-Host $line
}

Write-Host ""
Write-Host "Negative diff% = streaming tail is faster (expected). Positive = batch wait is faster." -ForegroundColor DarkGray
Write-Host ""
