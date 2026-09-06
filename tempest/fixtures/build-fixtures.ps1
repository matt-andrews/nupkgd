#Requires -Version 7
<#
.SYNOPSIS
    Rebuilds the .nupkg fixtures served by nupkgd during the Tempest integration run.

.DESCRIPTION
    Every src/<Id>.<Version>.nuspec becomes <Id>.<Version>.nupkg containing a single
    <Id>.nuspec entry. nupkgd only reads the first .nuspec inside the zip, so a nuspec-only
    package is a valid fixture. Broken.Package.1.0.0.nupkg is deliberately not a zip so the
    suite can prove unreadable files are ignored at startup.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$fixtures = $PSScriptRoot
$src = Join-Path $fixtures 'src'

Get-ChildItem -Path $fixtures -Filter '*.nupkg' | Remove-Item -Force

foreach ($nuspec in Get-ChildItem -Path $src -Filter '*.nuspec' | Sort-Object Name) {
    [xml]$doc = Get-Content -Raw $nuspec.FullName
    $id = $doc.package.metadata.id
    $version = $doc.package.metadata.version
    $out = Join-Path $fixtures "$id.$version.nupkg"

    $stream = [System.IO.File]::Open($out, [System.IO.FileMode]::CreateNew)
    try {
        $zip = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            $entry = $zip.CreateEntry("$id.nuspec", [System.IO.Compression.CompressionLevel]::Optimal)
            $entryStream = $entry.Open()
            try {
                $bytes = [System.IO.File]::ReadAllBytes($nuspec.FullName)
                $entryStream.Write($bytes, 0, $bytes.Length)
            } finally { $entryStream.Dispose() }
        } finally { $zip.Dispose() }
    } finally { $stream.Dispose() }

    Write-Host "built $(Split-Path -Leaf $out)"
}

$broken = Join-Path $fixtures 'Broken.Package.1.0.0.nupkg'
Set-Content -Path $broken -Value 'this is not a zip archive and must be ignored by nupkgd' -NoNewline
Write-Host "built $(Split-Path -Leaf $broken) (intentionally invalid)"
