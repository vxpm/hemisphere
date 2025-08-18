use std::fmt::Display;

use crate::{Registers, Sequence};
use hemicore::Address;
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};

#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
pub enum Action {
    /// No special action to take
    #[default]
    None,
    /// Jump to the address in the `addr` field
    Jump,
}

pub union Data {
    raw: u32,
    pub addr: Address,
}

impl Default for Data {
    fn default() -> Self {
        Self { raw: 0 }
    }
}

#[derive(Default)]
pub struct BlockOutput {
    /// How many instructions were executed.
    pub executed: u32,
    /// An action requested by the block.
    pub action: Action,
    /// Data associated with the action.
    pub data: Data,
}

pub type BlockFn = extern "sysv64" fn(&mut Registers, &mut BlockOutput);

/// A compiled block of PowerPC instructions.
pub struct Block {
    seq: Sequence,
    clir: String,
    code: Mmap,
}

impl Block {
    /// # Safety
    /// `code` must be the bytes of a valid host function with the [`BlockFn`] signature.
    pub(crate) unsafe fn new(seq: Sequence, clir: String, code: &[u8]) -> Self {
        let mut map = MmapOptions::new().len(code.len()).map_anon().unwrap();
        map.copy_from_slice(code);

        Self {
            seq,
            clir,
            code: map.make_exec().unwrap(),
        }
    }

    /// Executes this block of instructions.
    #[inline(always)]
    pub fn run(&self, registers: &mut Registers) -> BlockOutput {
        let func: BlockFn = unsafe { std::mem::transmute(self.code.as_ptr()) };

        let mut output = BlockOutput::default();
        func(registers, &mut output);

        output
    }

    /// Returns the sequence of instructions this block represents.
    pub fn sequence(&self) -> &Sequence {
        &self.seq
    }

    /// Returns the Cranelift IR generated for this block.
    pub fn clir(&self) -> &str {
        &self.clir
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut decoder =
            iced_x86::Decoder::new(usize::BITS, &self.code, iced_x86::DecoderOptions::NONE);

        let mut formatter = iced_x86::NasmFormatter::new();
        formatter.options_mut().set_digit_separator("_");
        formatter.options_mut().set_first_operand_char_index(0);
        formatter
            .options_mut()
            .set_space_after_operand_separator(true);
        formatter
            .options_mut()
            .set_space_between_memory_add_operators(true);
        formatter.options_mut().set_scale_before_index(true);
        formatter.options_mut().set_decimal_digit_group_size(3);
        formatter.options_mut().set_hex_prefix("0x");
        formatter.options_mut().set_binary_prefix("0b");
        formatter.options_mut().set_uppercase_prefixes(true);

        let mut output = String::new();
        let mut instruction = iced_x86::Instruction::default();
        while decoder.can_decode() {
            decoder.decode_out(&mut instruction);

            output.clear();
            formatter.format(&instruction, &mut output);

            write!(f, "{:05X} ", instruction.ip())?;
            writeln!(f, " {}", output)?;
        }

        Ok(())
    }
}
