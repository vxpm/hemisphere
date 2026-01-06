fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-arg=-T{manifest}/linker.ld");
}
