param([switch]$Remove)
$ErrorActionPreference = "Stop"

$admin = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $admin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this from an elevated PowerShell."
}

$root = $PSScriptRoot
$kernel = "$root\kernel\target\x86_64-unknown-none\release\kernel"
$loader = Get-ChildItem "$root\builder\target\release\build" -Recurse -Filter bootloader-x86_64-uefi.efi -ErrorAction SilentlyContinue |
    Select-Object -First 1 -ExpandProperty FullName
$entryFile = "$root\target\esp-entry.txt"
$guidPattern = "\{[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\}"

$letter = (68..90 | ForEach-Object { [char]$_ } | Where-Object { -not (Test-Path "${_}:\") })[0]
$drive = "${letter}:"

function Mount-Esp { mountvol $drive /S; if ($LASTEXITCODE -ne 0) { throw "Could not mount the EFI system partition." } }
function Dismount-Esp { mountvol $drive /D | Out-Null }

if ($Remove) {
    if (-not (Test-Path $entryFile)) { throw "No entry recorded at $entryFile." }
    $guid = (Get-Content $entryFile -Raw).Trim()
    Mount-Esp
    try {
        bcdedit /delete $guid
        Remove-Item "$drive\EFI\rustos" -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item "$drive\kernel-x86_64" -Force -ErrorAction SilentlyContinue
        Remove-Item $entryFile
    } finally {
        Dismount-Esp
    }
    "Removed the rustos boot entry and files."
    return
}

if (-not (Test-Path $kernel) -or -not $loader) { throw "Build first: run .\run.ps1 once." }
if (Test-Path $entryFile) { throw "Already installed. Run with -Remove first." }

try {
    if (Confirm-SecureBootUEFI) {
        Write-Warning "Secure Boot is ON. The firmware will most likely refuse the unsigned rustos loader."
    }
} catch {
    Write-Warning "Could not read the Secure Boot state."
}

"This will:"
"  1. copy the loader to <EFI partition>\EFI\rustos\bootx64.efi and the kernel to <EFI partition>\kernel-x86_64"
"  2. back up the boot configuration to $root\target\bcd-backup"
"  3. add a firmware boot entry named rustos and boot it ONCE on the next restart"
"Windows stays the default; nothing is erased. Undo with: .\boot-from-esp.ps1 -Remove"
if ((Read-Host "Type YES to continue") -ne "YES") { throw "Cancelled." }

New-Item -ItemType Directory -Force "$root\target" | Out-Null
$backup = "$root\target\bcd-backup"
Remove-Item $backup -Force -ErrorAction SilentlyContinue
bcdedit /export $backup | Out-Null

Mount-Esp
try {
    $needed = (Get-Item $kernel).Length + (Get-Item $loader).Length
    $free = (Get-PSDrive -Name $letter).Free
    if ($free -lt $needed + 4MB) { throw "The EFI partition has too little free space." }
    New-Item -ItemType Directory -Force "$drive\EFI\rustos" | Out-Null
    Copy-Item $loader "$drive\EFI\rustos\bootx64.efi" -Force
    Copy-Item $kernel "$drive\kernel-x86_64" -Force

    $copied = bcdedit /copy "{bootmgr}" /d "rustos"
    $guid = [regex]::Match(($copied | Out-String), $guidPattern).Value
    if (-not $guid) { throw "Could not read the new entry id from bcdedit." }
    Set-Content $entryFile $guid
    bcdedit /set $guid path "\EFI\rustos\bootx64.efi" | Out-Null
    bcdedit /set "{fwbootmgr}" bootsequence $guid | Out-Null
} finally {
    Dismount-Esp
}
"Done. Restart: the PC boots rustos once, then goes back to Windows. Type diag in the Terminal window."
