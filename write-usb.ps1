param(
    [Parameter(Mandatory)][int]$DiskNumber,
    [string]$Image = "$PSScriptRoot\target\uefi.img"
)
$ErrorActionPreference = "Stop"

$admin = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $admin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this from an elevated PowerShell."
}
if (-not (Test-Path $Image)) { throw "Image not found: $Image. Run run.ps1 first." }

$disk = Get-Disk -Number $DiskNumber
if ($disk.BusType -ne "USB") { throw "Disk $DiskNumber is $($disk.BusType), not USB. Refusing." }
if ($disk.IsBoot -or $disk.IsSystem) { throw "Disk $DiskNumber holds Windows. Refusing." }
if ($disk.LogicalSectorSize -ne 512) { throw "Only 512-byte sector drives are supported." }

$size = (Get-Item $Image).Length
if ($size % 512 -ne 0 -or $size -gt $disk.Size) { throw "Image size is not usable on this disk." }

"About to ERASE disk $DiskNumber : $($disk.FriendlyName), $([math]::Round($disk.Size / 1GB, 1)) GB"
if ((Read-Host "Type the disk number again to confirm") -ne "$DiskNumber") { throw "Cancelled." }

Clear-Disk -Number $DiskNumber -RemoveData -RemoveOEM -Confirm:$false
$stream = [IO.File]::Open("\\.\PhysicalDrive$DiskNumber", "Open", "Write", "ReadWrite")
try {
    $bytes = [IO.File]::ReadAllBytes($Image)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
} finally {
    $stream.Dispose()
}
Update-Disk -Number $DiskNumber
"Wrote $size bytes to disk $DiskNumber. Eject it, then boot from it via the firmware boot menu."
