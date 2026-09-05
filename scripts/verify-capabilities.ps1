[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$CapabilityDoc = if ($env:RTOOLS_CAPABILITY_DOC) {
    $env:RTOOLS_CAPABILITY_DOC
} else {
    Join-Path $RepoRoot 'docs/operations/capabilities.md'
}
$McpDoc = if ($env:RTOOLS_MCP_DOC) {
    $env:RTOOLS_MCP_DOC
} else {
    Join-Path $RepoRoot 'docs/MCP.md'
}
$DoctorJson = [System.IO.Path]::GetTempFileName()
$McpContractJson = [System.IO.Path]::GetTempFileName()
$NativeCargo = Get-Command cargo -ErrorAction SilentlyContinue
$WslExecutable = Get-Command wsl.exe -ErrorAction SilentlyContinue
$LastCargoExitCode = 0

function Invoke-Cargo {
    param([Parameter(Mandatory)][string[]]$CargoArguments)

    if ($NativeCargo) {
        & $NativeCargo.Source @CargoArguments
    }
    elseif ($WslExecutable) {
        & $WslExecutable.Source -e bash -lc 'cargo "$@"' cargo @CargoArguments
    }
    else {
        throw 'cargo is unavailable both natively and through WSL'
    }
    $script:LastCargoExitCode = $LASTEXITCODE
}

function Fail-CapabilityVerification {
    param([Parameter(Mandatory)][string]$Message)

    throw "capability verification failed: $Message"
}

try {
    Push-Location -LiteralPath $RepoRoot
    try {
        Invoke-Cargo -CargoArguments @('run', '--locked', '--quiet', '-p', 'rtools-cli', '--', '--output-format', 'json', 'doctor') > $DoctorJson
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "doctor exited with status $LastCargoExitCode"
        }
        Invoke-Cargo -CargoArguments @('run', '--locked', '--quiet', '-p', 'rtools-mcp', '--', '--print-contracts') > $McpContractJson
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "MCP contract export exited with status $LastCargoExitCode"
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
    try {
        $ExportedMcp = Get-Content -Raw -LiteralPath $McpContractJson | ConvertFrom-Json
        if ([int]$ExportedMcp.version -ne 1) {
            Fail-CapabilityVerification "MCP runtime contract has unsupported version $($ExportedMcp.version)"
        }
        $RuntimeMcpRows = @($ExportedMcp.tools)
    }
    catch {
        Fail-CapabilityVerification "cannot read MCP runtime contract: $($_.Exception.Message)"
    }
    if ($RuntimeMcpRows.Count -eq 0) {
        Fail-CapabilityVerification 'MCP runtime contract contains no tools'
    }
    $RuntimeMcpNames = @()
    foreach ($Row in $RuntimeMcpRows) {
        foreach ($Property in @('tool', 'operation_id', 'state', 'adapter_contract')) {
            $Value = [string]$Row.$Property
            if ([string]::IsNullOrWhiteSpace($Value)) {
                Fail-CapabilityVerification "MCP runtime contract contains an empty $Property"
            }
        }
        if (@('available', 'experimental', 'unavailable') -cnotcontains [string]$Row.state) {
            Fail-CapabilityVerification "MCP runtime contract has invalid state for $($Row.tool): $($Row.state)"
        }
        if ($Row.structured_errors -ne $true) {
            Fail-CapabilityVerification "MCP runtime contract disables structured errors for $($Row.tool)"
        }
        $RuntimeMcpNames += [string]$Row.tool
    }
    $RuntimeMcpDuplicates = @(
        $RuntimeMcpNames | Group-Object | Where-Object Count -gt 1 |
            ForEach-Object Name | Sort-Object -CaseSensitive
    )
    if ($RuntimeMcpDuplicates.Count -gt 0) {
        Fail-CapabilityVerification "MCP runtime contract contains duplicate tools: $($RuntimeMcpDuplicates -join ', ')"
    }

    $DocumentedMcpRows = @(
        Get-Content -LiteralPath $McpDoc | ForEach-Object {
            if ($_ -match $McpRowPattern) {
                [pscustomobject]@{
                    Tool = $Matches.tool
                    OperationId = $Matches.operation
                    State = $Matches.state
                    Contract = $Matches.contract
                }
            }
        }
    )
    $DocumentedMcpNames = @($DocumentedMcpRows | ForEach-Object Tool)
    $DocumentedMcpDuplicates = @(
        $DocumentedMcpNames | Group-Object | Where-Object Count -gt 1 |
            ForEach-Object Name | Sort-Object -CaseSensitive
    )
    if ($DocumentedMcpDuplicates.Count -gt 0) {
        Fail-CapabilityVerification "MCP documentation contains duplicate tools: $($DocumentedMcpDuplicates -join ', ')"
    }
    if (($DocumentedMcpNames -join "`n") -cne ($RuntimeMcpNames -join "`n")) {
        Fail-CapabilityVerification 'MCP adapter contract tools differ from the verified tool set'
    }

    for ($Index = 0; $Index -lt $RuntimeMcpRows.Count; $Index++) {
        $RuntimeRow = $RuntimeMcpRows[$Index]
        $DocRow = $DocumentedMcpRows[$Index]
        $Tool = [string]$RuntimeRow.tool
        if ($DocRow.OperationId -cne [string]$RuntimeRow.operation_id) {
            Fail-CapabilityVerification "MCP tool $Tool maps to $($DocRow.OperationId), expected $($RuntimeRow.operation_id)"
        }
        if ($DocRow.State -cne [string]$RuntimeRow.state) {
            Fail-CapabilityVerification "MCP tool $Tool state $($DocRow.State) differs from runtime MCP state $($RuntimeRow.state)"
        }
        if ($Documented[$DocRow.OperationId] -cne $DocRow.State) {
            Fail-CapabilityVerification "MCP tool $Tool state $($DocRow.State) differs from capability $($Documented[$DocRow.OperationId])"
        }
        $NormalizedContract = $DocRow.Contract.Replace('`', '').Replace('\|', '|').Trim()
        $ExpectedContract = "$($RuntimeRow.adapter_contract); structured_errors=true"
        if ($NormalizedContract -cne $ExpectedContract) {
            Fail-CapabilityVerification "MCP tool $Tool contract differs from runtime (runtime='$ExpectedContract', docs='$NormalizedContract')"
        }
    }

    Write-Output "verified $($RuntimeRows.Count) sorted capability rows and $($RuntimeMcpRows.Count) runtime-derived MCP adapter contracts"

    Push-Location -LiteralPath $RepoRoot
    try {
        Invoke-Cargo -CargoArguments @('test', '--locked', '-p', 'rtools-mcp', 'mcp_contract')
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "MCP adapter contract tests exited with status $LastCargoExitCode"
        }
        Invoke-Cargo -CargoArguments @('test', '--locked', '-p', 'rtools-api', 'recognized_but_unavailable_options_return_structured_501')
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "REST adapter contract test exited with status $LastCargoExitCode"
        }
        Invoke-Cargo -CargoArguments @('test', '--locked', '-p', 'rtools-api', 'rename_uses_one_isolated_batch_when_client_names_resemble_staging_names')
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "REST rename batch contract test exited with status $LastCargoExitCode"
        }
        Invoke-Cargo -CargoArguments @('test', '--locked', '-p', 'rtools-cli', 'webp_rejects_explicit_quality_but_allows_omitted_quality')
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "CLI quality contract test exited with status $LastCargoExitCode"
        }
        Invoke-Cargo -CargoArguments @('test', '--locked', '-p', 'rtools-core', 'portable_filename_component_rejects_superscript_devices_and_long_names')
        if ($LastCargoExitCode -ne 0) {
            Fail-CapabilityVerification "portable filename contract test exited with status $LastCargoExitCode"
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    Remove-Item -LiteralPath $DoctorJson -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $McpContractJson -Force -ErrorAction SilentlyContinue
}
