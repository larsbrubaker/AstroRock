# One-time WAV -> MP3 conversion for the AstroRock port.
#
#   scripts/convert-audio.ps1 -SourceRoot C:\Development\AstroRock [-Ffmpeg <path>]
#
# SOUND/*.WAV  -> assets/sfx/<name>.mp3    (embedded into the binary later)
# Music/*.wav  -> assets/music/track0N.mp3 (fetched at runtime, never embedded)
#
# The originals are mono PCM (SFX mostly 8-bit 11/22 kHz, music 22 kHz
# 16-bit); 128 kbps mono LAME is transparent for this material.

param(
    [Parameter(Mandatory = $true)][string]$SourceRoot,
    [string]$Ffmpeg = "ffmpeg"
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot

$sfxOut = Join-Path $repo "assets/sfx"
$musicOut = Join-Path $repo "assets/music"
New-Item -ItemType Directory -Force $sfxOut | Out-Null
New-Item -ItemType Directory -Force $musicOut | Out-Null

$failed = $false

foreach ($wav in Get-ChildItem (Join-Path $SourceRoot "SOUND") -Filter *.wav) {
    $name = $wav.BaseName.ToLowerInvariant()
    & $Ffmpeg -y -loglevel error -i $wav.FullName -codec:a libmp3lame -b:a 128k `
        (Join-Path $sfxOut "$name.mp3")
    if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $($wav.Name)"; $failed = $true }
}

foreach ($wav in Get-ChildItem (Join-Path $SourceRoot "Music") -Filter *.wav) {
    $name = $wav.BaseName.ToLowerInvariant()
    & $Ffmpeg -y -loglevel error -i $wav.FullName -codec:a libmp3lame -b:a 128k `
        (Join-Path $musicOut "$name.mp3")
    if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $($wav.Name)"; $failed = $true }
}

if ($failed) { exit 1 }
Write-Host "SFX:   $((Get-ChildItem $sfxOut -Filter *.mp3).Count) files"
Write-Host "Music: $((Get-ChildItem $musicOut -Filter *.mp3).Count) files"
