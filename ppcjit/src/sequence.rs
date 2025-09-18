use common::arch::disasm::{Ins, ParsedIns};
use std::ops::Deref;

/// A sequence of PowerPC instructions.
#[derive(Clone, Default)]
pub struct Sequence(pub Vec<Ins>);

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
