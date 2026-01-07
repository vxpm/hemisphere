pub mod interpreter;

const fn convert_to_dsp_words<const N: usize>(bytes: &[u8]) -> [u16; N] {
    assert!(bytes.len() / 2 == N);

    let mut result = [0; N];
    let mut i = 0;
    loop {
        if i == N {
            break;
        }

        result[i] = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
        i += 1;
    }

    result
}

pub static DSP_ROM: [u16; 4096] = convert_to_dsp_words(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/dsp_rom.bin"
)));

pub static DSP_COEF: [u16; 2048] = convert_to_dsp_words(include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/dsp_coef.bin"
)));
