#!/usr/bin/env bash
# Run QEMU with os.img (cwd = project root). Serial = terminal (Stage 1.1/1.2).
# -display none + -serial stdio only (no -nographic: it also uses stdio for monitor = conflict).
cd "$(dirname "$0")"
exec stdbuf -o0 -e0 qemu-system-x86_64 \
  -machine q35 \
  -cpu qemu64 \
  -m 512M \
  -display none \
  -serial stdio \
  -drive file=os.img,format=raw
