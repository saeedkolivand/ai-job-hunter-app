# Download all OFL-licensed fonts required for new resume/cover-letter templates.
# Run from the fonts/ directory: .\download-fonts.ps1
#
# All fonts are licensed under the Open Font License (OFL-1.1).
# Sources: Google Fonts CDN (gstatic.com) + official GitHub releases.

$ErrorActionPreference = "Stop"
$dir = $PSScriptRoot
$tmp = "$dir\_tmp_fonts"
New-Item -ItemType Directory -Force $tmp | Out-Null

# Old Android 2.x UA — Google Fonts serves raw TTF for this (not WOFF/WOFF2).
$ttfUA = "Mozilla/5.0 (Linux; U; Android 2.2; en-us; Nexus One Build/FRF91) AppleWebKit/533.1 (KHTML, like Gecko) Version/4.0 Mobile Safari/533.1"

function Skip-IfExists([string]$Dest) {
    if (Test-Path $Dest) {
        Write-Host "  already exists: $(Split-Path $Dest -Leaf)" -ForegroundColor DarkGray
        return $true
    }
    return $false
}

# Download a single TTF from fonts.gstatic.com via Google Fonts CSS API.
# $Family  : URL-encoded family name, e.g. "Source+Serif+4"
# $Weight  : numeric weight, e.g. 400
# $Italic  : $true for italic variant
# $OutFile : destination path
function Get-GoogleFontTtf {
    param([string]$Family, [int]$Weight, [bool]$Italic = $false, [string]$OutFile)
    if (Skip-IfExists $OutFile) { return }
    $wSpec  = if ($Italic) { "${Weight}italic" } else { "$Weight" }
    $cssUrl = "https://fonts.googleapis.com/css?family=${Family}:${wSpec}&subset=latin"
    Write-Host "  fetching CSS for $Family $wSpec..." -ForegroundColor DarkGray
    $css = (Invoke-WebRequest -Uri $cssUrl -UserAgent $ttfUA -UseBasicParsing).Content
    # Grab any fonts.gstatic.com font URL — we request a single weight so there's only one.
    if (-not ($css -match "url\((https://fonts\.gstatic\.com/[^)]+\.(ttf|woff))\)")) {
        Write-Host "  --- CSS response (first 800 chars) ---" -ForegroundColor Magenta
        Write-Host ($css.Substring(0, [Math]::Min(800, $css.Length))) -ForegroundColor Magenta
        throw "No font URL found for $Family $wSpec — see debug output above"
    }
    if ($Matches[2] -ne "ttf") {
        Write-Host "  WARNING: Google returned .$($Matches[2]) instead of .ttf for $Family $wSpec" -ForegroundColor Magenta
        Write-Host "  URL: $($Matches[1])" -ForegroundColor Magenta
        throw "Got .$($Matches[2]) — need TTF. See UA or fallback options."
    }
    $ttfUrl = $Matches[1]
    Write-Host "  downloading: $(Split-Path $OutFile -Leaf)" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $ttfUrl -OutFile $OutFile -UseBasicParsing
}

# Download a TTF from a direct GitHub raw/release URL (for fonts not on Google Fonts).
function Get-DirectTtf {
    param([string]$Url, [string]$OutFile)
    if (Skip-IfExists $OutFile) { return }
    Write-Host "  downloading: $(Split-Path $OutFile -Leaf)" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
}

function Get-FontFromZip {
    param([string]$ZipUrl, [string]$ZipDest, [hashtable]$Extractions)
    $allExist = $true
    foreach ($out in $Extractions.Values) {
        if (-not (Test-Path "$dir\$out")) { $allExist = $false; break }
    }
    if ($allExist) {
        foreach ($out in $Extractions.Values) {
            Write-Host "  already exists: $out" -ForegroundColor DarkGray
        }
        return
    }
    Write-Host "  downloading zip: $(Split-Path $ZipDest -Leaf)" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $ZipUrl -OutFile $ZipDest -UseBasicParsing
    $extractDir = "$tmp\_extract_$(Get-Random)"
    Expand-Archive $ZipDest $extractDir -Force
    foreach ($kv in $Extractions.GetEnumerator()) {
        $leaf  = Split-Path $kv.Key -Leaf
        $found = Get-ChildItem -Path $extractDir -Recurse -Filter $leaf | Select-Object -First 1
        if ($found) {
            Copy-Item $found.FullName "$dir\$($kv.Value)" -Force
            Write-Host "  extracted: $($kv.Value)" -ForegroundColor Green
        } else {
            Write-Warning "NOT FOUND in zip: $leaf"
        }
    }
    Remove-Item $ZipDest -Force
    Remove-Item $extractDir -Recurse -Force
}

# ── Inter (rsms/inter v4.0 release zip) ───────────────────────────────────────
Write-Host "`nDownloading Inter fonts..." -ForegroundColor Yellow
Get-FontFromZip `
    -ZipUrl "https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip" `
    -ZipDest "$tmp\inter.zip" `
    -Extractions @{
        "Inter-Regular.ttf" = "inter_regular.ttf"
        "Inter-Bold.ttf"    = "inter_bold.ttf"
    }

# ── Source Serif 4 (Google Fonts CSS API → gstatic TTF) ───────────────────────
Write-Host "`nDownloading Source Serif 4 fonts..." -ForegroundColor Yellow
Get-GoogleFontTtf -Family "Source+Serif+4" -Weight 400 -Italic $false -OutFile "$dir\source_serif4_regular.ttf"
Get-GoogleFontTtf -Family "Source+Serif+4" -Weight 700 -Italic $false -OutFile "$dir\source_serif4_bold.ttf"
Get-GoogleFontTtf -Family "Source+Serif+4" -Weight 400 -Italic $true  -OutFile "$dir\source_serif4_italic.ttf"

# ── Manrope (Google Fonts CSS API → gstatic TTF) ──────────────────────────────
Write-Host "`nDownloading Manrope fonts..." -ForegroundColor Yellow
Get-GoogleFontTtf -Family "Manrope" -Weight 400 -Italic $false -OutFile "$dir\manrope_regular.ttf"
Get-GoogleFontTtf -Family "Manrope" -Weight 700 -Italic $false -OutFile "$dir\manrope_bold.ttf"

# ── JetBrains Mono (GitHub release zip) ───────────────────────────────────────
Write-Host "`nDownloading JetBrains Mono fonts..." -ForegroundColor Yellow
Get-FontFromZip `
    -ZipUrl "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip" `
    -ZipDest "$tmp\jetbrains.zip" `
    -Extractions @{
        "JetBrainsMono-Regular.ttf" = "jetbrains_mono_regular.ttf"
        "JetBrainsMono-Bold.ttf"    = "jetbrains_mono_bold.ttf"
    }

# ── Playfair Display (Google Fonts CSS API → gstatic TTF) ─────────────────────
Write-Host "`nDownloading Playfair Display fonts..." -ForegroundColor Yellow
Get-GoogleFontTtf -Family "Playfair+Display" -Weight 400 -Italic $false -OutFile "$dir\playfair_display_regular.ttf"
Get-GoogleFontTtf -Family "Playfair+Display" -Weight 700 -Italic $false -OutFile "$dir\playfair_display_bold.ttf"

# ── Cleanup ───────────────────────────────────────────────────────────────────
Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue

# ── Summary ───────────────────────────────────────────────────────────────────
Write-Host "`nFont download complete. File check:" -ForegroundColor Green
$expected = @(
    "inter_regular.ttf", "inter_bold.ttf",
    "source_serif4_regular.ttf", "source_serif4_bold.ttf", "source_serif4_italic.ttf",
    "manrope_regular.ttf", "manrope_bold.ttf",
    "jetbrains_mono_regular.ttf", "jetbrains_mono_bold.ttf",
    "playfair_display_regular.ttf", "playfair_display_bold.ttf"
)
$ok = $true
foreach ($f in $expected) {
    $path = "$dir\$f"
    if (Test-Path $path) {
        $size = (Get-Item $path).Length
        Write-Host ("  [OK] {0,-45} {1,6} KB" -f $f, [math]::Round($size/1KB)) -ForegroundColor Green
    } else {
        Write-Host "  [MISSING] $f" -ForegroundColor Red
        $ok = $false
    }
}
if (-not $ok) { Write-Host "`nSome fonts are missing — check errors above." -ForegroundColor Red; exit 1 }
