$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$env:Path += ";$env:USERPROFILE\.cargo\bin;C:\Program Files\qemu"

Push-Location "$root\kernel"; cargo build --release; Pop-Location
New-Item -ItemType Directory -Force "$root\target" | Out-Null`nPush-Location "$root\builder"; cargo run --release -- "$root\kernel\target\x86_64-unknown-none\release\kernel" "$root\target\uefi.img"; Pop-Location

qemu-system-x86_64 `
    -machine q35 -m 512M `
    -drive if=pflash,format=raw,readonly=on,file="C:\Program Files\qemu\share\edk2-x86_64-code.fd" `
    -drive format=raw,file="$root\target\uefi.img" `
    -serial stdio

