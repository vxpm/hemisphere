use crate::Sequence;
use common::{Address, arch::Cpu};
use cranelift::{codegen::ir, prelude::isa};
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};
use std::fmt::Display;

pub type Context = std::ffi::c_void;
pub type GetRegistersHook = fn(*mut Context) -> *mut Cpu;
pub type ReadHook<T> = fn(*mut Context, Address) -> T;
pub type WriteHook<T> = fn(*mut Context, Address, T);
pub type GenericHook = fn(*mut Context);

/// External functions that JITed code calls.
pub struct Hooks {
    // registers
    pub get_registers: GetRegistersHook,

    // memory
    pub read_i8: ReadHook<i8>,
    pub write_i8: WriteHook<i8>,
    pub read_i16: ReadHook<i16>,
    pub write_i16: WriteHook<i16>,
    pub read_i32: ReadHook<i32>,
    pub write_i32: WriteHook<i32>,

    // bats
    pub ibat_changed: GenericHook,
    pub dbat_changed: GenericHook,
}

impl Hooks {
    /// Returns the function signature for the `get_registers` hook.
    pub(crate) fn get_registers_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type), // registers
            ],
            returns: vec![ir::AbiParam::new(ptr_type)],
            call_conv: isa::CallConv::SystemV,
        }
    }

    /// Returns the function signature for a memory read hook.
    pub(crate) fn read_sig(ptr_type: ir::Type, read_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address
            ],
            returns: vec![ir::AbiParam::new(read_type)], // value
            call_conv: isa::CallConv::SystemV,
        }
    }

    /// Returns the function signature for a memory write hook.
    pub(crate) fn write_sig(ptr_type: ir::Type, write_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address
                ir::AbiParam::new(write_type),     // value
            ],
            returns: vec![],
            call_conv: isa::CallConv::SystemV,
        }
    }

    /// Returns the function signature for a generic hook.
    pub(crate) fn generic_hook_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type), // ctx
            ],
            returns: vec![],
            call_conv: isa::CallConv::SystemV,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Executed {
    pub instructions: u32,
    pub cycles: u32,
}

pub type BlockFn = extern "sysv64" fn(*mut Context, *const Hooks) -> Executed;

/// A compiled block of PowerPC instructions.
pub struct Block {
    seq: Sequence,
    clir: String,
    cycles: u32,
    code: Mmap,
}

impl Block {
    /// # Safety
    /// `code` must be the bytes of a valid host function with the [`BlockFn`] signature.
    pub(crate) unsafe fn new(seq: Sequence, clir: String, cycles: u32, code: &[u8]) -> Self {
        let mut map = MmapOptions::new().len(code.len()).map_anon().unwrap();
        map.copy_from_slice(code);

        Self {
            seq,
            clir,
            cycles,
            code: map.make_exec().unwrap(),
        }
    }

    /// Executes this block of instructions and returns how many cycles were executed.
    #[inline(always)]
    pub fn run(&self, ctx: *mut Context, hooks: &Hooks) -> Executed {
        let func: BlockFn = unsafe { std::mem::transmute(self.code.as_ptr()) };
        func(ctx, hooks)
    }

    /// Returns the sequence of instructions this block represents.
    pub fn sequence(&self) -> &Sequence {
        &self.seq
    }

    /// Returns the Cranelift IR generated for this block.
    pub fn clir(&self) -> &str {
        &self.clir
    }

    /// Returns how many cycles this block executes _at most_ (i.e. without any early exits).
    pub fn cycles(&self) -> u32 {
        self.cycles
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
