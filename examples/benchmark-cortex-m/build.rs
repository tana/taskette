use std::{env, fs, path::Path};

#[cfg(feature = "rp2040")]
const MEMORY_X: &str = "memory-rp2040.x";
#[cfg(feature = "rp235x")]
const MEMORY_X: &str = "memory-rp235x.x";

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    fs::copy(MEMORY_X, out_dir.join("memory.x")).unwrap();

    println!("cargo::rustc-link-search={}", out_dir.to_str().unwrap());
}