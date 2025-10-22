use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fake_ipl_dol = manifest_dir.join("../resources/fake-ipl.dol");
    println!("cargo::rerun-if-changed={}", fake_ipl_dol.display());

    if !std::fs::exists(fake_ipl_dol).unwrap_or_default() {
        println!(
            "cargo::error=\"fake-ipl.dol not found in resources folder, please build it using 'j́ust fake-ipl build'\""
        );
    }
}
