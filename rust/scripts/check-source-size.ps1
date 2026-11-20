$ErrorActionPreference = 'Stop'
$sourceRoot = Join-Path $PSScriptRoot '..\crates'
$violations = @(
    Get-ChildItem -LiteralPath $sourceRoot -Directory |
        ForEach-Object {
            $source = Join-Path $_.FullName 'src'
            if (Test-Path -LiteralPath $source) {
                Get-ChildItem -LiteralPath $source -Recurse -File -Filter '*.rs'
            }
        } |
        ForEach-Object {
            $lineCount = (Get-Content -LiteralPath $_.FullName | Measure-Object -Line).Lines
            if ($lineCount -gt 250) {
                [pscustomobject]@{ Lines = $lineCount; Path = $_.FullName }
            }
        }
)

if ($violations.Count -gt 0) {
    $violations | ForEach-Object { Write-Output "$($_.Path): $($_.Lines) lines (maximum 250)" }
    exit 1
}

Write-Output 'OK: all Rust source modules contain at most 250 lines.'
