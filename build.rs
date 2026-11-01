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
}
