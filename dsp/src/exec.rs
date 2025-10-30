use bitos::BitUtils;

use crate::{Acc40, Dsp, Ins, Reg};

impl Dsp {
    fn base_flags(&mut self, value: i64) {
        self.regs.status.set_sign(value < 0);
        self.regs.status.set_arithmetic_zero(value == 0);
        self.regs
            .status
            .set_above_s32(value > i32::MAX as i64 || value < i32::MIN as i64);
        self.regs
            .status
            .set_overflow_fused(self.regs.status.overflow() || self.regs.status.overflow_fused());
        self.regs
            .status
            .set_top_two_bits_eq(value.bit(30) == value.bit(31));
    }

    pub fn halt(&mut self, _: Ins) {
        self.control.halt = true;
    }

    pub fn abs(&mut self, ins: Ins) {
        let idx = ins.base.bit(11) as usize;
        let old = self.regs.acc40[idx].get();
        let new = self.regs.acc40[idx].set(old.abs());

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(new == Acc40::MIN);

        self.base_flags(new);
    }

    pub fn add(&mut self, ins: Ins) {
        let idx = ins.base.bit(8) as usize;
        let lhs = self.regs.acc40[idx].get();
        let rhs = self.regs.acc40[1 - idx].get();
        let new = self.regs.acc40[idx].set(lhs + rhs);

        self.regs.status.set_carry(lhs as u64 > new as u64);
        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn addarn(&mut self, ins: Ins) {
        let addr = ins.base.bits(0, 2) as usize;
        let idx = ins.base.bits(2, 4) as usize;

        let ar = self.regs.addressing[addr];
        let ix = self.regs.indexing[idx];
        let wrap = self.regs.wrapping[addr];

        // following algorithm was described by @calc84maniac, thanks!

        // compute amount of significant bits, minimum 1
        let n = (16 - wrap.leading_zeros()).max(1);

        // create a mask of n bits
        let mask = 1u16.checked_shl(n).map(|r| r - 1).unwrap_or(!0);

        // compute the carry out of bit n
        let carry = ((ar & mask) as u32 + (ix & mask) as u32) > mask as u32;

        // compute result
        let mut result = ar.wrapping_add(ix);
        if ix as i16 >= 0 {
            if carry {
                result = result.wrapping_sub(wrap.wrapping_add(1));
            }
        } else {
            let low_sum = result & mask;
            let not_low_wrap = (!wrap) & mask;
            let carry_again = low_sum < not_low_wrap;

            if !carry || carry_again {
                result = result.wrapping_add(wrap.wrapping_add(1));
            }
        }

        self.regs.addressing[addr] = result;
    }

    pub fn addax(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc32[s] as i64;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(lhs as u64 > new as u64);
        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn addaxl(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc32[s].bits(0, 16) as u64 as i64;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(lhs as u64 > new as u64);
        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn addi(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.extra as i16 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(lhs as u64 > new as u64);
        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn addis(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.base.bits(0, 8) as i8 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(lhs as u64 > new as u64);
        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn addp(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let (carry, overflow, rhs) = self.regs.product.get();
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs
            .status
            .set_carry(lhs as u64 > new as u64 || rhs as u64 > new as u64 || carry);

        self.regs.status.set_overflow(
            ((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0)) ^ overflow,
        );

        self.base_flags(new);
    }

    // TODO: carry flag is still wrong
    pub fn addpaxz(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let (carry, overflow, lhs) = self.regs.product.get_rounded();
        let rhs = self.regs.acc32[s] as i64;
        let new = self.regs.acc40[d].set((lhs + rhs) & !0xFFFF);

        self.regs
            .status
            .set_carry((lhs as u64 > new as u64 || rhs as u64 > new as u64) ^ carry);

        self.regs.status.set_overflow(
            ((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0)) ^ overflow,
        );

        self.base_flags(new);
    }

    pub fn addr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bits(9, 11) as u8;

        let lhs = self.regs.acc40[d].get();
        let rhs = (self.regs.get(Reg::new(s + 0x18)) as i16 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs
            .status
            .set_carry(lhs as u64 > new as u64 || rhs as u64 > new as u64);

        self.regs
            .status
            .set_overflow((lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0));

        self.base_flags(new);
    }

    pub fn andc(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid &= self.regs.acc40[1 - d].mid;
        let new = self.regs.acc40[d].get();

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);

        self.regs
            .status
            .set_arithmetic_zero(self.regs.acc40[d].mid == 0);
        self.regs
            .status
            .set_sign((self.regs.acc40[d].mid as i16) < 0);
    }

    pub fn andcf(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let is_equal = self.regs.acc40[d].mid & ins.extra == ins.extra;
        self.regs.status.set_logic_zero(is_equal);
    }

    pub fn andf(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let is_equal = self.regs.acc40[d].mid & ins.extra == 0;
        self.regs.status.set_logic_zero(is_equal);
    }

    pub fn andi(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid &= ins.extra;
        let new = self.regs.acc40[d].get();

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);

        self.regs
            .status
            .set_arithmetic_zero(self.regs.acc40[d].mid == 0);
        self.regs
            .status
            .set_sign((self.regs.acc40[d].mid as i16) < 0);
    }

    pub fn andr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        self.regs.acc40[d].mid &= (self.regs.acc32[s] >> 16) as u16;
        let new = self.regs.acc40[d].get();

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);

        self.regs
            .status
            .set_arithmetic_zero(self.regs.acc40[d].mid == 0);
        self.regs
            .status
            .set_sign((self.regs.acc40[d].mid as i16) < 0);
    }

    pub fn asl(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let imm = ins.base.bits(0, 6) as u8;

        let lhs = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(lhs << imm);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn asr(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let imm = ins.base.bits(0, 6);

        let lhs = self.regs.acc40[r].get();
        let rhs = (64 - imm) % 64;
        let new = self.regs.acc40[r].set(lhs >> rhs);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn asrn(&mut self, _: Ins) {
        let lhs = self.regs.acc40[0].get();

        let signed_shift = self.regs.acc40[1].mid;
        let rhs = signed_shift.bits(0, 6);

        let new = if signed_shift.bit(6) {
            let rhs = (64 - rhs) % 64;
            self.regs.acc40[0].set(lhs << rhs)
        } else {
            self.regs.acc40[0].set(lhs >> rhs)
        };

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }
}
