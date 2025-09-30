mod unwind;

use crate::{Sequence, block::unwind::UnwindHandle};
use common::{Address, arch::Cpu};
use cranelift::{
    codegen::{CompiledCode, ir},
    prelude::isa,
};
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};
use std::fmt::Display;

pub type Context = std::ffi::c_void;
pub type GetRegistersHook = fn(*mut Context) -> *mut Cpu;
pub type ReadHook<T> = fn(*mut Context, Address, *mut T) -> bool;
pub type WriteHook<T> = fn(*mut Context, Address, T) -> bool;
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
    pub read_i64: ReadHook<i64>,
    pub write_i64: WriteHook<i64>,

    // bats
    pub ibat_changed: GenericHook,
    pub dbat_changed: GenericHook,

    // time base
    pub tb_read: GenericHook,
    pub tb_changed: GenericHook,

    // decrementer
    pub dec_read: GenericHook,
    pub dec_changed: GenericHook,
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
    pub(crate) fn read_sig(ptr_type: ir::Type, _read_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address
                ir::AbiParam::new(ptr_type),       // value ptr
            ],
            returns: vec![ir::AbiParam::new(ir::types::I8)], // success
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
            returns: vec![ir::AbiParam::new(ir::types::I8)], // success
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

pub type BlockFn = extern "sysv64-unwind" fn(*mut Context, *const Hooks) -> Executed;

pub struct Meta {
    pub seq: Sequence,
    pub clir: Option<String>,
    pub cycles: u32,
}

/// A compiled block of PowerPC instructions.
pub struct Block {
    meta: Meta,
    code: Mmap,
    _unwind: Option<UnwindHandle>,
}

impl Block {
    pub(crate) unsafe fn new(meta: Meta, isa: &dyn isa::TargetIsa, code: &CompiledCode) -> Self {
        let mut map = MmapOptions::new()
            .len(code.code_buffer().len())
            .map_anon()
            .unwrap();
        map.copy_from_slice(code.code_buffer());

        let _unwind = if let Ok(Some(unwind_info)) = code.create_unwind_info(isa) {
            UnwindHandle::new(isa, map.as_ptr() as usize, &unwind_info)
        } else {
            None
        };

        Self {
            meta,
            code: map.make_exec().unwrap(),
            _unwind,
        }
    }

    /// Returns the bytes of the host code.
    pub fn bytes(&self) -> &[u8] {
        &self.code
    }

    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Executes this block of instructions and returns how many cycles were executed.
    #[inline(always)]
    pub fn call(&self, ctx: *mut Context, hooks: &Hooks) -> Executed {
        let func: BlockFn = unsafe { std::mem::transmute(self.code.as_ptr()) };
        func(ctx, hooks)
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
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

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "disasm unsupported");
    }
}
