use crate::Sequence;
use hemicore::{Address, arch::Registers};
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};
use std::{fmt::Display, sync::Arc};

pub type ExternalData = std::ffi::c_void;
pub type GetRegistersFn = fn(*mut ExternalData) -> *mut Registers;
pub type ReadFn<T> = fn(*mut ExternalData, Address) -> T;
pub type WriteFn<T> = fn(*mut ExternalData, Address, T);
pub type GenericHookFn = fn(*mut ExternalData);

/// External functions that JITed code calls.
pub struct ExternalFunctions {
    // registers
    pub get_registers: GetRegistersFn,

    // memory
    pub read_i8: ReadFn<i8>,
    pub write_i8: WriteFn<i8>,
    pub read_i16: ReadFn<i16>,
    pub write_i16: WriteFn<i16>,
    pub read_i32: ReadFn<i32>,
    pub write_i32: WriteFn<i32>,

    // hooks
    pub ibat_changed: GenericHookFn,
    pub dbat_changed: GenericHookFn,
}

pub type BlockFn = extern "sysv64" fn(*mut ExternalData, *const ExternalFunctions) -> u32;

struct Inner {
    seq: Sequence,
    clir: String,
    code: Mmap,
}

/// A compiled block of PowerPC instructions.
///
/// Blocks are reference counted internally, so this is cheaply clonable.
#[derive(Clone)]
pub struct Block(Arc<Inner>);

impl Block {
    /// # Safety
    /// `code` must be the bytes of a valid host function with the [`BlockFn`] signature.
    pub(crate) unsafe fn new(seq: Sequence, clir: String, code: &[u8]) -> Self {
        let mut map = MmapOptions::new().len(code.len()).map_anon().unwrap();
        map.copy_from_slice(code);

        Self(Arc::new(Inner {
            seq,
            clir,
            code: map.make_exec().unwrap(),
        }))
    }

    /// Executes this block of instructions.
    #[inline(always)]
    pub fn run(
        &self,
        external_data: *mut ExternalData,
        external_functions: &ExternalFunctions,
    ) -> u32 {
        let func: BlockFn = unsafe { std::mem::transmute(self.0.code.as_ptr()) };
        func(external_data, external_functions)
    }

    /// Returns the sequence of instructions this block represents.
    pub fn sequence(&self) -> &Sequence {
        &self.0.seq
    }

    /// Returns the Cranelift IR generated for this block.
    pub fn clir(&self) -> &str {
        &self.0.clir
    }

    /// Returns the length, in bytes, of this block.
    pub fn len(&self) -> usize {
        self.0.code.len()
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut decoder =
            iced_x86::Decoder::new(usize::BITS, &self.0.code, iced_x86::DecoderOptions::NONE);

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
