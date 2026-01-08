[CmdletBinding()]
param(
	# Build the CUDA feature variant too.
	[switch]$Cuda,

	# Skip corepack/pnpm install. Useful if you already ran install.
	[switch]$SkipInstall,

	# Delete existing artifacts/ and target outputs first.
	[switch]$Clean,

	# Output directory for collected bundles (defaults to repo-root/artifacts)
	[string]$ArtifactsDir = "artifacts"
)

$ErrorActionPreference = "Stop"

function Write-Section([string]$Title) {
	Write-Host ""
	Write-Host "== $Title ==" -ForegroundColor Cyan
}

function Assert-Command([string]$Name, [string]$Hint) {
	if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
		throw "Missing required command '$Name'. $Hint"
	}
}

# Repo root = parent of scripts/
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$AppDir = Join-Path $RepoRoot "app"
$SrcTauriDir = Join-Path $AppDir "src-tauri"
$ArtifactsDirAbs = Resolve-Path (Join-Path $RepoRoot $ArtifactsDir) -ErrorAction SilentlyContinue
if (-not $ArtifactsDirAbs) {
	$ArtifactsDirAbs = Join-Path $RepoRoot $ArtifactsDir
}

Write-Section "Environment checks"
Assert-Command -Name "git" -Hint "Install Git or run from a working tree checkout."
Assert-Command -Name "node" -Hint "Install Node.js 20+."
Assert-Command -Name "corepack" -Hint "Corepack ships with Node; ensure it's enabled."
Assert-Command -Name "pnpm" -Hint "Run: corepack enable; corepack prepare pnpm@<version> --activate"

# Rust is required for Tauri builds.
Assert-Command -Name "cargo" -Hint "Install Rust (rustup)."

# Optional: sccache (speeds up rebuilds)
$HasSccache = [bool](Get-Command sccache -ErrorAction SilentlyContinue)
if ($HasSccache) {
	$env:RUSTC_WRAPPER = "sccache"
	$env:SCCACHE_NO_DAEMON = "1"
	Write-Host "Using sccache: $(sccache --version)" -ForegroundColor DarkGray
}

Write-Section "Resolve pnpm version (from app/package.json)"
$PkgJsonPath = Join-Path $AppDir "package.json"
if (-not (Test-Path $PkgJsonPath)) {
	throw "Could not find app/package.json at: $PkgJsonPath"
}

$Pkg = Get-Content $PkgJsonPath -Raw | ConvertFrom-Json
$pm = [string]$Pkg.packageManager
if (-not $pm.Contains("@")) {
	throw "Unexpected packageManager format in app/package.json: '$pm'"
}
$PnpmVersion = $pm.Split("@")[-1]
Write-Host "pnpm pinned version: $PnpmVersion" -ForegroundColor DarkGray

Write-Section "Clean (optional)"
if ($Clean) {
	if (Test-Path $ArtifactsDirAbs) {
		Remove-Item -Recurse -Force $ArtifactsDirAbs
	}
	# Mirror CI bundle locations.
	$TargetDir = Join-Path $SrcTauriDir "target"
	if (Test-Path $TargetDir) {
		Remove-Item -Recurse -Force $TargetDir
	}
}

Write-Section "Prepare pnpm (Corepack) + install (optional)"
Push-Location $RepoRoot
try {
	corepack enable | Out-Null
	corepack prepare "pnpm@$PnpmVersion" --activate | Out-Null
	Write-Host "pnpm version: $(pnpm -v)" -ForegroundColor DarkGray

	if (-not $SkipInstall) {
		Push-Location $AppDir
		try {
			pnpm install --frozen-lockfile
		} finally {
			Pop-Location
		}
	}
} finally {
	Pop-Location
}

function Collect-Bundles([string]$VariantName) {
	$OutDir = Join-Path $ArtifactsDirAbs ("windows-" + $VariantName)
	New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

	$BundleDir = Join-Path $SrcTauriDir "target\release\bundle"
	if (Test-Path $BundleDir) {
		Copy-Item -Recurse -Force $BundleDir (Join-Path $OutDir "bundle")
	} else {
		Write-Host "(warn) bundle directory not found at: $BundleDir" -ForegroundColor Yellow
	}

	Get-ChildItem (Join-Path $SrcTauriDir "target\release") -Filter "*.exe" -ErrorAction SilentlyContinue |
		ForEach-Object { Copy-Item -Force $_.FullName $OutDir }
}

Write-Section "Build: default"
Push-Location $AppDir
try {
	# Mirrors CI: `pnpm build` == `tauri build`
	pnpm build
} finally {
	Pop-Location
}
Collect-Bundles -VariantName "default"

Write-Section "Build: local-whisper (CPU)"
Push-Location $AppDir
try {
	# Mirrors CI: forward cargo args after `--`.
	pnpm tauri build -- --features local-whisper
} finally {
	Pop-Location
}
Collect-Bundles -VariantName "local-whisper"

if ($Cuda) {
	Write-Section "Build: local-whisper-cuda"

	# Best-effort sanity checks (does not guarantee runtime works).
	$CudaRoot = 'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA'
	if (-not (Test-Path $CudaRoot)) {
		Write-Host "(warn) CUDA Toolkit root not found at $CudaRoot" -ForegroundColor Yellow
		Write-Host "      You installed CUDA via winget, so you likely have it; if not, install it first." -ForegroundColor Yellow
	}

	Push-Location $AppDir
	try {
		pnpm tauri build -- --features local-whisper-cuda
	} finally {
		Pop-Location
	}
	Collect-Bundles -VariantName "local-whisper-cuda"
}

Write-Section "Done"
Write-Host "Artifacts written to: $ArtifactsDirAbs" -ForegroundColor Green
Write-Host "- $ArtifactsDirAbs\windows-default" -ForegroundColor DarkGray
Write-Host "- $ArtifactsDirAbs\windows-local-whisper" -ForegroundColor DarkGray
if ($Cuda) {
	Write-Host "- $ArtifactsDirAbs\windows-local-whisper-cuda" -ForegroundColor DarkGray
}
