#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

# --- Пошук OVMF firmware ---
OVMF_CODE_CANDIDATES=(
    /usr/share/edk2-ovmf/x64/OVMF_CODE.4m.fd
    /usr/share/OVMF/OVMF_CODE.fd
    /usr/share/OVMF/OVMF_CODE.rom
)

OVMF_VARS_CANDIDATES=(
    /usr/share/edk2-ovmf/x64/OVMF_VARS.4m.fd
    /usr/share/OVMF/OVMF_VARS.fd
    /usr/share/OVMF/OVMF_VARS.rom
)

OVMF_CODE=""
OVMF_VARS_TEMPLATE=""

for f in "${OVMF_CODE_CANDIDATES[@]}"; do
    if [ -f "$f" ]; then
        OVMF_CODE="$f"
        break
    fi
done

for f in "${OVMF_VARS_CANDIDATES[@]}"; do
    if [ -f "$f" ]; then
        OVMF_VARS_TEMPLATE="$f"
        break
    fi
done

if [[ -z "$OVMF_CODE" || -z "$OVMF_VARS_TEMPLATE" ]]; then
    echo "ERROR: Cannot find OVMF firmware (CODE or VARS)"
    exit 1
fi

OVMF_VARS=/tmp/OVMF_VARS.fd

# --- Вибір режиму: BIOS або UEFI ---
MODE="${1:-uefi}"  # default uefi

if [[ "$MODE" == "bios" ]]; then
    IMG=os-bios.img
elif [[ "$MODE" == "uefi" ]]; then
    IMG=os-uefi.img

    # завжди створюємо чисту writable копію для розробки
    cp "$OVMF_VARS_TEMPLATE" "$OVMF_VARS"
else
    echo "Unknown mode: $MODE. Use 'bios' or 'uefi'."
    exit 1
fi

if [ ! -f "$IMG" ]; then
    echo "Disk image '$IMG' not found. Run: cargo run -p boot"
    exit 1
fi

# --- Загальні параметри QEMU ---
QEMU_COMMON=(
  -m 512M
  -cpu qemu64
  -machine q35
  -serial stdio
  -display none
  -drive file="$IMG",format=raw
)

if [[ "$MODE" == "uefi" ]]; then
    exec stdbuf -o0 -e0 qemu-system-x86_64 \
        "${QEMU_COMMON[@]}" \
        -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
        -drive if=pflash,format=raw,readonly=off,file="$OVMF_VARS"
else
    exec stdbuf -o0 -e0 qemu-system-x86_64 "${QEMU_COMMON[@]}"
fi
