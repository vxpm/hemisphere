use crate::system::System;
use common::Address;

#[derive(Debug)]
pub struct StackFrame {
    /// Name of the routine that owns this frame.
    pub routine: Option<String>,
    /// Address of this stack frame.
    pub address: Address,
    /// Address of the previous stack frame.
    pub previous: Address,
    /// Return address.
    pub link: Address,
}

pub struct CallStack(pub Vec<StackFrame>);

impl std::fmt::Display for CallStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for frame in &self.0 {
            write!(
                f,
                "{}: {}",
                frame.address,
                frame.routine.as_deref().unwrap_or("<unknown>"),
            )?;

            if frame.link != 0 {
                writeln!(f, " (return to {})", frame.link)?;
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
                && let Some(prev_frame) = self.bus.read_pure(prev_frame_addr)
                && let Some(prev_routine) = self.bus.read_pure(prev_routine_addr)
            {
                let routine = self
                    .config
                    .executable
                    .as_ref()
                    .and_then(|e| e.find_symbol(Address(current_routine)))
                    .map(|s| s.into_owned());

                call_stack.push(StackFrame {
                    routine,
                    address: Address(current_frame),
                    previous: Address(prev_frame),
                    link: Address(prev_routine),
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
