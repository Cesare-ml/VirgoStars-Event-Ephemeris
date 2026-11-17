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
    [string]$ReferenceProviderSha256,
    [Nullable[int]]$ReferenceRescanStepHours,
    [switch]$VerifyHalfStep,
    [int]$AdaptiveStationDepth = 0,
    [int]$AdaptiveIngressDepth = 0,
    [int]$AdaptiveLunarDepth = 0
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
if ($AdaptiveStationDepth -ne 0 -and ($Mode -ne 'calculate' -or $Family -ne 'motions' -or
    $AdaptiveStationDepth -lt 1 -or $AdaptiveStationDepth -gt 8)) {
    throw '-AdaptiveStationDepth must be 1..8 and requires -Mode calculate -Family motions.'
}
if ($AdaptiveIngressDepth -ne 0 -and ($Mode -ne 'calculate' -or $Family -ne 'ingresses' -or
    $AdaptiveIngressDepth -lt 1 -or $AdaptiveIngressDepth -gt 8)) {
    throw '-AdaptiveIngressDepth must be 1..8 and requires -Mode calculate -Family ingresses.'
}
if ($AdaptiveLunarDepth -ne 0 -and ($Mode -ne 'calculate' -or $Family -ne 'synods' -or
    $Body -ne 'Moon' -or $AdaptiveLunarDepth -lt 1 -or $AdaptiveLunarDepth -gt 8)) {
    throw '-AdaptiveLunarDepth must be 1..8 and requires -Mode calculate -Family synods -Body Moon.'
}
if ($ReferenceProviderProgram) {
    if ($Mode -ne 'calculate') {
        throw '-ReferenceProviderProgram requires -Mode calculate.'
    }
    $ReferenceProviderProgram = (Resolve-Path -LiteralPath $ReferenceProviderProgram).Path
}
if ($PSBoundParameters.ContainsKey('ReferenceProviderSha256') -and
    (-not $ReferenceProviderProgram -or $ReferenceProviderSha256 -cnotmatch '^[0-9a-f]{64}$')) {
    throw '-ReferenceProviderSha256 requires -ReferenceProviderProgram and 64 lowercase hex digits from a separately trusted file digest.'
}
if ($null -ne $ReferenceRescanStepHours) {
    if ($Mode -ne 'calculate' -or -not $ReferenceProviderProgram -or
        $ReferenceRescanStepHours -le 0 -or $ReferenceRescanStepHours -ge $StepHours) {
        throw '-ReferenceRescanStepHours requires -Mode calculate, -ReferenceProviderProgram and a positive step finer than -StepHours.'
    }
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
        if ($AdaptiveStationDepth -gt 0) { $modeLabel += '_adaptive' + $AdaptiveStationDepth }
        if ($AdaptiveIngressDepth -gt 0) { $modeLabel += '_ingressAdaptive' + $AdaptiveIngressDepth }
        if ($AdaptiveLunarDepth -gt 0) { $modeLabel += '_lunarAdaptive' + $AdaptiveLunarDepth }
        if ($ReferenceProviderProgram) { $modeLabel += '_reference' }
        if ($ReferenceProviderSha256) { $modeLabel += '_sha' + $ReferenceProviderSha256.Substring(0, 12) }
        if ($null -ne $ReferenceRescanStepHours) { $modeLabel += '_rescan' + $ReferenceRescanStepHours + 'h' }
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
            if ($AdaptiveStationDepth -gt 0) { $argsForCargo += @('--adaptive-station-depth', [string]$AdaptiveStationDepth) }
            if ($AdaptiveIngressDepth -gt 0) { $argsForCargo += @('--adaptive-ingress-depth', [string]$AdaptiveIngressDepth) }
            if ($AdaptiveLunarDepth -gt 0) { $argsForCargo += @('--adaptive-lunar-depth', [string]$AdaptiveLunarDepth) }
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
        if ($AdaptiveStationDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
            ($report.adaptive_station_check.mode -ne 'risk_midpoint' -or
             $report.adaptive_station_check.max_depth -ne $AdaptiveStationDepth -or
             $report.adaptive_station_check.coverage_certified -ne $false -or
             $report.adaptive_station_check.matched_candidates -ne $report.candidates.Count)) {
            throw "Rust adaptive station check metadata missing or inconsistent for $filename"
        }
        if ($AdaptiveIngressDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
            ($report.adaptive_ingress_check.mode -ne 'zodiac_boundary_midpoint' -or
             $report.adaptive_ingress_check.max_depth -ne $AdaptiveIngressDepth -or
             $report.adaptive_ingress_check.coverage_certified -ne $false -or
             $report.adaptive_ingress_check.matched_candidates -ne $report.candidates.Count)) {
            throw "Rust adaptive ingress check metadata missing or inconsistent for $filename"
        }
        if ($AdaptiveLunarDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
            ($report.adaptive_lunar_check.mode -ne 'phase_angle_midpoint' -or
             $report.adaptive_lunar_check.max_depth -ne $AdaptiveLunarDepth -or
             $report.adaptive_lunar_check.coverage_certified -ne $false -or
             $report.adaptive_lunar_check.matched_candidates -ne $report.candidates.Count)) {
            throw "Rust adaptive lunar check metadata missing or inconsistent for $filename"
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
            if ($ReferenceProviderSha256) {
                $reviewArgs += @('--reference-provider-sha256', $ReferenceProviderSha256)
            }
            if ($null -ne $ReferenceRescanStepHours) {
                $reviewArgs += @('--reference-rescan-step-us', [string]([long]$ReferenceRescanStepHours * 3600000000L))
            }
            $reviewLines = & cargo @reviewArgs
            if ($LASTEXITCODE -ne 0) {
                throw "Independent Rust staging review failed; no artifact saved for $filename"
            }
            $review = ($reviewLines -join "`n") | ConvertFrom-Json
            if ($review.staging_structure_verified -ne $true -or
                $review.source_archive_authenticated -ne $true -or
                $review.safe_to_import -ne $false -or
                $review.coverage_certified -ne $false -or
                $review.astronomical_accuracy_verified -ne $false) {
                throw "Independent Rust review rejected the staging-only contract for $filename"
            }
            if ($ReferenceProviderProgram -and
                ($review.reference_crossings_consistent -ne $true -or
                 $review.numerical_crossings_checked -ne $report.candidates.Count)) {
                throw "Reference provider did not verify all candidate crossing brackets for $filename"
            }
            if ($ReferenceProviderSha256 -and
                ($review.reference_provider_digest_authenticated -ne $true -or
                 $review.reference_provider_executable_sha256 -cne $ReferenceProviderSha256)) {
                throw "Reference executable did not match independently trusted SHA-256 for $filename"
            }
            if ($null -ne $ReferenceRescanStepHours -and
                ($review.reference_rescan_performed -ne $true -or
                 $review.reference_rescan_correspondence_consistent -ne $true -or
                 $null -eq $review.reference_rescan_step_us -or
                 $review.reference_rescan_step_us -ne ([long]$ReferenceRescanStepHours * 3600000000L) -or
                 $null -eq $review.reference_rescan_candidates_matched -or
                 $review.reference_rescan_candidates_matched -ne $report.candidates.Count -or
                 $null -eq $review.reference_rescan_grid_steps -or
                 $review.reference_rescan_grid_steps -le 0 -or
                 $review.reference_rescan_grid_steps -gt 100000)) {
                throw "Independent reference full-span rescan missing or inconsistent for $filename"
            }
            if ($AdaptiveStationDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
                ($review.producer_claimed_adaptive_station_check -ne $true -or
                 $review.adaptive_station_check_independently_recomputed -ne $false)) {
                throw "Independent Rust review rejected adaptive station metadata for $filename"
            }
            if ($AdaptiveIngressDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
                ($review.producer_claimed_adaptive_ingress_check -ne $true -or
                 $review.adaptive_ingress_check_independently_recomputed -ne $false)) {
                throw "Independent Rust review rejected adaptive ingress metadata for $filename"
            }
            if ($AdaptiveLunarDepth -gt 0 -and $report.sparse_spans.Count -gt 0 -and
                ($review.producer_claimed_adaptive_lunar_check -ne $true -or
                 $review.adaptive_lunar_check_independently_recomputed -ne $false)) {
                throw "Independent Rust review rejected adaptive lunar metadata for $filename"
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
