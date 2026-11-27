// SPDX-License-Identifier: GPL-3.0-only

fn main() {
    let out = std::env::var_os("OUT_DIR").unwrap();
    std::fs::copy("memory.x", std::path::Path::new(&out).join("memory.x")).unwrap();
    println!(
        "cargo:rustc-link-search={}",
        std::path::Path::new(&out).display()
    );
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-changed=memory.x");

    // The kernel boots an app by its header: take that address from the app's build without
    // linking its code (the Makefile's BOOT picks the app).
    if std::env::var("CARGO_PKG_NAME").unwrap() == "librecalc-kernel" {
        let boot = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../build/boot.sym");
        if boot.exists() {
            println!("cargo:rustc-link-arg=--just-symbols={}", boot.display());
        }
        println!("cargo:rerun-if-changed={}", boot.display());
    }
}
