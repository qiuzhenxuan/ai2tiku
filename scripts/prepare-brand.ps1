$ErrorActionPreference = 'Stop'
$projectDir = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = Join-Path $projectDir 'src/assets/brand/infinity-brush.png'
$encoded = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($source))
$inputSvg = Join-Path ([System.IO.Path]::GetTempPath()) 'ai2tiku-brush-app-icon.svg'
# The generated brush painting remains unchanged; the SVG only provides the icon's paper tile.
$svg = '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512"><rect x="16" y="16" width="480" height="480" rx="108" fill="#fafafa" stroke="#dedede" stroke-width="1.5"/><image x="32" y="144" width="448" height="224" href="data:image/png;base64,' + $encoded + '"/></svg>'
[System.IO.File]::WriteAllText($inputSvg,$svg)
Push-Location $projectDir
try {
    pnpm tauri icon $inputSvg --output src-tauri/icons
    if ($LASTEXITCODE -ne 0) { throw 'Icon generation failed' }
    Copy-Item -LiteralPath (Join-Path $projectDir 'src-tauri/icons/32x32.png') -Destination (Join-Path $projectDir 'src/assets/brand/favicon.png')
    $iconRoot = (Resolve-Path -LiteralPath (Join-Path $projectDir 'src-tauri/icons')).Path
    foreach ($folder in @('android','ios')) {
        $generatedPath = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($iconRoot,$folder))
        if ([System.IO.Path]::GetDirectoryName($generatedPath) -ne $iconRoot) { throw 'Unexpected icon path' }
        if (Test-Path -LiteralPath $generatedPath) { Remove-Item -LiteralPath $generatedPath -Recurse -Force }
    }
} finally { Pop-Location }
