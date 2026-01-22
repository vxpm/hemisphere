use gekko::Address;

use crate::system::System;

#[derive(Debug)]
pub struct CallFrame {
    /// Address of this call.
    pub address: Address,
    /// Name of the symbol that was called.
    pub symbol: Option<String>,
    /// Name of the location of the symbol.
    pub location: Option<String>,
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

pub fn call_stack(sys: &System, top_frame: Address, top_routine: Address) -> CallStack {
    let mut call_stack = Vec::new();
    let mut current_frame = top_frame.value();
    let mut current_routine = top_routine.value();

    loop {
        if current_frame == 0 || current_routine == 0 {
            break;
        }

        let prev_frame_addr = Address(current_frame);
        let return_target_addr = Address(current_frame.wrapping_add(4));

        if let Some(prev_frame) = sys.read_pure(prev_frame_addr)
            && let Some(return_target) = sys.read_pure::<u32>(return_target_addr)
        {
            let called_at = return_target.wrapping_sub(4);
            let name = sys.modules.debug.find_symbol(Address(current_routine));
            let location = sys
                .modules
                .debug
                .find_location(Address(current_routine))
                .map(|l| l.to_string());

            call_stack.push(CallFrame {
                address: Address(called_at),
                symbol: name,
                location,
                stack: Address(current_frame),
                returns: Address(return_target),
            });

            current_frame = prev_frame;
            current_routine = called_at;
        } else {
            break;
        }
    }

    CallStack(call_stack)
}

pub fn current_call_stack(sys: &System) -> CallStack {
    self::call_stack(sys, Address(sys.cpu.user.gpr[1]), sys.cpu.pc)
}
