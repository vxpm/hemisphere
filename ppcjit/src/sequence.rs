use common::arch::disasm::{Ins, Opcode, ParsedIns};
use easyerr::Error;
use std::ops::Deref;

/// A sequence of PowerPC instructions which can be contained in a single JIT [`Block`](super::Block).
pub struct Sequence(Vec<Ins>);

fn is_jump(ins: &Ins) -> bool {
    ins.is_unconditional_branch() || matches!(ins.op, Opcode::Rfi)
}

fn is_sync(ins: &Ins) -> bool {
    matches!(ins.op, Opcode::Isync | Opcode::Sync | Opcode::Tlbsync)
}

fn is_exception(ins: &Ins) -> bool {
    matches!(ins.op, Opcode::Sc)
}

fn is_terminal(ins: &Ins) -> bool {
    is_jump(ins) || is_sync(ins) || is_exception(ins)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SequenceStatus {
    Open,
    Terminated,
}

#[derive(Debug, Clone, Copy, Error)]
#[error("failed to push instruction: sequence is complete")]
pub struct PushError;

impl Default for Sequence {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequence {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, ins: Ins) -> Result<SequenceStatus, PushError> {
        if self.0.last().is_some_and(is_terminal) {
            Err(PushError)
        } else {
            self.0.push(ins);
            Ok(if is_terminal(&ins) {
                SequenceStatus::Terminated
            } else {
                SequenceStatus::Open
            })
        }
    }
}

impl Deref for Sequence {
    type Target = [Ins];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parsed = ParsedIns::new();
        for ins in &self.0 {
            ins.parse_basic(&mut parsed);
            writeln!(f, "{parsed}")?;
        }

        Ok(())
    }
}
