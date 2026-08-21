<#
.SYNOPSIS
    Installe la build locale du widget dans un dossier stable et fiabilise son
    démarrage automatique avec Windows.

.DESCRIPTION
    Pourquoi ce script existe :

    L'entrée de démarrage automatique pointait vers `target\release\`, le dossier
    de SORTIE DE COMPILATION. C'est fragile : un `cargo clean`, un build raté ou
    un changement de branche supprime ou remplace l'exécutable, et le démarrage
    automatique échoue alors EN SILENCE (l'app est un `windows_subsystem`, elle
    n'affiche aucune erreur). Constaté le 2026-08-14 : le widget ne tournait plus
    depuis un redémarrage, sans trace.

    Ce script copie l'exécutable vers un emplacement stable, hors du dossier de
    build, et fait pointer le démarrage automatique vers cette copie. Les
    recompilations ultérieures ne peuvent donc plus casser le démarrage : il faut
    relancer ce script pour publier une nouvelle version, ce qui est explicite.

    Cette build est volontairement HORS du circuit officiel (winget / releases) :
    elle porte la jauge Fable et le sélecteur d'écran, absents de la version
    publiée. Une mise à jour automatique la remplacerait par la version publiée.

.PARAMETER SkipBuild
    Ne pas recompiler : installer l'exécutable déjà présent dans target\release.

.PARAMETER NoDiagnose
    Ne pas ajouter `--diagnose` à la commande de démarrage automatique.
    Par défaut le diagnostic est ACTIF : sur un outil perso, pouvoir lire les
    logs de la version installée est un acquis, pas une gêne (journal dans
    %TEMP%\claude-code-usage-monitor.log).

.PARAMETER Uninstall
    Retirer l'entrée de démarrage automatique et le dossier d'installation.

.EXAMPLE
    pwsh -File scripts\install-local.ps1
    pwsh -File scripts\install-local.ps1 -SkipBuild
    pwsh -File scripts\install-local.ps1 -Uninstall
#>

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$NoDiagnose,
    [switch]$Uninstall,
    # Thème à activer après installation :
    #   classic-fable         = avec les jauges (écran large)
    #   classic-fable-compact = sans les jauges, environ moitié moins large
    # Omis : on garde le thème déjà actif.
    [ValidateSet('classic-fable', 'classic-fable-compact')]
    [string]$Theme,
    # Écran qui accueille le widget : 0 = principal, 1 = suivant.
    [int]$Display = 0
)

$ErrorActionPreference = 'Stop'

$ExeName     = 'claude-code-usage-monitor.exe'
$InstallDir  = Join-Path $env:LOCALAPPDATA 'ClaudeCodeUsageMonitor'
$InstallPath = Join-Path $InstallDir $ExeName
$RunKey      = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$RunValue    = 'ClaudeCodeUsageMonitor'
$ProjectRoot = Split-Path -Parent $PSScriptRoot

function Stop-Widget {
    # Arrêt CIBLÉ par nom de produit. Ne jamais viser un nom de runtime
    # générique : d'autres applications partagent ces processus.
    $procs = Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($ExeName)) -ErrorAction SilentlyContinue
    foreach ($p in $procs) {
        Write-Host "  arret du processus PID $($p.Id)"
        taskkill /PID $p.Id /T /F 2>&1 | Out-Null
    }
    if ($procs) { Start-Sleep -Milliseconds 700 }
}

if ($Uninstall) {
    Write-Host 'Desinstallation...'
    Stop-Widget
    if (Get-ItemProperty -Path $RunKey -Name $RunValue -ErrorAction SilentlyContinue) {
        Remove-ItemProperty -Path $RunKey -Name $RunValue
        Write-Host '  entree de demarrage automatique retiree'
    }
    if (Test-Path $InstallDir) {
        # Jamais de suppression seche : on deplace dans TEMP pour pouvoir revenir en arriere.
        $backup = Join-Path $env:TEMP ("ClaudeCodeUsageMonitor-desinstalle-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
        Move-Item -Path $InstallDir -Destination $backup
        Write-Host "  dossier deplace vers $backup"
    }
    Write-Host 'Termine.'
    return
}

if (-not $SkipBuild) {
    Write-Host 'Compilation (release)...'
    Push-Location $ProjectRoot
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build a echoue (code $LASTEXITCODE)" }
    } finally {
        Pop-Location
    }
}

$BuiltExe = Join-Path $ProjectRoot "target\release\$ExeName"
if (-not (Test-Path $BuiltExe)) {
    throw "Executable introuvable : $BuiltExe (compiler d'abord, ou retirer -SkipBuild)"
}

Write-Host 'Installation...'
Stop-Widget

# Thèmes personnels (rangée Fable, variante sans jauges). Régénérés depuis le
# thème officiel embarqué, pour suivre ses évolutions.
& (Join-Path $PSScriptRoot 'make-themes.ps1') -Install -Display $Display

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
Copy-Item -Path $BuiltExe -Destination $InstallPath -Force
Write-Host "  installe : $InstallPath"

# Thème actif. Écrit avant le lancement : au démarrage l'application migre les
# anciens réglages en créant son propre thème, et prendrait la main sinon.
if ($Theme) {
    $settingsPath = Join-Path $env:APPDATA 'ClaudeCodeUsageMonitor\settings.json'
    $themePath = Join-Path $env:APPDATA "ClaudeCodeUsageMonitor\themes\$Theme.json"
    if (-not (Test-Path $themePath)) { throw "Theme introuvable : $themePath" }

    $settings = if (Test-Path $settingsPath) {
        Get-Content -Raw $settingsPath | ConvertFrom-Json
    } else {
        [pscustomobject]@{}
    }
    $settings | Add-Member -NotePropertyName active_theme_path -NotePropertyValue $themePath -Force
    $settings | Add-Member -NotePropertyName custom_theme_enabled -NotePropertyValue $true -Force

    $parent = Split-Path -Parent $settingsPath
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    # UTF-8 sans BOM : un BOM casserait la lecture JSON.
    [System.IO.File]::WriteAllText($settingsPath, ($settings | ConvertTo-Json -Depth 20), (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "  theme actif : $Theme"
}

# Démarrage automatique -> la copie stable, jamais le dossier de build.
$command = if ($NoDiagnose) { "`"$InstallPath`"" } else { "`"$InstallPath`" --diagnose" }
Set-ItemProperty -Path $RunKey -Name $RunValue -Value $command
Write-Host "  demarrage automatique : $command"

# Relance depuis le nouvel emplacement, détachée de ce terminal.
Start-Process -FilePath $InstallPath -ArgumentList (@('--diagnose') | Where-Object { -not $NoDiagnose })
Start-Sleep -Seconds 3

$running = Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($ExeName)) -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "  widget relance (PID $($running.Id))" -ForegroundColor Green
} else {
    Write-Warning '  le widget ne tourne pas apres le lancement - verifier %TEMP%\claude-code-usage-monitor.log'
    exit 1
}

Write-Host ''
Write-Host 'Termine. Le demarrage automatique ne depend plus du dossier de compilation.' -ForegroundColor Green
