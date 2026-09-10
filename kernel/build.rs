use std::{env, fs, path::PathBuf};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::copy("memory.x", output.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", output.display());
    println!("cargo:rerun-if-changed=memory.x");
}
