use std::path::PathBuf;
use bootloader::{BiosBoot, UefiBoot};

fn main() {
    let workspace_root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();

    let kernel_path = workspace_root
        .join("target")
        .join("x86_64-unknown-none")
        .join("debug")
        .join("os");

    if !kernel_path.exists() {
        eprintln!("  [boot] Kernel binary not found: {}", kernel_path.display());
        eprintln!("  [boot] Run: make build");
        std::process::exit(1);
    }

    // --- BIOS image ---
    let bios_img_path = workspace_root.join("os-bios.img");
    eprintln!("  [boot] Creating BIOS disk image (os-bios.img)...");
    BiosBoot::new(&kernel_path)
        .create_disk_image(&bios_img_path)
        .expect("failed to create BIOS disk image");
    eprintln!("  [boot] Done: {}", bios_img_path.display());

    // --- UEFI image ---
    let uefi_img_path = workspace_root.join("os-uefi.img");
    eprintln!("  [boot] Creating UEFI disk image (os-uefi.img)...");
    UefiBoot::new(&kernel_path)
        .create_disk_image(&uefi_img_path)
        .expect("failed to create UEFI disk image");
    eprintln!("  [boot] Done: {}", uefi_img_path.display());

    // --- Cargo build triggers ---
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR");
    println!("cargo:rerun-if-changed=../os");
}
