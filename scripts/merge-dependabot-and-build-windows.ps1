#!/usr/bin/env pwsh
<#!
.SYNOPSIS
Merges all open Dependabot PRs and dispatches the Windows build workflow.

.DESCRIPTION
Uses GitHub CLI (gh) to:
- List open PRs authored by Dependabot
- Merge each PR (default: squash) and optionally delete its branch
- Trigger the GitHub Actions workflow '.github/workflows/windows-build.yml' via workflow_dispatch

This is intended as a "repo maintenance" helper.

REQUIREMENTS
- GitHub CLI (gh) installed and authenticated with 'repo' scope
- This repo checked out locally (for auto-detecting the GitHub repo from remotes)

.PARAMETER Repo
GitHub repo in OWNER/NAME format. If omitted, the script attempts to infer it from the 'origin' remote.

.PARAMETER BaseBranch
Target base branch to merge into (default: master).

.PARAMETER Workflow
Workflow file name or workflow name to dispatch (default: windows-build.yml).

.PARAMETER MergeMethod
Merge method: squash | merge | rebase (default: squash).

.PARAMETER DeleteBranch
If set, deletes the PR branch after merge.

.PARAMETER DryRun
If set, prints what would happen without merging/dispatching.

.PARAMETER Limit
Max number of PRs to process (default: 100).

.EXAMPLE
./scripts/merge-dependabot-and-build-windows.ps1

.EXAMPLE
./scripts/merge-dependabot-and-build-windows.ps1 -Repo DovieW/kolboo -BaseBranch master -DeleteBranch

#>

[CmdletBinding(SupportsShouldProcess = $true)]
param(
	[string]$Repo,
	[string]$BaseBranch = "master",
	[string]$Workflow = "windows-build.yml",
	[ValidateSet("squash", "merge", "rebase")]
	[string]$MergeMethod = "squash",
	[switch]$DeleteBranch,
	[switch]$DryRun,
	[int]$Limit = 100
)

$ErrorActionPreference = "Stop"

function Assert-CommandExists {
	param([Parameter(Mandatory)] [string]$Name)
	if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
		throw "Required command '$Name' was not found in PATH."
	}
}

function Get-RepoFromOriginRemote {
	try {
		$origin = git remote get-url origin 2>$null
		if (-not $origin) {
			return $null
		}

		# Supports:
		# - git@github.com:OWNER/REPO.git
		# - https://github.com/OWNER/REPO.git
		# - https://github.com/OWNER/REPO
		$origin = $origin.Trim()

		$sshMatch = [regex]::Match($origin, "^git@github\.com:(?<owner>[^/]+)/(?<repo>[^/]+?)(?:\.git)?$")
		if ($sshMatch.Success) {
			return "{0}/{1}" -f $sshMatch.Groups["owner"].Value, $sshMatch.Groups["repo"].Value
		}

		$httpsMatch = [regex]::Match($origin, "^https?://github\.com/(?<owner>[^/]+)/(?<repo>[^/]+?)(?:\.git)?$")
		if ($httpsMatch.Success) {
			return "{0}/{1}" -f $httpsMatch.Groups["owner"].Value, $httpsMatch.Groups["repo"].Value
		}

		return $null
	} catch {
		return $null
	}
}

function Ensure-GhAuth {
	# 'gh auth status' exits non-zero if not authenticated.
	$null = gh auth status 2>$null
}

Assert-CommandExists -Name "git"
Assert-CommandExists -Name "gh"

if (-not $Repo) {
	$Repo = Get-RepoFromOriginRemote
}

if (-not $Repo) {
	throw "Could not infer -Repo from git remote 'origin'. Pass -Repo OWNER/NAME explicitly."
}

Write-Host "Repo: $Repo"
Write-Host "Base branch: $BaseBranch"
Write-Host "Workflow: $Workflow"
Write-Host "Merge method: $MergeMethod"
Write-Host "Delete branch: $($DeleteBranch.IsPresent)"
Write-Host "Dry run: $($DryRun.IsPresent)"

Ensure-GhAuth

# Fetch open PRs authored by Dependabot.
# Depending on installation, author login can be either:
# - dependabot[bot]
# - app/dependabot
# We filter on the 'is_bot' author flag to be resilient.
$prsJson = gh pr list -R $Repo --state open --limit $Limit --json number,title,author,isDraft | Out-String
$prs = @()
if ($prsJson.Trim()) {
	$prs = $prsJson | ConvertFrom-Json
}

$dependabotPrs = @($prs | Where-Object {
	$_.author -and $_.author.is_bot -eq $true -and ($_.author.login -like "*dependabot*")
})

if ($dependabotPrs.Count -eq 0) {
	Write-Host "No open Dependabot PRs found."
} else {
	Write-Host "Found $($dependabotPrs.Count) open Dependabot PR(s):"
	foreach ($pr in $dependabotPrs) {
		Write-Host ("- #{0}: {1}" -f $pr.number, $pr.title)
	}
}

foreach ($pr in $dependabotPrs) {
	if ($pr.isDraft -eq $true) {
		Write-Host ("Skipping draft PR #{0} ({1})" -f $pr.number, $pr.title)
		continue
	}

	$mergeArgs = @(
		"pr", "merge",
		"-R", $Repo,
		"$($pr.number)",
		"--$MergeMethod"
	)
	if ($DeleteBranch.IsPresent) {
		$mergeArgs += "--delete-branch"
	}

	if ($DryRun.IsPresent) {
		Write-Host ("[DryRun] Would merge PR #{0} with: gh {1}" -f $pr.number, ($mergeArgs -join " "))
		continue
	}

	if ($PSCmdlet.ShouldProcess("PR #$($pr.number)", "Merge ($MergeMethod)")) {
		& gh @mergeArgs
	}
}

# Kick off Windows build workflow.
$dispatchArgs = @(
	"workflow", "run",
	"-R", $Repo,
	"$Workflow",
	"--ref", $BaseBranch
)

if ($DryRun.IsPresent) {
	Write-Host ("[DryRun] Would dispatch workflow with: gh {0}" -f ($dispatchArgs -join " "))
	return
}

if ($PSCmdlet.ShouldProcess("workflow $Workflow", "Dispatch on $BaseBranch")) {
	& gh @dispatchArgs
}
