<#
.SYNOPSIS
    Génère les variantes personnelles du thème « Classic v1 » :
    une rangée Fable, et une option sans jauges (widget étroit).

.DESCRIPTION
    Pourquoi ce script existe :

    1. RANGÉE FABLE. L'API d'usage expose une limite hebdomadaire propre à un
       modèle (`weekly_scoped`, aujourd'hui Fable) que le thème officiel
       n'affiche pas. Les variantes générées ici ajoutent une 3e rangée qui
       n'apparaît QUE si l'API rapporte cette limite. La mise en page est
       pilotée par des expressions (`claude.scoped.available`) : deux rangées
       quand Fable est absent, trois quand il est présent, sans changer la
       hauteur du widget — la barre des tâches rogne ce qui dépasse.

    2. SANS JAUGES. Le widget occupe beaucoup de largeur et chevauche le bouton
       Démarrer sur un écran étroit. La jauge segmentée est ce qui prend le plus
       de place, alors que le pourcentage chiffré porte la même information.
       La variante compacte masque les jauges et resserre la largeur d'environ
       moitié, en gardant libellés et pourcentages.

    Pourquoi des THÈMES et non une option codée en Rust : depuis la v2.x le
    widget est entièrement décrit par un thème JSON. Passer par un thème veut
    dire zéro modification du code de rendu, donc rien à re-appliquer ni à
    reperdre aux mises à jour officielles. Le script REGÉNÈRE depuis le thème
    officiel embarqué : si l'auteur fait évoluer « Classic v1 », relancer ce
    script reporte ses évolutions.

    (La rangée Fable a besoin, elle, que le poller lise le tableau `limits[]` :
    c'est la part Rust, dans src/poller/claude.rs.)

.PARAMETER Install
    Copier les thèmes générés dans le dossier des thèmes utilisateur
    (%APPDATA%\ClaudeCodeUsageMonitor\themes) pour qu'ils apparaissent dans
    l'application.

.EXAMPLE
    pwsh -File scripts\make-themes.ps1
    pwsh -File scripts\make-themes.ps1 -Install
#>

[CmdletBinding()]
param(
    [switch]$Install,
    # Écran qui accueille le widget, tel que numéroté par l'application :
    # 0 = écran principal, 1 = suivant, etc. (les écrans sont ordonnés principal
    # d'abord, puis de gauche à droite). Le placement fait partie du thème.
    [int]$Display = 0
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Source      = Join-Path $ProjectRoot 'src\themes\classic-usage-widget.json'
$OutputDir   = Join-Path $ProjectRoot 'themes'

if (-not (Test-Path $Source)) { throw "Theme source introuvable : $Source" }

# Sans -Display explicite, reprendre l'écran du thème déjà installé : une simple
# réinstallation ne doit pas ramener le widget sur l'écran principal alors que
# l'utilisateur l'a déplacé.
if (-not $PSBoundParameters.ContainsKey('Display')) {
    $installed = Join-Path $env:APPDATA 'ClaudeCodeUsageMonitor\themes\classic-fable.json'
    if (Test-Path $installed) {
        try {
            $current = (Get-Content -Raw $installed | ConvertFrom-Json).surfaces[0].placement.reference.display
            if ($null -ne $current) {
                $Display = [int]$current
                Write-Host "  ecran conserve : $Display"
            }
        } catch {
            Write-Host "  ecran du theme installe illisible, retour a l'ecran principal"
        }
    }
}

# Largeur d'une jauge dans le thème officiel, telle qu'écrite dans le JSON.
$GaugeExpr = 'ceil(10 / max(1, providers.count)) * 11 - 1'

# Bandes verticales. Sans Fable on garde 5 / 28 ; avec Fable les deux premières
# remontent pour loger la troisième à 32, dans la même hauteur de 46 px.
$SessionY = "5 - claude.scoped.available * 3"    # 5 -> 2
$WeeklyY  = "28 - claude.scoped.available * 11"  # 28 -> 17
$ScopedY  = "32"

# Le libellé de la rangée Fable est un nom de modèle ("Fable"), bien plus large
# que "5h" ou "7d" : il déborde sur la colonne des pourcentages, qui commence à
# 41. On élargit donc la colonne des libellés et on décale les fournisseurs
# d'autant — uniquement quand la rangée est là, pour ne pas élargir le widget
# le reste du temps.
$ScopedLabelWidth = 34                        # 13 -> 47
$LabelShift       = "claude.scoped.available * 14"

function Copy-Layer($layer) {
    # Copie profonde : les calques clonés doivent être indépendants.
    $layer | ConvertTo-Json -Depth 60 | ConvertFrom-Json
}

function Set-Prop($object, [string]$name, $value) {
    if ($null -eq $object.PSObject.Properties[$name]) {
        $object | Add-Member -NotePropertyName $name -NotePropertyValue $value
    } else {
        $object.$name = $value
    }
}

function New-ThemeVariant([bool]$NoGauges, [string]$Id, [string]$Name) {
    $theme = Get-Content -Raw -Encoding UTF8 $Source | ConvertFrom-Json
    $surface = $theme.surfaces[0]
    $layers = [System.Collections.ArrayList]::new()
    foreach ($l in $surface.children) { [void]$layers.Add($l) }

    # --- Rangée Fable : cloner les calques de la rangée hebdo -----------------
    $added = 0
    foreach ($sourceId in @(
        'weekly-label-dark', 'weekly-label-light',
        'claude-weekly-segments-dark', 'claude-weekly-segments-light',
        'claude-weekly-value-dark', 'claude-weekly-value-light'
    )) {
        $src = $layers | Where-Object { $_.id -eq $sourceId } | Select-Object -First 1
        if (-not $src) { throw "Calque '$sourceId' introuvable - le theme officiel a change de forme." }

        $clone = Copy-Layer $src
        $isDark = $sourceId -like '*-dark'
        $available = 'claude.scoped.available'
        $darkTerm = if ($isDark) { 'system.dark' } else { '(1 - system.dark)' }

        Set-Prop $clone 'id'     ($sourceId -replace 'weekly', 'scoped')
        Set-Prop $clone 'name'   ("Fable row - " + $clone.id)
        Set-Prop $clone 'render' "$darkTerm * $available"
        Set-Prop $clone 'y'      $ScopedY

        if ($sourceId -like '*-label-*') {
            Set-Prop $clone 'width' "$ScopedLabelWidth"
            $clone.content.template = '{claude.scoped.label}'
        } elseif ($sourceId -like '*-segments-*') {
            $clone.content.value = 'claude.scoped.percentage'
        } else {
            $clone.content.template = '{claude.scoped:usage_line}'
        }

        [void]$layers.Add($clone)
        $added++
    }

    # --- Remonter les deux rangées existantes quand Fable est là -------------
    $moved = 0
    foreach ($layer in $layers) {
        if ($null -eq $layer.PSObject.Properties['y']) { continue }
        if ($layer.id -like '*scoped*') { continue }
        if ("$($layer.y)" -eq '5')  { $layer.y = $SessionY; $moved++ }
        if ("$($layer.y)" -eq '28') { $layer.y = $WeeklyY;  $moved++ }
    }
    if ($moved -eq 0) { throw "Aucune rangee repositionnee - le theme officiel a change de forme." }

    # --- Décaler la colonne des fournisseurs pour dégager le libellé Fable ----
    $row = $layers | Where-Object { $_.id -eq 'provider-row' } | Select-Object -First 1
    if (-not $row) { throw "Calque 'provider-row' introuvable - le theme officiel a change de forme." }
    Set-Prop $row 'x'     "$($row.x) + $LabelShift"
    Set-Prop $row 'width' "$($row.width) - $LabelShift"

    # --- Variante sans jauges ------------------------------------------------
    $hidden = 0
    if ($NoGauges) {
        foreach ($layer in $layers) {
            if ($layer.id -like '*-segments-*') {
                Set-Prop $layer 'render' '0'
                $hidden++
            } elseif ($layer.id -like '*-value-*') {
                # Le pourcentage reprend la place libérée par la jauge.
                Set-Prop $layer 'x' '0'
            } elseif ($layer.id -like '*-provider') {
                # Largeur du groupe : "<jauge> + 4 + <largeur texte>" -> "<largeur texte>"
                foreach ($w in 62, 90) {
                    $layer.width = "$($layer.width)".Replace("$GaugeExpr + 4 + $w", "$w")
                }
            }
        }
        if ($hidden -eq 0) { throw "Aucune jauge masquee - le theme officiel a change de forme." }
        foreach ($w in 62, 90) {
            $surface.width = "$($surface.width)".Replace("$GaugeExpr + 4 + $w", "$w")
        }
    }

    # Le widget s'élargit du même décalage, sinon le dernier fournisseur est coupé.
    $surface.width = "$($surface.width) + $LabelShift"

    $surface.children = $layers.ToArray()

    # Écran et ancrage vont ensemble. La position vaut « point d'ancrage moins
    # (facteur x largeur) » : sur l'écran principal on aligne le bord DROIT du
    # widget sur le bord gauche de la zone de notification, donc juste à sa
    # gauche. Les autres écrans n'ont pas de zone de notification ; le même
    # réglage mesurerait alors depuis le bord gauche de la barre des tâches et
    # placerait le widget une largeur entière hors écran. On y aligne donc le
    # bord GAUCHE sur le bord gauche de la barre, là où l'espace est libre.
    Set-Prop $surface.placement.reference 'display' $Display
    if ($Display -eq 0) {
        Set-Prop $surface.placement.reference 'region' 'system_tray'
        Set-Prop $surface.placement 'surface_horizontal' 'right'
    } else {
        Set-Prop $surface.placement.reference 'region' 'taskbar'
        Set-Prop $surface.placement 'surface_horizontal' 'left'
    }
    Set-Prop $surface.placement 'horizontal' 'left'
    Set-Prop $surface.placement 'offset_x' 0
    Set-Prop $theme 'id'   $Id
    Set-Prop $theme 'name' $Name

    $json = $theme | ConvertTo-Json -Depth 60
    $output = Join-Path $OutputDir "$Id.json"
    if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null }
    # UTF-8 sans BOM : un BOM casserait la lecture JSON.
    [System.IO.File]::WriteAllText($output, $json, (New-Object System.Text.UTF8Encoding($false)))

    Write-Host "  $output"
    Write-Host "     calques Fable ajoutes : $added | rangees repositionnees : $moved | jauges masquees : $hidden"
    return $output
}

Write-Host 'Generation des themes...'
$generated = @(
    New-ThemeVariant -NoGauges $false -Id 'classic-fable'         -Name 'Classic + Fable'
    New-ThemeVariant -NoGauges $true  -Id 'classic-fable-compact' -Name 'Classic + Fable (sans jauges)'
)

if ($Install) {
    $themesDir = Join-Path $env:APPDATA 'ClaudeCodeUsageMonitor\themes'
    if (-not (Test-Path $themesDir)) { New-Item -ItemType Directory -Path $themesDir -Force | Out-Null }
    foreach ($file in $generated) {
        Copy-Item -Path $file -Destination (Join-Path $themesDir (Split-Path -Leaf $file)) -Force
    }
    Write-Host ''
    Write-Host "Installes dans : $themesDir"
    Write-Host "Choisir le theme dans l'application (clic droit sur le widget)."
}
