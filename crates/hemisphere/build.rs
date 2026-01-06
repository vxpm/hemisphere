use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ipl_hle_dol = manifest_dir.join("../../local/ipl-hle.dol");
    println!("cargo::rerun-if-changed={}", ipl_hle_dol.display());

    if !std::fs::exists(ipl_hle_dol).unwrap_or_default() {
        println!(
            "cargo::error=\"ipl-hle.dol not found in local resources folder, please build it using 'j́ust ipl-hle build'\""
        );
    }
}
