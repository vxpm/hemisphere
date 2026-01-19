#[path = "src/ins/opcodes.rs"]
mod opcodes;
use std::io::Write;
use std::path::PathBuf;

use opcodes::{ExtensionOpcode, Opcode};

fn main() {
    let lut: Vec<String> = Vec::from_iter((0..(1 << 16)).map(|i| {
        let base = i as u16;
        let opcode = Opcode::find_match(base);
        let extension = opcode
            .has_extension()
            .then(|| ExtensionOpcode::find_match(base & opcode.extension_mask()));

        format!(
            "Decoded {{ opcode: Opcode::{:?}, extension: {}, needs_extra: {} }}",
            opcode,
            if let Some(ext) = extension {
                format!("Some(ExtensionOpcode::{ext:?})")
            } else {
                "None".to_string()
            },
            opcode.needs_extra(),
        )
    }));

    let lut = lut.join(", ");
    let code = format!("static DECODING_LUT: [Decoded; 1 << 16] = [{lut}];");

    let path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("dsp_decoding_lut.rs");
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(code.as_bytes()).unwrap();
}
