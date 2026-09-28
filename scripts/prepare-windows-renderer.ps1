$ErrorActionPreference = 'Stop'
# Build the renderer ourselves so exact sources, patches and notices travel with it.
$revision = '07f4812200df3d3c931c0c8a6081d3b21fe2bf9f'
$root = Join-Path $env:RUNNER_TEMP 'casy-vcpkg'
$distribution = Join-Path $env:RUNNER_TEMP 'casy-renderer'
function Checked { param([scriptblock]$Command) & $Command; if ($LASTEXITCODE -ne 0) { throw "Native command failed: $Command" } }
Checked { git clone --filter=blob:none https://github.com/microsoft/vcpkg.git $root }
Checked { git -C $root checkout $revision }
Checked { & "$root/bootstrap-vcpkg.bat" -disableMetrics }
$overlay = Join-Path $env:RUNNER_TEMP 'casy-ports'
New-Item -ItemType Directory -Force $overlay | Out-Null
Copy-Item "$root/ports/poppler" "$overlay/poppler" -Recurse
$port = "$overlay/poppler/portfile.cmake"
$content = Get-Content $port -Raw
if (-not $content.Contains('-DENABLE_UTILS=OFF')) { throw 'Unexpected pinned Poppler recipe' }
$content = $content.Replace('-DENABLE_UTILS=OFF', '-DENABLE_UTILS=ON')
$content = $content.Replace('vcpkg_cmake_install()', 'vcpkg_cmake_install()' + "`nvcpkg_copy_tools(TOOL_NAMES pdftoppm pdfattach pdfdetach pdffonts pdfimages pdfinfo pdfseparate pdftocairo pdftohtml pdftops pdftotext pdfunite AUTO_CLEAN)")
# pdftocairo is conditional on Cairo, which this minimal renderer does not enable.
$content = $content.Replace(' pdftocairo', '')
Set-Content $port $content
Checked { & "$root/vcpkg.exe" install 'poppler[core,zlib]:x64-windows-static-md' 'openssl:x64-windows-static-md' "--overlay-ports=$overlay" --no-binarycaching }
$installed = "$root/installed/x64-windows-static-md"
foreach ($dir in @('bin', 'lib', 'licenses/renderer/sources', 'licenses/renderer/recipes')) { New-Item -ItemType Directory -Force "$distribution/$dir" | Out-Null }
Copy-Item "$installed/tools/poppler/pdftoppm.exe" "$distribution/bin/"
Get-ChildItem "$installed/tools/poppler" -Filter '*.dll' | Copy-Item -Destination "$distribution/bin/"
Copy-Item "$installed/share" "$distribution/licenses/renderer/notices" -Recurse
# Source trees are vcpkg's exact patched build inputs; keep all dependency recipes too.
Get-ChildItem "$root/buildtrees" -Directory | ForEach-Object {
  if (Test-Path "$($_.FullName)/src") {
    $name = $_.Name
    Checked { tar -czf "$distribution/licenses/renderer/sources/$name.tar.gz" -C $_.FullName src }
  }
}
Copy-Item "$root/ports" "$distribution/licenses/renderer/recipes/ports" -Recurse
Copy-Item "$root/triplets" "$distribution/licenses/renderer/recipes/triplets" -Recurse
Copy-Item "$root/scripts" "$distribution/licenses/renderer/recipes/scripts" -Recurse
Copy-Item "$overlay/poppler" "$distribution/licenses/renderer/recipes/poppler-overlay" -Recurse
Copy-Item $PSCommandPath "$distribution/licenses/renderer/build.ps1"
Set-Content "$distribution/licenses/renderer/vcpkg-revision.txt" $revision
Add-Content $env:GITHUB_ENV "OPENSSL_DIR=$installed"
Add-Content $env:GITHUB_ENV 'OPENSSL_STATIC=1'
Add-Content $env:GITHUB_ENV "CASY_RENDERER_DISTRIBUTION=$distribution"
