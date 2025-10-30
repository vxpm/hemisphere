use crate::{Acc40, Dsp, Ins, Reg, ins::CondCode};
use bitos::BitUtils;

#[inline(always)]
fn add_carried(lhs: i64, new: i64) -> bool {
    lhs as u64 > new as u64
}

#[inline(always)]
fn sub_carried(lhs: i64, new: i64) -> bool {
    lhs as u64 >= new as u64
}

#[inline(always)]
fn add_overflowed(lhs: i64, rhs: i64, new: i64) -> bool {
    (lhs > 0 && rhs > 0 && new <= 0) || (lhs < 0 && rhs < 0 && new >= 0)
}

#[inline(always)]
fn round_40(value: i64) -> i64 {
    if value.bit(16) {
        (value + 0x8000) & !0xFFFF
    } else {
        (value + 0x7FFF) & !0xFFFF
    }
}

impl Dsp {
    fn base_flags(&mut self, value: i64) {
        self.regs.status.set_sign(value < 0);
        self.regs.status.set_arithmetic_zero(value == 0);
        self.regs
            .status
            .set_above_s32(value > i32::MAX as i64 || value < i32::MIN as i64);
        self.regs
            .status
            .set_top_two_bits_eq(value.bit(30) == value.bit(31));
        self.regs
            .status
            .set_overflow_fused(self.regs.status.overflow() || self.regs.status.overflow_fused());
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

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    fn add_to_addr_reg(&mut self, addr: usize, value: u16, cond: bool) {
        let ar = self.regs.addressing[addr];
        let wrap = self.regs.wrapping[addr];

        // following algorithm was described by @calc84maniac, thanks!

        // compute amount of significant bits, minimum 1
        let n = (16 - wrap.leading_zeros()).max(1);

        // create a mask of n bits
        let mask = 1u16.checked_shl(n).map(|r| r - 1).unwrap_or(!0);

        // compute the carry out of bit n
        let carry = ((ar & mask) as u32 + (value & mask) as u32) > mask as u32;

        // compute result
        let mut result = ar.wrapping_add(value);
        if value as i16 > 0 || (cond && value as i16 == 0) {
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

    pub fn addarn(&mut self, ins: Ins) {
        let addr = ins.base.bits(0, 2) as usize;
        let idx = ins.base.bits(2, 4) as usize;

        let ix = self.regs.indexing[idx];
        self.add_to_addr_reg(addr, ix, true);
    }

    pub fn addax(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc32[s] as i64;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn addaxl(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc32[s].bits(0, 16) as u64 as i64;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn addi(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.extra as i16 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn addis(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.base.bits(0, 8) as i8 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn addp(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let (carry, overflow, rhs) = self.regs.product.get();
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new) || carry);
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, rhs, new) ^ overflow);

        self.base_flags(new);
    }

    pub fn addpaxz(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let (carry, overflow, lhs) = self.regs.product.get();
        let lhs = round_40(lhs);

        let rhs = self.regs.acc32[s] as i64;
        let new = self.regs.acc40[d].set((lhs + rhs) & !0xFFFF);

        self.regs.status.set_carry(add_carried(lhs, new) ^ carry);
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, rhs, new) ^ overflow);

        self.base_flags(new);
    }

    pub fn addr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bits(9, 11) as u8;

        let lhs = self.regs.acc40[d].get();
        let rhs = (self.regs.get(Reg::new(s + 0x18)) as i16 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

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

    pub fn asrnr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let signed_shift = self.regs.acc40[1 - d].mid;
        let rhs = signed_shift.bits(0, 6);

        let new = if signed_shift.bit(6) {
            let rhs = (64 - rhs) % 64;
            self.regs.acc40[d].set(lhs >> rhs)
        } else {
            self.regs.acc40[d].set(lhs << rhs)
        };

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn asrnrx(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let signed_shift = self.regs.acc32[s] >> 16;
        let rhs = signed_shift.bits(0, 6);

        let new = if signed_shift.bit(6) {
            let rhs = (64 - rhs) % 64;
            self.regs.acc40[d].set(lhs >> rhs)
        } else {
            self.regs.acc40[d].set(lhs << rhs)
        };

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn asr16(&mut self, ins: Ins) {
        let r = ins.base.bit(11) as usize;

        let old = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(old >> 16);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn clr15(&mut self, _: Ins) {
        self.regs.status.set_unsigned_mul(false);
    }

    pub fn clr(&mut self, ins: Ins) {
        let r = ins.base.bit(11) as usize;

        let new = self.regs.acc40[r].set(0);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn clrl(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;

        let old = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(round_40(old));

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn clrp(&mut self, _: Ins) {
        self.regs.product.low = 0x0000;
        self.regs.product.mid1 = 0xFFF0;
        self.regs.product.mid2 = 0x0010;
        self.regs.product.high = 0x00FF;
    }

    pub fn cmp(&mut self, _: Ins) {
        let lhs = self.regs.acc40[0].get();
        let rhs = self.regs.acc40[1].get();
        let diff = Acc40::from(lhs - rhs).get();

        self.regs.status.set_carry(sub_carried(lhs, diff));
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, -rhs, diff));

        self.base_flags(diff);
    }

    pub fn cmpaxh(&mut self, ins: Ins) {
        let s = ins.base.bit(11) as usize;
        let r = ins.base.bit(12) as usize;

        let lhs = self.regs.acc40[s].get();
        let rhs = ((self.regs.acc32[r] as i64) >> 16) << 16;
        let diff = Acc40::from(lhs - rhs).get();

        self.regs.status.set_carry(sub_carried(lhs, diff));
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, -rhs, diff));

        self.base_flags(diff);
    }

    pub fn cmpi(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.extra as i16 as i64) << 16;
        let diff = Acc40::from(lhs - rhs).get();

        self.regs.status.set_carry(sub_carried(lhs, diff));
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, -rhs, diff));

        self.base_flags(diff);
    }

    pub fn cmpis(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = (ins.base as i8 as i64) << 16;
        let diff = Acc40::from(lhs - rhs).get();

        self.regs.status.set_carry(sub_carried(lhs, diff));
        self.regs
            .status
            .set_overflow(add_overflowed(lhs, -rhs, diff));

        self.base_flags(diff);
    }

    pub fn dar(&mut self, ins: Ins) {
        let addr = ins.base.bits(0, 2) as usize;
        self.add_to_addr_reg(addr, -1i16 as u16, false);
    }

    pub fn dec(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let old = self.regs.acc40[d].get();
        let new = self.regs.acc40[d].set(old.wrapping_sub(1));

        self.regs.status.set_carry(sub_carried(old, new));
        self.regs.status.set_overflow(add_overflowed(old, -1, new));

        self.base_flags(new);
    }

    pub fn decm(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let old = self.regs.acc40[d].get();
        let new = self.regs.acc40[d].set(old - (1 << 16));

        self.regs.status.set_carry(sub_carried(old, new));
        self.regs
            .status
            .set_overflow(add_overflowed(old, -(1 << 16), new));

        self.base_flags(new);
    }

    pub fn halt(&mut self, _: Ins) {
        self.control.halt = true;
    }

    pub fn iar(&mut self, ins: Ins) {
        let addr = ins.base.bits(0, 2) as usize;
        self.add_to_addr_reg(addr, 1i16 as u16, false);
    }

    fn condition(&self, code: CondCode) -> bool {
        let status = self.regs.status.clone();
        match code {
            CondCode::GreaterOrEqual => status.overflow() == status.sign(),
            CondCode::Less => status.overflow() != status.sign(),
            CondCode::Greater => status.overflow() == status.sign() && !status.arithmetic_zero(),
            CondCode::LessOrEqual => status.overflow() != status.sign() || status.arithmetic_zero(),
            CondCode::NotZero => !status.arithmetic_zero(),
            CondCode::Zero => status.arithmetic_zero(),
            CondCode::NotCarry => !status.carry(),
            CondCode::Carry => status.carry(),
            CondCode::BelowS32 => !status.above_s32(),
            CondCode::AboveS32 => status.above_s32(),
            CondCode::WeirdA => {
                (status.above_s32() || status.top_two_bits_eq()) && !status.arithmetic_zero()
            }
            CondCode::WeirdB => {
                (!status.above_s32() && !status.top_two_bits_eq()) || status.arithmetic_zero()
            }
            CondCode::NotLogicZero => !status.logic_zero(),
            CondCode::LogicZero => status.logic_zero(),
            CondCode::Overflow => status.overflow(),
            CondCode::Always => true,
        }
    }

    pub fn ifcc(&mut self, ins: Ins) {
        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        if !self.condition(code) {
            self.regs.pc += 1;
        }
    }

    pub fn inc(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let old = self.regs.acc40[d].get();
        let new = self.regs.acc40[d].set(old.wrapping_add(1));

        self.regs.status.set_carry(add_carried(old, new));
        self.regs.status.set_overflow(add_overflowed(old, 1, new));

        self.base_flags(new);
    }

    pub fn incm(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let old = self.regs.acc40[d].get();
        let new = self.regs.acc40[d].set(old + (1 << 16));

        self.regs.status.set_carry(add_carried(old, new));
        self.regs
            .status
            .set_overflow(add_overflowed(old, 1 << 16, new));

        self.base_flags(new);
    }

    pub fn lsl(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let shift = ins.base.bits(0, 6);

        let old = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(old << shift);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn lsl16(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;

        let old = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(old << 16);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn lsr(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let shift = ins.base.bits(0, 6);

        let lhs = (self.regs.acc40[r].get() as u64) & ((1 << 40) - 1);
        let rhs = (64 - shift) % 64;
        let new = self.regs.acc40[r].set((lhs >> rhs) as i64);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn lsrn(&mut self, _: Ins) {
        let lhs = (self.regs.acc40[0].get()) & ((1 << 40) - 1);
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

    pub fn lsrnr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = (self.regs.acc40[d].get()) & ((1 << 40) - 1);
        let signed_shift = self.regs.acc40[1 - d].mid;
        let rhs = signed_shift.bits(0, 6);

        let new = if signed_shift.bit(6) {
            let rhs = (64 - rhs) % 64;
            self.regs.acc40[d].set(lhs >> rhs)
        } else {
            self.regs.acc40[d].set(lhs << rhs)
        };

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn lsrnrx(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = (self.regs.acc40[d].get()) & ((1 << 40) - 1);
        let signed_shift = self.regs.acc32[s] >> 16;
        let rhs = signed_shift.bits(0, 6);

        let new = if signed_shift.bit(6) {
            let rhs = (64 - rhs) % 64;
            self.regs.acc40[d].set(lhs >> rhs)
        } else {
            self.regs.acc40[d].set(lhs << rhs)
        };

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn lsr16(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;

        let old = (self.regs.acc40[r].get() as u64) & ((1 << 40) - 1);
        let new = self.regs.acc40[r].set((old >> 16) as i64);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn m0(&mut self, _: Ins) {
        self.regs.status.set_dont_double_result(true);
    }

    pub fn m2(&mut self, _: Ins) {
        self.regs.status.set_dont_double_result(false);
    }

    // NOTE: carry flag issue
    pub fn madd(&mut self, ins: Ins) {
        let s = ins.base.bit(8) as usize;

        let acc = self.regs.acc32[s];
        let low = (acc << 16) >> 16;
        let high = acc >> 16;
        let mul = low * high;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod + result as i64);
    }

    // NOTE: carry flag issue
    pub fn maddc(&mut self, ins: Ins) {
        let t = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod + result as i64);
    }

    // NOTE: carry flag issue
    pub fn maddx(&mut self, ins: Ins) {
        let t = ins.base.bit(8) as u8;
        let s = ins.base.bit(9) as u8;

        let lhs = self.regs.get(Reg::new(0x18 + 2 * s)) as i16 as i32;
        let rhs = self.regs.get(Reg::new(0x19 + 2 * t)) as i16 as i32;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod + result as i64);
    }

    pub fn mov(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let new = self.regs.acc40[d].set(self.regs.acc40[1 - d].get());

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn movax(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let new = self.regs.acc40[d].set(self.regs.acc32[s] as i64);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn movnp(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let (carry, overflow, prod) = self.regs.product.get();
        let new = self.regs.acc40[d].set(-prod);

        self.regs.status.set_carry((prod != 0) && !carry);
        self.regs.status.set_overflow(overflow);

        self.base_flags(new);
    }

    pub fn movp(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let (carry, overflow, prod) = self.regs.product.get();
        let new = self.regs.acc40[d].set(prod);

        self.regs.status.set_carry(carry);
        self.regs.status.set_overflow(overflow);

        self.base_flags(new);
    }

    // TODO: carry flag
    pub fn movpz(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let (carry, overflow, prod) = self.regs.product.get();
        let new = self.regs.acc40[d].set(round_40(prod));

        self.regs.status.set_carry(carry);
        self.regs.status.set_overflow(overflow);

        self.base_flags(new);
    }

    pub fn movr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bits(9, 11) as u8;

        let lhs = self.regs.get(Reg::new(0x18 + s)) as i16 as i64;
        let new = self.regs.acc40[d].set(lhs << 16);

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(new);
    }

    pub fn mrr(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 5) as u8;
        let d = ins.base.bits(5, 10) as u8;

        let acc_src = |i: usize| {
            let ml = self.regs.acc40[i].get() as i32 as i64;
            let hml = self.regs.acc40[i].get();

            if self.regs.status.sign_extend_to_40() && ml != hml {
                if hml >= 0 { 0x7FFF } else { 0x8000 }
            } else {
                self.regs.acc40[i].mid
            }
        };

        let src = Reg::new(s);
        let src = match src {
            Reg::Acc40Mid0 => acc_src(0),
            Reg::Acc40Mid1 => acc_src(1),
            _ => self.regs.get(src),
        };

        let mut acc_dst = |i: usize| {
            if !self.regs.status.sign_extend_to_40() {
                self.regs.acc40[i].mid = src;
                return;
            }

            self.regs.acc40[i].low = 0;
            self.regs.acc40[i].mid = src;
            self.regs.acc40[i].high = if src.bit(15) { !0 } else { 0 };
        };

        let dst = Reg::new(d);
        match dst {
            Reg::Acc40Mid0 => acc_dst(0),
            Reg::Acc40Mid1 => acc_dst(1),
            _ => self.regs.set(dst, src),
        }
    }

    // NOTE: carry flag issue
    pub fn msub(&mut self, ins: Ins) {
        let s = ins.base.bit(8) as usize;

        let acc = self.regs.acc32[s];
        let low = (acc << 16) >> 16;
        let high = acc >> 16;
        let mul = low * high;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod - result as i64);
    }

    // NOTE: carry flag issue
    pub fn msubc(&mut self, ins: Ins) {
        let t = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod - result as i64);
    }

    // NOTE: carry flag issue
    pub fn msubx(&mut self, ins: Ins) {
        let t = ins.base.bit(8) as u8;
        let s = ins.base.bit(9) as u8;

        let lhs = self.regs.get(Reg::new(0x18 + 2 * s)) as i16 as i32;
        let rhs = self.regs.get(Reg::new(0x19 + 2 * t)) as i16 as i32;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        let (_, _, prod) = self.regs.product.get();
        self.regs.product.set(prod - result as i64);
    }

    // NOTE: carry flag issue
    pub fn mul(&mut self, ins: Ins) {
        let s = ins.base.bit(11) as usize;

        let acc = self.regs.acc32[s];
        let low = (acc << 16) >> 16;
        let high = acc >> 16;
        let mul = low * high;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
    }

    // NOTE: carry flag issue
    pub fn mulac(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let s = ins.base.bit(11) as usize;

        let acc_r = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(acc_r + self.regs.product.get().2);

        let acc_s = self.regs.acc32[s];
        let low = (acc_s << 16) >> 16;
        let high = acc_s >> 16;
        let mul = low * high;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);

        self.regs.status.set_overflow(false);
        self.base_flags(new);
    }

    // NOTE: carry flag issue
    pub fn mulaxh(&mut self, _: Ins) {
        let val = (self.regs.acc32[0] >> 16) as i64;
        let mul = val * val;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
    }
}
