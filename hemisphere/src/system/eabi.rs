use crate::system::System;
use gekko::Address;

#[derive(Debug)]
pub struct CallFrame {
    /// Address of this call.
    pub address: Address,
    /// Symbol name of the call.
    pub symbol: Option<String>,
    /// Address of the stack frame of this call.
    pub stack: Address,
    /// Return address.
    pub returns: Address,
}

#[derive(Default)]
pub struct CallStack(pub Vec<CallFrame>);

impl std::fmt::Display for CallStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for frame in &self.0 {
            write!(
                f,
                "{}: {}",
                frame.address,
                frame.symbol.as_deref().unwrap_or("<unknown>"),
            )?;

            if frame.returns != 0 {
                writeln!(f, " (return to {})", frame.returns)?;
            } else {
                writeln!(f, " (link unsaved)")?;
            }
        }

        Ok(())
    }
}

impl System {
    pub fn call_stack(&self) -> CallStack {
        let mut call_stack = Vec::new();
        let mut current_frame = self.cpu.user.gpr[1];
        let mut current_routine = self.cpu.pc.value();

        loop {
            if current_frame == 0 || current_routine == 0 {
                break;
            }

            let prev_frame_addr = Address(current_frame);
            let prev_routine_addr = Address(current_frame.wrapping_add(4));

            if let Some(prev_frame_addr) = self.translate_data_addr(prev_frame_addr)
                && let Some(prev_routine_addr) = self.translate_data_addr(prev_routine_addr)
                && let Some(prev_frame) = self.read_pure(prev_frame_addr)
                && let Some(prev_routine) = self.read_pure(prev_routine_addr)
            {
                let name = self
                    .config
                    .debug_info
                    .as_ref()
                    .and_then(|d| d.find_symbol(Address(current_routine)));

                call_stack.push(CallFrame {
                    address: Address(current_routine),
                    symbol: name,
                    stack: Address(current_frame),
                    returns: Address(prev_routine),
                });

                current_frame = prev_frame;
                current_routine = prev_routine;
            } else {
                break;
            }
        }

        CallStack(call_stack)
    }
}
