# CPAL-1.0 — Powered by VirgoStars — https://www.virgostars.it/
# Developer-only Windows PowerShell 5.1+/PowerShell 7 batching for the Rust
# sparse-span planner and separately licensed ephemeris candidate provider.
# Does not alter the owner P0 archive, a database or any existing JSON file.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [Parameter(Mandatory = $true)][string]$TrustedSha256,
    [Parameter(Mandatory = $true)][string]$OutputDir,
    [ValidateSet('plan', 'calculate')][string]$Mode = 'plan',
    [ValidateSet('synods', 'motions', 'ingresses')][string]$Family = 'synods',
    [string]$Body = 'Moon',
    [string]$FromUtc,
    [string]$ToUtc,
    [switch]$KnownMoonGaps,
    [int]$ThresholdDays = 45,
    [int]$StepHours = 6,
    [int]$SliceYears = 1,
    [int]$MaxJobs = 10,
    [int]$MaxSteps = 100000,
    [string]$BackendRoot,
    [string]$ProviderProgram,
    [string]$ReferenceProviderProgram,
    [switch]$VerifyHalfStep
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot 'Cargo.toml'
$archivePath = (Resolve-Path -LiteralPath $Archive).Path

if ($TrustedSha256 -cnotmatch '^[0-9a-f]{64}$') {
    throw 'TrustedSha256 must be the independently trusted lowercase SHA-256 of the P0 archive.'
}
if ($ThresholdDays -le 0 -or $StepHours -le 0 -or $SliceYears -le 0 -or $MaxJobs -le 0 -or $MaxSteps -le 0 -or $MaxSteps -gt 1000000) {
    throw 'ThresholdDays, StepHours, SliceYears, MaxJobs and MaxSteps must be positive and bounded.'
}
if ($KnownMoonGaps -and ($Family -ne 'synods' -or $Body -ne 'Moon')) {
    throw '-KnownMoonGaps only applies to synods/Moon.'
}
if ($VerifyHalfStep -and $Mode -ne 'calculate') {
    throw '-VerifyHalfStep requires -Mode calculate.'
}
if ($ReferenceProviderProgram) {
    if ($Mode -ne 'calculate') {
        throw '-ReferenceProviderProgram requires -Mode calculate.'
    }
    $ReferenceProviderProgram = (Resolve-Path -LiteralPath $ReferenceProviderProgram).Path
}

if ($Mode -eq 'calculate') {
    if ($BackendRoot) {
        $backendPath = (Resolve-Path -LiteralPath $BackendRoot).Path
        $modulePath = Join-Path $backendPath 'backend/services/astrology/func.js'
        $gitRoot = $backendPath
        if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
            $modulePath = Join-Path $backendPath 'services/astrology/func.js'
        }
        if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
            throw 'Cannot find services/astrology/func.js in the supplied authorized private backend.'
        }
        $env:VSEPH_LEGACY_FUNC_MODULE = $modulePath
        $sourceCommit = (& git -C $gitRoot rev-parse HEAD).Trim()
        if ($LASTEXITCODE -ne 0 -or $sourceCommit -cnotmatch '^[0-9a-f]{40}$') {
            throw 'Cannot identify an exact source revision in the authorized backend checkout.'
        }
        $env:VSEPH_LEGACY_COMMIT = $sourceCommit
    }
    if (-not $ProviderProgram) {
        if (-not $env:VSEPH_LEGACY_FUNC_MODULE -or -not $env:VSEPH_LEGACY_COMMIT) {
            throw 'Using legacy Swiss requires -BackendRoot or VSEPH_LEGACY_FUNC_MODULE and VSEPH_LEGACY_COMMIT; alternatively specify -ProviderProgram.'
        }
        $ProviderProgram = Join-Path $repoRoot 'tools/providers/legacy-swiss-position.cmd'
    }
    $ProviderProgram = (Resolve-Path -LiteralPath $ProviderProgram).Path
}

function Parse-Utc([string]$text) {
    if ($text -cnotmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$') {
        throw "Expected strict UTC yyyy-MM-ddTHH:mm:ssZ, received: $text"
    }
    return [DateTimeOffset]::ParseExact($text, 'yyyy-MM-ddTHH:mm:ssZ',
        [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::AssumeUniversal)
}

$ranges = @()
if ($KnownMoonGaps) {
    $ranges += ,@('1918-12-03T15:18:52Z', '1920-01-21T05:26:29Z')
    $ranges += ,@('2017-12-18T06:30:25Z', '2019-01-06T01:28:11Z')
} else {
    if (-not $FromUtc -or -not $ToUtc) {
        throw 'Specify -FromUtc and -ToUtc, or -KnownMoonGaps.'
    }
    $ranges += ,@($FromUtc, $ToUtc)
}

[void][IO.Directory]::CreateDirectory($OutputDir)
$outputRoot = (Resolve-Path -LiteralPath $OutputDir).Path
$completeJobs = 0
$unprocessed = 0
foreach ($range in $ranges) {
    $cursor = Parse-Utc $range[0]
    $rangeEnd = Parse-Utc $range[1]
    if ($cursor -ge $rangeEnd) { throw 'Invalid non-increasing requested interval.' }
    while ($cursor -lt $rangeEnd) {
        $next = $cursor.AddYears($SliceYears)
        if ($next -gt $rangeEnd) { $next = $rangeEnd }
        $fromUs = [long]($cursor.ToUnixTimeMilliseconds() * 1000L)
        $toUs = [long]($next.ToUnixTimeMilliseconds() * 1000L)
        $safeBody = $Body -replace '[^A-Za-z0-9_-]', '_'
        $modeLabel = $Mode
        if ($VerifyHalfStep) { $modeLabel += '_halfstep' }
        if ($ReferenceProviderProgram) { $modeLabel += '_reference' }
        $filename = '{0}_{1}_{2}_{3}_{4}_{5}.json' -f $Family, $safeBody, $fromUs, $toUs, $modeLabel, $TrustedSha256.Substring(0, 12)
        $destination = Join-Path $outputRoot $filename
        if (Test-Path -LiteralPath $destination) {
            Write-Host "Existing artifact preserved (skip): $filename"
            $cursor = $next
            continue
        }
        if ($completeJobs -ge $MaxJobs) {
            $unprocessed++
            $cursor = $next
            continue
        }
        $argsForCargo = @('run', '--quiet', '--locked', '--manifest-path', $manifest,
            '-p', 'virgostars-events-generator', '--bin', 'vseph-fill-gaps', '--',
            '--mode', $Mode, '--archive', $archivePath, '--sha256', $TrustedSha256,
            '--family', $Family, '--body', $Body,
            '--start-us', [string]$fromUs, '--end-us', [string]$toUs,
            '--threshold-us', [string]([long]$ThresholdDays * 86400000000L))
        if ($Mode -eq 'calculate') {
            $argsForCargo += @('--provider-program', $ProviderProgram,
                '--step-us', [string]([long]$StepHours * 3600000000L),
                '--precision-us', '1000000', '--max-steps', [string]$MaxSteps)
            if ($VerifyHalfStep) { $argsForCargo += @('--cadence-check', 'half') }
        }
        Write-Host "Processing $Family/$Body $($cursor.ToString('u')) → $($next.ToString('u')) ($Mode)"
        $jsonLines = & cargo @argsForCargo
        if ($LASTEXITCODE -ne 0) {
            throw "Rust interval calculation failed; no staging output saved for $filename"
        }
        $jsonText = ($jsonLines -join "`n") + "`n"
        $report = $jsonText | ConvertFrom-Json
        if ($report.kind -ne 'unverified_sparse_interval_staging' -or
            $report.dataset.sha256 -cne $TrustedSha256 -or $report.coverage_certified -ne $false) {
            throw 'Rust report did not match the trusted archive or staging-only contract.'
        }
        if ($VerifyHalfStep -and $report.sparse_spans.Count -gt 0 -and
            ($report.cadence_check.mode -ne 'half_step' -or
             $report.cadence_check.matched_candidates -ne $report.candidates.Count)) {
            throw "Rust half-step rescan metadata missing or inconsistent for $filename"
        }
        $temporary = Join-Path $outputRoot ('.' + $filename + '.' + [Guid]::NewGuid().ToString('N') + '.tmp')
        [IO.File]::WriteAllText($temporary, $jsonText, (New-Object Text.UTF8Encoding($false)))
        try {
            # Second Rust process: independently opens/authenticates P0 and
            # recomputes every recorded anchor before saving staging output.
            # This structural gate does NOT establish astronomical accuracy.
            $reviewArgs = @('run', '--quiet', '--locked', '--manifest-path', $manifest,
                '-p', 'virgostars-events-generator', '--bin', 'vseph-review-candidates', '--',
                '--archive', $archivePath, '--sha256', $TrustedSha256, '--staging', $temporary)
            if ($ReferenceProviderProgram) {
                $reviewArgs += @('--reference-provider-program', $ReferenceProviderProgram)
            }
            $reviewLines = & cargo @reviewArgs
            if ($LASTEXITCODE -ne 0) {
                throw "Independent Rust staging review failed; no artifact saved for $filename"
            }
            $review = ($reviewLines -join "`n") | ConvertFrom-Json
            if ($review.staging_structure_verified -ne $true -or
                $review.source_archive_authenticated -ne $true -or
                $review.safe_to_import -ne $false -or
                $review.coverage_certified -ne $false) {
                throw "Independent Rust review rejected the staging-only contract for $filename"
            }
            if ($ReferenceProviderProgram -and
                ($review.reference_crossings_consistent -ne $true -or
                 $review.numerical_crossings_checked -ne $report.candidates.Count)) {
                throw "Reference provider did not verify all candidate crossing brackets for $filename"
            }
            Move-Item -LiteralPath $temporary -Destination $destination -ErrorAction Stop
        } finally {
            if (Test-Path -LiteralPath $temporary) {
                Remove-Item -LiteralPath $temporary -Force
            }
        }
        $completeJobs++
        Write-Host "Saved structurally reviewed, astronomically unverified staging artifact: $destination"
        $cursor = $next
    }
}
if ($unprocessed -gt 0) {
    Write-Warning "$unprocessed interval slices not processed because MaxJobs=$MaxJobs. Rerun to continue; existing JSON files are preserved."
}
Write-Host "Completed $completeJobs new $Mode staging artifacts. No original data or database changed."
