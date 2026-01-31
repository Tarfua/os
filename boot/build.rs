use std::path::PathBuf;

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

    let image_path = workspace_root.join("os.img");
    eprintln!("  [boot] Creating disk image (os.img)...");

    bootloader::DiskImageBuilder::new(kernel_path.clone())
        .create_bios_image(&image_path)
        .expect("failed to create disk image");

    eprintln!("  [boot] Done: {}", image_path.display());

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR");
    println!("cargo:rerun-if-changed=../os");
}
