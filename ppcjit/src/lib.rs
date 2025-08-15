#![feature(debug_closure_helpers)]

mod builder;
mod sequence;

pub mod registers;

use crate::builder::BlockBuilder;
use cranelift::{
    codegen::{self, ir},
    frontend, native,
    prelude::Configurable,
};
use easyerr::{Error, ResultExt};
use iced_x86::Formatter;
use memmap2::{Mmap, MmapOptions};
use std::{fmt::Display, sync::Arc};

pub use registers::Registers;
pub use sequence::Sequence;

pub type BlockFn = extern "sysv64" fn(&mut Registers);

pub struct Block {
    clir: String,
    map: Mmap,
}

impl Block {
    /// # Safety
    /// `code` must be the bytes of a valid host function with the [`BlockFn`] signature.
    unsafe fn new(clir: String, code: &[u8]) -> Self {
        let mut map = MmapOptions::new().len(code.len()).map_anon().unwrap();
        map.copy_from_slice(code);

        Self {
            clir,
            map: map.make_exec().unwrap(),
        }
    }

    #[inline(always)]
    pub fn run(&self, registers: &mut Registers) {
        let func: BlockFn = unsafe { std::mem::transmute(self.map.as_ptr()) };
        func(registers);
    }

    pub fn clir(&self) -> &str {
        &self.clir
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut decoder =
            iced_x86::Decoder::new(usize::BITS, &self.map, iced_x86::DecoderOptions::NONE);

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

pub struct JIT {
    isa: Arc<dyn codegen::isa::TargetIsa>,
    func_ctx: frontend::FunctionBuilderContext,
}

impl Default for JIT {
    fn default() -> Self {
        let mut builder = codegen::settings::builder();
        builder.set("use_colocated_libcalls", "false").unwrap();
        builder.set("is_pic", "false").unwrap();
        builder.set("stack_switch_model", "basic").unwrap();
        builder.set("opt_level", "speed_and_size").unwrap();
        builder.enable("enable_alias_analysis").unwrap();
        builder.enable("enable_jump_tables").unwrap();

        let isa_builder = native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });

        let isa = isa_builder
            .finish(codegen::settings::Flags::new(builder))
            .unwrap();

        Self {
            isa,
            func_ctx: frontend::FunctionBuilderContext::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error(transparent)]
    Builder { source: builder::EmitError },
    #[error(transparent)]
    Codegen { source: codegen::CodegenError },
}

impl JIT {
    fn block_signature(&self) -> ir::Signature {
        let ptr = self.isa.pointer_type();
        ir::Signature {
            params: vec![ir::AbiParam::new(ptr)],
            returns: vec![],
            call_conv: codegen::isa::CallConv::SystemV,
        }
    }

    pub fn build(&mut self, sequence: Sequence) -> Result<Block, BuildError> {
        let mut func = ir::Function::new();
        func.signature = self.block_signature();

        let mut builder = BlockBuilder::new(&mut func, &mut self.func_ctx);
        for ins in sequence.iter().copied() {
            builder.emit(ins).context(BuildCtx::Builder)?;
        }
        builder.finish();

        let mut ctx = codegen::Context::for_function(func);
        let ir = ctx.func.display().to_string();

        let compiled = ctx
            .compile(&*self.isa, &mut codegen::control::ControlPlane::default())
            .map_err(|e| e.inner)
            .context(BuildCtx::Codegen)?;

        Ok(unsafe { Block::new(ir, compiled.code_buffer()) })
    }
}

#[cfg(test)]
mod test {
    use crate::{JIT, Registers, Sequence};
    use powerpc::Ins;
    use powerpc_asm::{Argument, Arguments, assemble};

    #[test]
    fn test() {
        let mut seq = Sequence::new();
        let args: Arguments = [
            Argument::Unsigned(0),
            Argument::Unsigned(0),
            Argument::Unsigned(1),
            Argument::None,
            Argument::None,
        ];

        let a = assemble("add.", &args).expect("Invalid arguments");
        seq.push(Ins::new(a, powerpc::Extensions::none())).unwrap();

        let mut registers = Registers::default();
        registers.user.gpr[0] = 1;
        registers.user.gpr[1] = i32::MAX as u32;

        let mut jit = JIT::default();
        let block = jit.build(seq).unwrap();
        println!("{:?}", &registers.user);
        block.run(&mut registers);
        println!("{:?}", &registers.user);
        println!("{block}");
    }
}
