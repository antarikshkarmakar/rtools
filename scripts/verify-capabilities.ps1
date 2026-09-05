[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$CapabilityDoc = Join-Path $RepoRoot 'docs/operations/capabilities.md'
$McpDoc = Join-Path $RepoRoot 'docs/MCP.md'
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

    $McpRowPattern = '^\|\s*`(?<tool>[a-z0-9_]+)`\s*\|\s*`(?<operation>[a-z0-9._-]+)`\s*\|\s*`(?<state>available|experimental|unavailable)`\s*\|(?<contract>.*)\|$'
    $McpRows = @{}
    Get-Content -LiteralPath $McpDoc | ForEach-Object {
        if ($_ -match $McpRowPattern) {
            $McpRows[$Matches.tool] = [pscustomobject]@{
                OperationId = $Matches.operation
                State = $Matches.state
                Contract = $Matches.contract
            }
        }
    }
    $ExpectedMcp = [ordered]@{
        compress_image = 'image.compress'
        convert_image = 'image.convert'
        resize_image = 'image.resize'
        organize_photos = 'ai.organize.date'
        rename_photos = 'ai.rename.deterministic'
        generate_alt_text = 'ai.alt_text'
        find_duplicates = 'ai.duplicates.report'
        compress_pdf = 'pdf.compress'
        merge_pdfs = 'pdf.merge'
        extract_text = 'ai.ocr'
        get_metadata = 'image.exif.json'
    }
    if ($McpRows.Count -ne $ExpectedMcp.Count) {
        Fail-CapabilityVerification 'MCP adapter contract tools differ from the verified tool set'
    }
    foreach ($Tool in $ExpectedMcp.Keys) {
        if (-not $McpRows.ContainsKey($Tool)) {
            Fail-CapabilityVerification "MCP adapter contract is missing $Tool"
        }
        $Row = $McpRows[$Tool]
        $ExpectedOperation = $ExpectedMcp[$Tool]
        if ($Row.OperationId -cne $ExpectedOperation) {
            Fail-CapabilityVerification "MCP tool $Tool maps to $($Row.OperationId), expected $ExpectedOperation"
        }
        if ($Documented[$ExpectedOperation] -cne $Row.State) {
            Fail-CapabilityVerification "MCP tool $Tool state $($Row.State) differs from capability $($Documented[$ExpectedOperation])"
        }
        if (-not $Row.Contract.Contains('`structured_errors=true`')) {
            Fail-CapabilityVerification "MCP tool $Tool lacks the structured error contract marker"
        }
    }
    if (-not $McpRows['compress_pdf'].Contract.Contains('`level=medium`')) {
        Fail-CapabilityVerification 'MCP compress_pdf must document medium as its only supported level'
    }

    Write-Output "verified $($RuntimeRows.Count) sorted capability rows and $($McpRows.Count) MCP adapter contracts"

    Push-Location -LiteralPath $RepoRoot
    try {
        & cargo test --locked -p rtools-mcp mcp_contract
        if ($LASTEXITCODE -ne 0) {
            Fail-CapabilityVerification "MCP adapter contract tests exited with status $LASTEXITCODE"
        }
        & cargo test --locked -p rtools-api recognized_but_unavailable_options_return_structured_501
        if ($LASTEXITCODE -ne 0) {
            Fail-CapabilityVerification "REST adapter contract test exited with status $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    Remove-Item -LiteralPath $DoctorJson -Force -ErrorAction SilentlyContinue
}
