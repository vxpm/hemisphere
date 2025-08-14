use std::ops::Deref;

use easyerr::Error;
use powerpc::Ins;

/// A sequence of PowerPC instructions which can be contained in a single block.
pub struct Sequence(Vec<Ins>);

fn is_terminal(ins: &Ins) -> bool {
    ins.is_branch()
}

#[derive(Debug, Clone, Copy, Error)]
#[error("failed to push instruction: sequence is complete")]
pub struct PushError;

impl Sequence {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, ins: Ins) -> Result<(), PushError> {
        if self.0.last().is_some_and(is_terminal) {
            Err(PushError)
        } else {
            self.0.push(ins);
            Ok(())
        }
    }
}

impl Deref for Sequence {
    type Target = [Ins];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
