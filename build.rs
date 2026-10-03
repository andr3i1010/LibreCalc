// SPDX-License-Identifier: GPL-3.0-only

fn main() {
    println!("cargo:rustc-link-search={}", env!("CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed=memory.x");
}
