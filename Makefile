.PHONY: build image image-verbose run setup

# Install nightly components required by bootloader (llvm-tools). Run once.
setup:
	rustup component add llvm-tools-preview --toolchain nightly
	rustup target add x86_64-unknown-none --toolchain nightly

# Rebuild kernel if linker.ld changed (Cargo does not track it by default).
build:
	@kernel=target/x86_64-unknown-none/debug/os; \
	if [ -f "$$kernel" ] && [ os/linker.ld -nt "$$kernel" ]; then \
		rm -f "$$kernel" && cargo clean -p os; \
	fi
	cargo build -p os --target x86_64-unknown-none

# Depends on build so kernel binary exists. Touch build.rs so cargo re-runs it and creates fresh os.img.
image: build
	@echo "Building disk image..."
	@touch boot/build.rs
	cargo build -p boot

# Verbose: see cargo subprocess output (bootloader stages, our build.rs steps).
image-verbose:
	@echo "Building disk image (verbose)..."
	cargo build -p boot -vv

# Run in default mode (UEFI)
run: image
	./run-qemu.sh uefi

# Run in BIOS mode
run-bios: image
	./run-qemu.sh bios

# Run in UEFI mode explicitly
run-uefi: image
	./run-qemu.sh uefi
