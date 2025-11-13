use crate::block::IdleLoop;
use gekko::{
    InsExt,
    disasm::{Ins, Opcode, ParsedIns},
};
use std::ops::Deref;

/// A sequence of PowerPC instructions.
#[derive(Clone, Default)]
pub struct Sequence(pub Vec<Ins>);

impl Sequence {
    pub fn detect_idle_loop(&self) -> IdleLoop {
        if self.len() == 1 && self[0].code == 0x4800_0000 {
            return IdleLoop::Simple;
        }

        if self.len() < 3 {
            return IdleLoop::None;
        }

        let is_load = matches!(
            self[0].op,
            Opcode::Lbz | Opcode::Lha | Opcode::Lhz | Opcode::Lwz
        );
        let is_cmp_imm = matches!(self[1].op, Opcode::Cmpi | Opcode::Cmpli);
        let is_branch_cond = matches!(self[2].op, Opcode::Bc);
        let load_dst_is_cmp_src = self[0].gpr_d() == self[1].gpr_a();
        let is_rel_jmp_to_start = !self[2].field_aa() && self[2].field_bd() == -8;
        let code_matches =
            is_load && is_cmp_imm && is_branch_cond && load_dst_is_cmp_src && is_rel_jmp_to_start;

        if code_matches {
            IdleLoop::VolatileValue
        } else {
            IdleLoop::None
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
