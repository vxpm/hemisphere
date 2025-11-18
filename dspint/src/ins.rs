mod opcodes;

pub use opcodes::{CondCode, ExtensionOpcode, Opcode};

include!(concat!(env!("OUT_DIR"), "/dsp_decoding_lut.rs"));

#[derive(Clone, Copy)]
pub struct Ins {
    pub base: u16,
    pub extra: u16,
}

impl std::fmt::Debug for Ins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let decoded = self.decoded();
        if let Some(extension) = decoded.extension {
            write!(
                f,
                "{:?}'{:?} ({:04X}:{:04X})",
                decoded.opcode, extension, self.base, self.extra
            )
        } else {
            write!(
                f,
                "{:?} ({:04X}:{:04X})",
                decoded.opcode, self.base, self.extra
            )
        }
    }
}

#[derive(Clone, Copy)]
pub struct Decoded {
    pub opcode: Opcode,
    pub extension: Option<ExtensionOpcode>,
    pub needs_extra: bool,
}

impl Decoded {
    pub fn len(self) -> u16 {
        if self.needs_extra { 2 } else { 1 }
    }
}

impl Ins {
    pub fn new(base: u16) -> Self {
        Self { base, extra: 0 }
    }

    pub fn with_extra(base: u16, extra: u16) -> Self {
        Self { base, extra }
    }

    pub fn decoded(self) -> Decoded {
        DECODING_LUT[self.base as usize]
    }
}

#[cfg(test)]
mod test {
    use super::{ExtensionOpcode, Opcode};
    use strum::VariantArray;

    #[test]
    fn unique_opcodes() {
        for value in 0..u16::MAX {
            let mut hit = None;
            for opcode in Opcode::VARIANTS {
                if opcode.info().is_some_and(|i| i.matches(value)) {
                    if let Some(hit) = hit {
                        panic!("opcodes {hit:?} and {opcode:?} are valid for {value:016b}");
                    }

                    hit = Some(*opcode);
                }
            }
        }
    }

    #[test]
    fn unique_extension_opcodes() {
        for value in 0..u16::MAX {
            let mut hit = None;
            for opcode in ExtensionOpcode::VARIANTS {
                if opcode.info().is_some_and(|i| i.matches(value)) {
                    if let Some(hit) = hit {
                        panic!(
                            "extension opcodes {hit:?} and {opcode:?} are valid for {value:016b}"
                        );
                    }

                    hit = Some(*opcode);
                }
            }
        }
    }
}
