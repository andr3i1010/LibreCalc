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

    // The kernel starts home by its header; take that address from home without linking its code.
    if std::env::var("CARGO_PKG_NAME").unwrap() == "librecalc-kernel" {
        let home = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../build/home.sym");
        if home.exists() {
            println!("cargo:rustc-link-arg=--just-symbols={}", home.display());
        }
        println!("cargo:rerun-if-changed={}", home.display());
    }
}
