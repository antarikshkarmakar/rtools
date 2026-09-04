[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$CapabilityDoc = Join-Path $RepoRoot 'docs/operations/capabilities.md'
$DoctorJson = [System.IO.Path]::GetTempFileName()

function Fail-CapabilityVerification {
    param([Parameter(Mandatory)][string]$Message)

    throw "capability verification failed: $Message"
}

try {
    Push-Location -LiteralPath $RepoRoot
    try {
        & cargo run --locked --quiet -p rtools-cli -- --output-format json doctor > $DoctorJson
        if ($LASTEXITCODE -ne 0) {
            Fail-CapabilityVerification "doctor exited with status $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }

    try {
        $Report = Get-Content -Raw -LiteralPath $DoctorJson | ConvertFrom-Json
        $RuntimeRows = @(
            $Report.result.capabilities | ForEach-Object {
                [pscustomobject]@{
                    OperationId = [string]$_.operation_id
                    State = [string]$_.state
                }
            }
        )
    }
    catch {
        Fail-CapabilityVerification "doctor did not emit the expected JSON report: $($_.Exception.Message)"
    }

    $RowPattern = '^\|\s*`(?<operation>[a-z0-9._-]+)`\s*\|\s*`(?<state>available|experimental|unavailable)`\s*\|'
    $DocumentedRows = @(
        Get-Content -LiteralPath $CapabilityDoc | ForEach-Object {
            if ($_ -match $RowPattern) {
                [pscustomobject]@{
                    OperationId = $Matches.operation
                    State = $Matches.state
                }
            }
        }
    )

    foreach ($Entry in @(
        [pscustomobject]@{ Label = 'doctor JSON'; Rows = $RuntimeRows },
        [pscustomobject]@{ Label = 'documentation'; Rows = $DocumentedRows }
    )) {
        $OperationIds = @($Entry.Rows | ForEach-Object { $_.OperationId })
        if ($OperationIds.Count -eq 0) {
            Fail-CapabilityVerification "$($Entry.Label) contains no capability rows"
        }
        $SortedIds = @($OperationIds | Sort-Object -CaseSensitive)
        if (($OperationIds -join "`n") -cne ($SortedIds -join "`n")) {
            Fail-CapabilityVerification "$($Entry.Label) operation IDs are not sorted"
        }
        $Duplicates = @(
            $OperationIds | Group-Object | Where-Object Count -gt 1 |
                ForEach-Object Name | Sort-Object -CaseSensitive
        )
        if ($Duplicates.Count -gt 0) {
            Fail-CapabilityVerification "$($Entry.Label) contains duplicate operation IDs: $($Duplicates -join ', ')"
        }
    }

    $Runtime = @{}
    foreach ($Row in $RuntimeRows) {
        $Runtime[$Row.OperationId] = $Row.State
    }
    $Documented = @{}
    foreach ($Row in $DocumentedRows) {
        $Documented[$Row.OperationId] = $Row.State
    }

    $Missing = @($Runtime.Keys | Where-Object { -not $Documented.ContainsKey($_) } | Sort-Object -CaseSensitive)
    $Extra = @($Documented.Keys | Where-Object { -not $Runtime.ContainsKey($_) } | Sort-Object -CaseSensitive)
    $Misclassified = @(
        $Runtime.Keys | Where-Object {
            $Documented.ContainsKey($_) -and $Runtime[$_] -cne $Documented[$_]
        } | Sort-Object -CaseSensitive
    )

    $Problems = @()
    if ($Missing.Count -gt 0) {
        $Problems += "missing from documentation: $($Missing -join ', ')"
    }
    if ($Extra.Count -gt 0) {
        $Problems += "extra in documentation: $($Extra -join ', ')"
    }
    if ($Misclassified.Count -gt 0) {
        $Details = @(
            $Misclassified | ForEach-Object {
                "$_ (runtime=$($Runtime[$_]), docs=$($Documented[$_]))"
            }
        )
        $Problems += "misclassified: $($Details -join ', ')"
    }
    if ($Problems.Count -gt 0) {
        Fail-CapabilityVerification ($Problems -join '; ')
    }

    Write-Output "verified $($RuntimeRows.Count) sorted capability rows"
}
finally {
    Remove-Item -LiteralPath $DoctorJson -Force -ErrorAction SilentlyContinue
}
