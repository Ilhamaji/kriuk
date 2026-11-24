param([string]$Path, [long]$Offset, [int]$Count)
$fs = [IO.File]::OpenRead($Path)
try {
  $fs.Position = $Offset
  $b = New-Object byte[] $Count
  $read = $fs.Read($b, 0, $Count)
  $slice = $b[0..($read - 1)]
  $hex = ($slice | ForEach-Object { $_.ToString('X2') }) -join ' '
  $asc = -join ($slice | ForEach-Object { if ($_ -ge 32 -and $_ -lt 127) { [char]$_ } else { '.' } })
  Write-Output "READ $read bytes at $Offset"
  Write-Output "HEX: $hex"
  Write-Output "ASC: $asc"
}
finally { $fs.Close() }
