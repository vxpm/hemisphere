mod unwind;

use crate::{Sequence, block::unwind::UnwindHandle};
use cranelift::{
    codegen::{CompiledCode, ir},
    prelude::isa,
};
use gekko::{Address, Cpu};

pub type Context = std::ffi::c_void;
pub type GetRegistersHook = fn(*mut Context) -> *mut Cpu;
pub type ReadHook<T> = fn(*mut Context, Address, *mut T) -> bool;
pub type WriteHook<T> = fn(*mut Context, Address, T) -> bool;
pub type ReadQuantizedHook = fn(*mut Context, Address, u8, *mut f64) -> u8;
pub type WriteQuantizedHook = fn(*mut Context, Address, u8, f64) -> u8;
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
    pub read_quantized: ReadQuantizedHook,
    pub write_quantized: WriteQuantizedHook,
    pub cache_dma: GenericHook,

    // msr
    pub msr_changed: GenericHook,

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

    /// Returns the function signature for a quantized memory read hook.
    pub(crate) fn read_quantized_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address
                ir::AbiParam::new(ir::types::I8),  // gqr
                ir::AbiParam::new(ptr_type),       // value ptr
            ],
            returns: vec![ir::AbiParam::new(ir::types::I8)], // size
            call_conv: isa::CallConv::SystemV,
        }
    }

    /// Returns the function signature for a quantized memory read hook.
    pub(crate) fn write_quantized_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address
                ir::AbiParam::new(ir::types::I8),  // gqr
                ir::AbiParam::new(ir::types::F64), // value
            ],
            returns: vec![ir::AbiParam::new(ir::types::I8)], // size
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

/// Information regarding a block's execution.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Executed {
    /// How many instructions were executed.
    pub instructions: u32,
    /// How many cycles were executed.
    pub cycles: u32,
}

pub type BlockFn = extern "sysv64-unwind" fn(*mut Context, *const Hooks) -> Executed;

#[derive(Clone, Copy)]
pub enum IdleLoop {
    /// Not an idle loop
    None,
    /// Branching to self
    Simple,
    /// Reading from a fixed memory location on a loop
    VolatileValue,
}

/// Meta information regarding a block.
pub struct Meta {
    /// The sequence of instructions this block contains.
    pub seq: Sequence,
    /// The Cranelift IR of this block. Only available if `cfg!(debug_assertions)` is true.
    pub clir: Option<String>,
    /// How many cycles this block executes at most.
    pub cycles: u32,
    /// Whether this block is an idle loop and if so, what kind.
    pub idle_loop: IdleLoop,
}

/// A compiled block of PowerPC instructions.
pub struct Block {
    meta: Meta,
    code: *const u8,
    _unwind: Option<UnwindHandle>,
}

impl Block {
    pub(crate) fn new(
        ptr: *const u8,
        code: &CompiledCode,
        meta: Meta,
        isa: &dyn isa::TargetIsa,
    ) -> Self {
        let _unwind = if let Ok(Some(unwind_info)) = code.create_unwind_info(isa) {
            unsafe { UnwindHandle::new(isa, ptr.addr(), &unwind_info) }
        } else {
            None
        };

        Self {
            meta,
            code: ptr,
            _unwind,
        }
    }

    /// Meta information regarding this block.
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Executes this block of instructions and returns how many cycles were executed.
    #[inline(always)]
    pub fn call(&self, ctx: *mut Context, hooks: *const Hooks) -> Executed {
        let func: BlockFn = unsafe { std::mem::transmute(self.code.addr()) };
        func(ctx, hooks)
    }
}
