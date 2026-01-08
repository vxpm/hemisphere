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
                frame.stack,
                frame.symbol.as_deref().unwrap_or("<unknown>"),
            )?;

            if !frame.returns.is_null() {
                writeln!(f, " (return to {})", frame.returns)?;
            } else {
                writeln!(f, " (link unsaved)")?;
            }
        }

        Ok(())
    }
}

pub fn call_stack(sys: &System, last_frame: Address, last_routine: Address) -> CallStack {
    let mut call_stack = Vec::new();
    let mut current_frame = last_frame.value();
    let mut current_routine = last_routine.value();

    loop {
        if current_frame == 0 || current_routine == 0 {
            break;
        }

        let prev_frame_addr = Address(current_frame);
        let prev_routine_addr = Address(current_frame.wrapping_add(4));

        if let Some(prev_frame) = sys.read_pure(prev_frame_addr)
            && let Some(prev_routine) = sys.read_pure(prev_routine_addr)
        {
            let name = sys.modules.debug.find_symbol(Address(current_routine));
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

pub fn current_call_stack(sys: &System) -> CallStack {
    self::call_stack(sys, Address(sys.cpu.user.gpr[1]), sys.cpu.pc)
}
