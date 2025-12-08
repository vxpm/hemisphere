use crate::Sequence;
use cranelift::{codegen::ir, prelude::isa};
use gekko::{Address, Cpu};

#[derive(Debug)]
#[repr(C)]
pub struct LinkData {
    /// Linked block
    pub link: BlockFn,
    /// Information regarding the pattern of the linked block
    pub pattern: Pattern,
}

pub type Context = std::ffi::c_void;

pub type GetRegistersHook = fn(*mut Context) -> *mut Cpu;
pub type FollowLinkHook = fn(*const Info, *mut Context, *mut LinkData) -> bool;
pub type TryLinkHook = fn(*mut Context, Address, *mut LinkData);

pub type ReadHook<T> = fn(*mut Context, Address, *mut T) -> bool;
pub type WriteHook<T> = fn(*mut Context, Address, T) -> bool;
pub type ReadQuantizedHook = fn(*mut Context, Address, u8, *mut f64) -> u8;
pub type WriteQuantizedHook = fn(*mut Context, Address, u8, f64) -> u8;

pub type GenericHook = fn(*mut Context);

/// Information about block execution.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Info {
    /// How many instructions have been executed already. Updated on block exits only.
    pub instructions: u32,
    /// How many cycles have been executed already. Updated on block exits only.
    pub cycles: u32,
}

/// External functions that JITed code calls.
pub struct Hooks {
    /// Hook that returns a pointer to the CPU state struct given the context.
    pub get_registers: GetRegistersHook,
    /// Hook that checks whether a linked block should be followed or the execution should return.
    pub follow_link: FollowLinkHook,
    /// Tries to link this block to another one given the current context, the destination address
    /// and a pointer to where the linked block function pointer should be stored.
    pub try_link: TryLinkHook,

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

    /// Returns the function signature for the `follow_link` hook.
    pub(crate) fn follow_link_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type), // info
                ir::AbiParam::new(ptr_type), // ctx
                ir::AbiParam::new(ptr_type), // lnk data
            ],
            returns: vec![ir::AbiParam::new(ir::types::I8)], // follow?
            call_conv: isa::CallConv::SystemV,
        }
    }

    /// Returns the function signature for the `try_link` hook.
    pub(crate) fn try_link_sig(ptr_type: ir::Type) -> ir::Signature {
        ir::Signature {
            params: vec![
                ir::AbiParam::new(ptr_type),       // ctx
                ir::AbiParam::new(ir::types::I32), // address to link to
                ir::AbiParam::new(ptr_type),       // link ptr storage
            ],
            returns: vec![],
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

/// A block pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pattern {
    /// No known pattern.
    None = 0,
    /// A single instruction long block with a call.
    Call,
    /// Branching to self
    IdleBasic,
    /// Idling by reading from a fixed memory location on a loop
    IdleVolatileRead,
    /// Function which the status of the CPU->DSP mailbox and returns it.
    GetMailboxStatusFunc,
}

/// Meta information regarding a block.
#[derive(Clone)]
pub struct Meta {
    /// The sequence of instructions this block contains.
    pub seq: Sequence,
    /// The Cranelift IR of this block. Only available if `cfg!(debug_assertions)` is true.
    pub clir: Option<String>,
    /// How many cycles this block executes at most.
    pub cycles: u32,
    /// The pattern of this block.
    pub pattern: Pattern,
}

/// A handle representing a compiled block of PowerPC instructions. This struct does not manage the
/// memory behind the block.
///
/// In order to call a block, you need a trampoline.
#[derive(Clone)]
pub struct Block {
    code: *const u8,
    meta: Meta,
}

pub type BlockFn = *const std::ffi::c_void;

impl Block {
    pub(crate) fn new(code: *const u8, meta: Meta) -> Self {
        // let _unwind = if let Ok(Some(unwind_info)) = code.create_unwind_info(isa) {
        //     unsafe { UnwindHandle::new(isa, ptr.addr(), &unwind_info) }
        // } else {
        //     None
        // };

        Self { code, meta }
    }

    /// Meta information regarding this block.
    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Returns a pointer to the function of this block.
    pub fn as_ptr(&self) -> BlockFn {
        self.code.cast()
    }
}

pub struct Trampoline(pub(super) *const u8);

type TrampolineFn = extern "sysv64-unwind" fn(*mut Info, *mut Context, *const Hooks, BlockFn);

impl Trampoline {
    /// Calls the given block using this trampoline.
    pub unsafe fn call(&self, ctx: *mut Context, hooks: *const Hooks, block: BlockFn) -> Info {
        let mut info = Info {
            instructions: 0,
            cycles: 0,
        };

        let trampoline: TrampolineFn = unsafe { std::mem::transmute(self.0) };
        trampoline(&raw mut info, ctx, hooks, block);

        info
    }
}
