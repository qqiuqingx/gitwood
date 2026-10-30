# Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
# Usage: .\scripts\make-fixture-repos.ps1 D:\Temp\gitwood-fixtures
param([string]$Destination = (Join-Path ([IO.Path]::GetTempPath()) 'gitwood-fixtures'))
$ErrorActionPreference = 'Stop'
if (Get-Command py -ErrorAction SilentlyContinue) {
    & py -3 (Join-Path $PSScriptRoot 'make-fixture-repos.py') $Destination
} else {
    & python3 (Join-Path $PSScriptRoot 'make-fixture-repos.py') $Destination
}
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
