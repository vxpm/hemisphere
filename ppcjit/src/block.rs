use crate::{Registers, Sequence};
use hemicore::Address;
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};
use std::fmt::Display;

type ExternalData = std::ffi::c_void;
type ReadFunction<T> = fn(*mut ExternalData, *const Registers, Address) -> T;
type WriteFunction<T> = fn(*mut ExternalData, *const Registers, Address, T);

/// External functions that JITed code calls.
pub struct ExternalFunctions {
    pub read_i8: ReadFunction<i8>,
    pub write_i8: WriteFunction<i8>,
    pub read_i16: ReadFunction<i16>,
    pub write_i16: WriteFunction<i16>,
    pub read_i32: ReadFunction<i32>,
    pub write_i32: WriteFunction<i32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Jump {
    /// Should a jump be executed?
    pub execute: bool,
    /// Whether the jump is relative or not.
    pub relative: bool,
    /// Whether the CPU should link before the jump.
    pub link: bool,
    /// Data associated with the jump. Can be an address or an offset.
    pub data: i32,
}

#[derive(Default)]
pub struct BlockOutput {
    /// How many instructions were executed.
    pub executed: u32,
    /// Information regarding jumps.
    pub jump: Jump,
}

pub type BlockFn = extern "sysv64" fn(
    *mut Registers,
    *mut ExternalData,
    *const ExternalFunctions,
    *mut BlockOutput,
);

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
    pub fn run(
        &self,
        registers: &mut Registers,
        external_data: *mut ExternalData,
        external_functions: &ExternalFunctions,
    ) -> BlockOutput {
        let func: BlockFn = unsafe { std::mem::transmute(self.code.as_ptr()) };

        let mut output = BlockOutput::default();
        func(registers, external_data, external_functions, &mut output);

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

    /// Returns the length, in bytes, of this block.
    pub fn len(&self) -> usize {
        self.code.len()
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
        formatter.options_mut().set_hex_suffix("");
        formatter.options_mut().set_binary_prefix("0b");
        formatter.options_mut().set_binary_suffix("");
        formatter.options_mut().set_uppercase_prefixes(true);
        formatter
            .options_mut()
            .set_small_hex_numbers_in_decimal(false);

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
