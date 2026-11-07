use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=WII_OS_DOLPHIN_FFI_LIB_DIR");
    if env::var_os("CARGO_FEATURE_LINKED_DOLPHIN").is_none() {
        return;
    }

    let library_dir = env::var_os("WII_OS_DOLPHIN_FFI_LIB_DIR")
        .map(PathBuf::from)
        .expect("linked-dolphin requires WII_OS_DOLPHIN_FFI_LIB_DIR from the Dolphin CMake build");
    if !library_dir.is_dir() {
        panic!("Dolphin FFI library directory does not exist: {}", library_dir.display());
    }
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!("cargo:rustc-link-lib=dylib=wii_dolphin_ffi");
}
