$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    python -m PyInstaller --noconfirm --onefile --windowed --name Navegante-Web --icon "${projectRoot}/desktop-assets/app.ico" --distpath release --workpath analysis/desktop-build --specpath analysis/desktop-spec --add-data "${projectRoot}/web/index.html:web" --add-data "${projectRoot}/web/app.js:web" --add-data "${projectRoot}/web/style.css:web" --add-data "${projectRoot}/web/forms.css:web" desktop.py
    if ($LASTEXITCODE -ne 0) { throw 'Falhou a criação do executável.' }
    Get-FileHash -Algorithm SHA256 -LiteralPath release/Navegante-Web.exe
} finally {
    Pop-Location
}
