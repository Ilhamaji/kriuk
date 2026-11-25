# Builds a release body from every commit since the last *published* release.
# Docs, chores and CI edits are dropped: a README fix is not a release.
#
#   ./release.ps1 -Tag v0.3.7 -From v0.3.6 -Out release.md -Sha <sha256> -Repo Ilhamaji/kriuk
#
# -From is the last release tag, not the last tag: a build that was tagged but
# never published is folded into the next release instead of disappearing.
#
# Exit codes: 0 notes written, 2 nothing user-facing (skip the release), 1 error.
param(
  [Parameter(Mandatory = $true)][string]$Tag,
  [string]$From = '',
  [string]$Out = 'release.md',
  [string]$Sha = '',
  [string]$Repo = ''
)

$ErrorActionPreference = 'Stop'

# Commits that are never user-facing, and the sections that survive.
$DROP = @('docs', 'chore', 'ci', 'test', 'style', 'build')
$SECTIONS = [ordered]@{
  feat     = 'Added'
  refactor = 'Changed'
  perf     = 'Performance'
  fix      = 'Fixed'
  remove   = 'Removed'
  revert   = 'Reverted'
}

$base = $From
if (-not $base) { $base = (git describe --tags --abbrev=0 --match 'v*' "$Tag^" 2>$null) }
$range = if ($base) { "$base..$Tag" } else { $Tag }

$log = git log $range --no-merges --pretty=format:'%h%x09%s'
if (-not $log) {
  Write-Host "no commits in $range"
  exit 2
}

$found = [ordered]@{}
foreach ($key in $SECTIONS.Keys) { $found[$key] = New-Object System.Collections.ArrayList }

foreach ($line in $log) {
  $hash, $subject = $line -split "`t", 2
  if (-not $subject) { continue }

  $type = 'other'
  $scope = ''
  $text = $subject
  if ($subject -match '^(?<type>[a-zA-Z]+)(\((?<scope>[^)]+)\))?!?:\s*(?<text>.+)$') {
    $type = $Matches['type'].ToLower()
    $scope = $Matches['scope']
    $text = $Matches['text']
  }
  if ($DROP -contains $type) { continue }

  # A commit that only touches prose is not a release, whatever it is labelled.
  $paths = git show --name-only --pretty=format: $hash
  $code = $paths | Where-Object { $_ -and $_ -notmatch '^(docs/|\.github/|LICENSE|\.gitignore)|\.md$' }
  if (-not $code) { continue }

  $bullet = if ($scope) { "- **$($scope)**: $text" } else { "- $text" }
  $section = if ($SECTIONS.Contains($type)) { $type } else { 'other' }
  if (-not $found.Contains($section)) { $found[$section] = New-Object System.Collections.ArrayList }
  [void]$found[$section].Add($bullet)
}

$total = ($found.Values | ForEach-Object { $_.Count } | Measure-Object -Sum).Sum
if (-not $total) {
  Write-Host "no user-facing commits in $range"
  exit 2
}

$md = New-Object System.Text.StringBuilder
[void]$md.AppendLine('One portable `Kriuk.exe` for Windows 10/11. No installer, no service, no driver.')
[void]$md.AppendLine('')
foreach ($key in $found.Keys) {
  if ($found[$key].Count -eq 0) { continue }
  $title = if ($SECTIONS.Contains($key)) { $SECTIONS[$key] } else { 'Other changes' }
  [void]$md.AppendLine("## $title")
  foreach ($b in $found[$key]) { [void]$md.AppendLine($b) }
  [void]$md.AppendLine('')
}
[void]$md.AppendLine('## Install')
[void]$md.AppendLine('1. Download `Kriuk.exe` below.')
[void]$md.AppendLine('2. Run it. SmartScreen warns because the binary is unsigned: **More info**, then **Run anyway**.')
[void]$md.AppendLine('3. Open the panel from the tray icon. Settings persist in `%APPDATA%\Kriuk\settings.json`.')
[void]$md.AppendLine('')
if ($Sha) { [void]$md.AppendLine("SHA-256: ``$Sha``"); [void]$md.AppendLine('') }
if ($Repo -and $base) {
  [void]$md.AppendLine("Full changelog: https://github.com/$Repo/compare/$base...$Tag")
}

$target = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $PWD $Out }
[IO.File]::WriteAllText($target, $md.ToString(), (New-Object System.Text.UTF8Encoding $false))
Write-Host "wrote $Out ($total entries, $range)"
exit 0
