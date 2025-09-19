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
        let mut current_sp = self.cpu.user.gpr[1];
        let mut current_lr = self.cpu.pc.value();

        loop {
            let Some(prev_addr) = self.translate_data_addr(Address(current_sp)) else {
                break;
            };

            let Some(prev) = self.bus.read_pure(prev_addr) else {
                break;
            };

            let Some(link_addr) = self.translate_data_addr(Address(current_sp.wrapping_add(4)))
            else {
                break;
            };

            let Some(link) = self.bus.read_pure(link_addr) else {
                break;
            };

            let routine = self
                .config
                .executable
                .as_ref()
                .and_then(|e| e.find_symbol(Address(current_lr)))
                .map(|s| s.into_owned());

            call_stack.push(StackFrame {
                routine,
                address: Address(current_sp),
                previous: Address(prev),
                link: Address(link),
            });

            if prev == current_sp || prev == 0 {
                break;
            }

            current_sp = prev;
            current_lr = link;
        }

        CallStack(call_stack)
    }
}
