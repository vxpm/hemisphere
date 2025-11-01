use crate::{Acc40, Dsp, Ins, Reg, Registers, Status, ins::CondCode};
use bitos::BitUtils;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MultiplyMode {
    Unsigned,
    Mixed,
    Signed,
}

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
fn sub_overflowed(lhs: i64, rhs: i64, new: i64) -> bool {
    add_overflowed(lhs, -rhs, new)
}

#[inline(always)]
fn round_40(value: i64) -> i64 {
    if value.bit(16) {
        (value + 0x8000) & !0xFFFF
    } else {
        (value + 0x7FFF) & !0xFFFF
    }
}

fn add_to_addr_reg(ar: u16, wr: u16, value: i16) -> u16 {
    // following algorithm was created by @hrydgard, version implemented here was refined and
    // described by @calc84maniac, and @zaydlang helped me understand - thanks!!

    // compute amount of significant bits in wr, minimum 1
    let n = (16 - wr.leading_zeros()).max(1);

    // create a mask of n bits
    let mask = 1u16.checked_shl(n).map(|r| r - 1).unwrap_or(!0);

    // compute the carry out of bit n
    let carry = ((ar & mask) as u32 + (value as u16 & mask) as u32) > mask as u32;

    // compute result
    let mut result = ar.wrapping_add_signed(value);

    if value >= 0 {
        if carry {
            result = result.wrapping_sub(wr.wrapping_add(1));
        }
    } else {
        let low_sum = result & mask;
        let low_not_wrap = (!wr) & mask;
        let carry_again = low_sum < low_not_wrap;

        if !carry || carry_again {
            result = result.wrapping_add(wr.wrapping_add(1));
        }
    }

    result
}

fn sub_from_addr_reg(ar: u16, wr: u16, value: i16) -> u16 {
    // following algorithm was created by @hrydgard, version implemented here was refined and
    // described by @calc84maniac, and @zaydlang helped me understand - thanks!!

    // subtraction uses the one's complement
    let value = !value;

    // compute amount of significant bits in wr, minimum 1
    let n = (16 - wr.leading_zeros()).max(1);

    // create a mask of n bits
    let mask = 1u16.checked_shl(n).map(|r| r - 1).unwrap_or(!0);

    // compute the carry out of bit n
    let carry = ((ar & mask) as u32 + (value as u16 & mask) as u32 + 1) > mask as u32;

    // compute result
    let mut result = ar.wrapping_add_signed(value).wrapping_add(1);
    if (value.wrapping_add(1)) > 0 {
        if carry {
            result = result.wrapping_sub(wr.wrapping_add(1));
        }
    } else {
        let low_sum = result & mask;
        let low_not_wrap = (!wr) & mask;
        let carry_again = low_sum < low_not_wrap;

        if !carry || carry_again {
            result = result.wrapping_add(wr.wrapping_add(1));
        }
    }

    result
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
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc40[1 - d].get();
        let new = self.regs.acc40[d].set(lhs + rhs);

        self.regs.status.set_carry(add_carried(lhs, new));
        self.regs.status.set_overflow(add_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn addarn(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 2) as usize;
        let s = ins.base.bits(2, 4) as usize;

        let ar = self.regs.addressing[d];
        let wr = self.regs.wrapping[d];
        let ix = self.regs.indexing[s];

        self.regs.addressing[d] = add_to_addr_reg(ar, wr, ix as i16);
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
            .set_overflow(sub_overflowed(lhs, rhs, diff));

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
            .set_overflow(sub_overflowed(lhs, rhs, diff));

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
            .set_overflow(sub_overflowed(lhs, rhs, diff));

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
            .set_overflow(sub_overflowed(lhs, rhs, diff));

        self.base_flags(diff);
    }

    pub fn dar(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 2) as usize;

        let ar = self.regs.addressing[d];
        let wr = self.regs.wrapping[d];

        self.regs.addressing[d] = sub_from_addr_reg(ar, wr, 1i16);
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
        let r = ins.base.bits(0, 2) as usize;

        let ar = self.regs.addressing[r];
        let wr = self.regs.wrapping[r];

        self.regs.addressing[r] = add_to_addr_reg(ar, wr, 1);
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

        let src = self.regs.get(Reg::new(s));
        self.regs.set_saturate(Reg::new(d), src);
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

    // NOTE: carry flag issue
    pub fn mulc(&mut self, ins: Ins) {
        let t = ins.base.bit(11) as usize;
        let s = ins.base.bit(12) as usize;

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
    }

    // NOTE: carry flag issue
    pub fn mulcac(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11) as usize;
        let s = ins.base.bit(12) as usize;

        let (_, _, prod) = self.regs.product.get();

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
        let acc_r = self.regs.acc40[r].get();
        let new = self.regs.acc40[r].set(acc_r + prod);

        self.regs.status.set_overflow(false);
        self.base_flags(new);
    }

    // NOTE: carry flag issue
    pub fn mulcmv(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11) as usize;
        let s = ins.base.bit(12) as usize;

        let (_, _, prod) = self.regs.product.get();

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
        let new = self.regs.acc40[r].set(prod);

        self.regs.status.set_overflow(false);
        self.base_flags(new);
    }

    // NOTE: carry flag issue
    pub fn mulcmvz(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11) as usize;
        let s = ins.base.bit(12) as usize;

        let (_, _, prod) = self.regs.product.get();

        let lhs = self.regs.acc40[s].mid as i16 as i32;
        let rhs = self.regs.acc32[t] >> 16;
        let mul = lhs * rhs;
        let result = if self.regs.status.dont_double_result() {
            mul
        } else {
            2 * mul
        };

        self.regs.product.set(result as i64);
        let new = self.regs.acc40[r].set(round_40(prod));

        self.regs.status.set_overflow(false);
        self.base_flags(new);
    }

    // NOTE: carry flag issue
    pub fn mulmv(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let s = ins.base.bit(11) as usize;

        let (_, _, prod) = self.regs.product.get();
        let new = self.regs.acc40[r].set(prod);

        let low = (self.regs.acc32[s] << 16) >> 16;
        let high = self.regs.acc32[s] >> 16;
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
    pub fn mulmvz(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let s = ins.base.bit(11) as usize;

        let (_, _, prod) = self.regs.product.get();
        let new = self.regs.acc40[r].set(round_40(prod));

        let low = (self.regs.acc32[s] << 16) >> 16;
        let high = self.regs.acc32[s] >> 16;
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

    fn multiply(&self, mode: MultiplyMode, a: u16, b: u16) -> i64 {
        let factor = if self.regs.status.dont_double_result() {
            1
        } else {
            2
        };

        let (a, b) = if mode == MultiplyMode::Signed || !self.regs.status.unsigned_mul() {
            // sign ext, sign ext
            (a as i16 as i64, b as i16 as i64)
        } else if mode == MultiplyMode::Mixed {
            // zero ext, sign ext
            (a as u64 as i64, b as i16 as i64)
        } else {
            // zero ext, zero ext
            (a as u64 as i64, b as u64 as i64)
        };

        a * b * factor
    }

    pub fn mulx(&mut self, ins: Ins) {
        let t = ins.base.bit(11);
        let s = ins.base.bit(12);

        let lhs = if s {
            (self.regs.acc32[0] >> 16) as u16
        } else {
            self.regs.acc32[0] as u16
        };

        let rhs = if t {
            (self.regs.acc32[1] >> 16) as u16
        } else {
            self.regs.acc32[1] as u16
        };

        let (mode, lhs, rhs) = match (s, t) {
            (false, false) => (MultiplyMode::Unsigned, lhs, rhs),
            (false, true) => (MultiplyMode::Mixed, lhs, rhs),
            (true, false) => (MultiplyMode::Mixed, rhs, lhs),
            (true, true) => (MultiplyMode::Signed, lhs, rhs),
        };

        let result = self.multiply(mode, lhs, rhs);
        self.regs.product.set(result);
    }

    pub fn mulxac(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11);
        let s = ins.base.bit(12);

        let (_, _, prod) = self.regs.product.get();
        let acc = self.regs.acc40[r].get();
        self.regs.acc40[r].set(acc + prod);

        let lhs = if s {
            (self.regs.acc32[0] >> 16) as u16
        } else {
            self.regs.acc32[0] as u16
        };

        let rhs = if t {
            (self.regs.acc32[1] >> 16) as u16
        } else {
            self.regs.acc32[1] as u16
        };

        let (mode, lhs, rhs) = match (s, t) {
            (false, false) => (MultiplyMode::Unsigned, lhs, rhs),
            (false, true) => (MultiplyMode::Mixed, lhs, rhs),
            (true, false) => (MultiplyMode::Mixed, rhs, lhs),
            (true, true) => (MultiplyMode::Signed, lhs, rhs),
        };

        let result = self.multiply(mode, lhs, rhs);
        self.regs.product.set(result);
    }

    pub fn mulxmv(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11);
        let s = ins.base.bit(12);

        let (_, _, prod) = self.regs.product.get();
        self.regs.acc40[r].set(prod);

        let lhs = if s {
            (self.regs.acc32[0] >> 16) as u16
        } else {
            self.regs.acc32[0] as u16
        };

        let rhs = if t {
            (self.regs.acc32[1] >> 16) as u16
        } else {
            self.regs.acc32[1] as u16
        };

        let (mode, lhs, rhs) = match (s, t) {
            (false, false) => (MultiplyMode::Unsigned, lhs, rhs),
            (false, true) => (MultiplyMode::Mixed, lhs, rhs),
            (true, false) => (MultiplyMode::Mixed, rhs, lhs),
            (true, true) => (MultiplyMode::Signed, lhs, rhs),
        };

        let result = self.multiply(mode, lhs, rhs);
        self.regs.product.set(result);
    }

    pub fn mulxmvz(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;
        let t = ins.base.bit(11);
        let s = ins.base.bit(12);

        let (_, _, prod) = self.regs.product.get();
        self.regs.acc40[r].set(round_40(prod));

        let lhs = if s {
            (self.regs.acc32[0] >> 16) as u16
        } else {
            self.regs.acc32[0] as u16
        };

        let rhs = if t {
            (self.regs.acc32[1] >> 16) as u16
        } else {
            self.regs.acc32[1] as u16
        };

        let (mode, lhs, rhs) = match (s, t) {
            (false, false) => (MultiplyMode::Unsigned, lhs, rhs),
            (false, true) => (MultiplyMode::Mixed, lhs, rhs),
            (true, false) => (MultiplyMode::Mixed, rhs, lhs),
            (true, true) => (MultiplyMode::Signed, lhs, rhs),
        };

        let result = self.multiply(mode, lhs, rhs);
        self.regs.product.set(result);
    }

    pub fn neg(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let old = self.regs.acc40[d].get();
        let new = self.regs.acc40[d].set(-old);

        self.regs.status.set_carry(old == 0);
        self.regs.status.set_overflow(old == (1 << 40));

        self.base_flags(new);
    }

    pub fn not(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid ^= 0xFFFF;
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

    pub fn orc(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid |= self.regs.acc40[1 - d].mid;
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

    pub fn ori(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid |= ins.extra;
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

    pub fn orr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        self.regs.acc40[d].mid |= (self.regs.acc32[s] >> 16) as u16;
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

    pub fn sbclr(&mut self, ins: Ins) {
        let i = ins.base.bits(0, 3) as u8;

        let idx = 6 + i;
        let old = self.regs.status.to_bits();
        let new = if idx == 13 {
            old
        } else {
            old.with_bit(idx, false)
        };

        self.regs.status = Status::from_bits(new);
    }

    pub fn sbset(&mut self, ins: Ins) {
        let i = ins.base.bits(0, 3) as u8;

        let idx = 6 + i;
        let old = self.regs.status.to_bits();
        let new = if idx == 13 || idx == 8 {
            old
        } else {
            old.with_bit(idx, true)
        };

        self.regs.status = Status::from_bits(new);
    }

    pub fn set15(&mut self, _: Ins) {
        self.regs.status.set_unsigned_mul(true);
    }

    pub fn set16(&mut self, _: Ins) {
        self.regs.status.set_sign_extend_to_40(false);
    }

    pub fn set40(&mut self, _: Ins) {
        self.regs.status.set_sign_extend_to_40(true);
    }

    pub fn sub(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc40[1 - d].get();
        let new = self.regs.acc40[d].set(lhs - rhs);

        self.regs.status.set_carry(sub_carried(lhs, new));
        self.regs.status.set_overflow(sub_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn subarn(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 2) as usize;

        let ix = self.regs.indexing[d];
        let ar = self.regs.addressing[d];
        let wr = self.regs.wrapping[d];

        self.regs.addressing[d] = sub_from_addr_reg(ar, wr, ix as i16);
    }

    pub fn subax(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        let lhs = self.regs.acc40[d].get();
        let rhs = self.regs.acc32[s] as i64;
        let new = self.regs.acc40[d].set(lhs - rhs);

        self.regs.status.set_carry(sub_carried(lhs, new));
        self.regs.status.set_overflow(sub_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn subp(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        let lhs = self.regs.acc40[d].get();
        let (carry, overflow, rhs) = self.regs.product.get();
        let new = self.regs.acc40[d].set(lhs - rhs);

        self.regs.status.set_carry(sub_carried(lhs, new) ^ !carry);
        self.regs
            .status
            .set_overflow(sub_overflowed(lhs, rhs, new) ^ overflow);

        self.base_flags(new);
    }

    pub fn subr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bits(9, 11) as u8;

        let lhs = self.regs.acc40[d].get();
        let rhs = (self.regs.get(Reg::new(s + 0x18)) as i16 as i64) << 16;
        let new = self.regs.acc40[d].set(lhs - rhs);

        self.regs.status.set_carry(sub_carried(lhs, new));
        self.regs.status.set_overflow(sub_overflowed(lhs, rhs, new));

        self.base_flags(new);
    }

    pub fn tst(&mut self, ins: Ins) {
        let r = ins.base.bit(11) as usize;

        let acc = self.regs.acc40[r].get();

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(acc);
    }

    pub fn tstaxh(&mut self, ins: Ins) {
        let r = ins.base.bit(8) as usize;

        let acc = self.regs.acc32[r] >> 16;

        self.regs.status.set_carry(false);
        self.regs.status.set_overflow(false);

        self.base_flags(acc as i64);

        self.regs
            .status
            .set_top_two_bits_eq(acc.bit(15) == acc.bit(14));
    }

    pub fn tstprod(&mut self, _: Ins) {
        let (carry, overflow, prod) = self.regs.product.get();

        self.regs.status.set_carry(carry);
        self.regs.status.set_overflow(overflow);

        self.base_flags(prod);
    }

    pub fn xorc(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid ^= self.regs.acc40[1 - d].mid;
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

    pub fn xori(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;

        self.regs.acc40[d].mid ^= ins.extra;
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

    pub fn xorr(&mut self, ins: Ins) {
        let d = ins.base.bit(8) as usize;
        let s = ins.base.bit(9) as usize;

        self.regs.acc40[d].mid ^= (self.regs.acc32[s] >> 16) as u16;
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

    pub fn bloop(&mut self, ins: Ins) {
        let r = ins.base.bits(0, 5) as u8;

        let counter = self.regs.get(Reg::new(r));
        self.regs.call_stack.push(self.regs.pc.wrapping_add(2));
        self.regs.loop_stack.push(ins.extra + 1);
        self.regs.loop_count.push(counter);
    }

    pub fn bloopi(&mut self, ins: Ins) {
        let counter = ins.base.bits(0, 8);
        self.regs.call_stack.push(self.regs.pc.wrapping_add(2));
        self.regs.loop_stack.push(ins.extra + 1);
        self.regs.loop_count.push(counter);
    }

    pub fn call(&mut self, ins: Ins) {
        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        if self.condition(code) {
            self.regs.call_stack.push(self.regs.pc.wrapping_add(2));
            self.regs.pc = ins.extra - 2;
        }
    }

    pub fn callr(&mut self, ins: Ins) {
        let r = ins.base.bits(5, 8) as u8;

        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        let addr = self.regs.get(Reg::new(r));

        if self.condition(code) {
            self.regs.call_stack.push(self.regs.pc.wrapping_add(1));
            self.regs.pc = addr - 1;
        }
    }

    pub fn jmp(&mut self, ins: Ins) {
        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        if self.condition(code) {
            self.regs.pc = ins.extra - 2;
        }
    }

    pub fn jmpr(&mut self, ins: Ins) {
        let r = ins.base.bits(5, 8) as u8;

        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        let addr = self.regs.get(Reg::new(r));

        if self.condition(code) {
            self.regs.pc = addr - 1;
        }
    }

    pub fn ret(&mut self, ins: Ins) {
        let code = CondCode::new(ins.base.bits(0, 4) as u8);
        if self.condition(code) {
            let addr = self.regs.call_stack.pop().unwrap();
            self.regs.pc = addr - 1;
        }
    }

    pub fn lr(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        let data = self.read_data(ins.extra);
        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn lri(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        self.regs.set_saturate(Reg::new(d), ins.extra);
    }

    pub fn lris(&mut self, ins: Ins) {
        let d = ins.base.bits(8, 11) as u8;
        let imm = ins.base.bits(0, 8) as i8 as i16;
        self.regs.set_saturate(Reg::new(0x18 + d), imm as u16);
    }

    pub fn lrr(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        let s = ins.base.bits(5, 7) as usize;

        let addr = self.regs.addressing[s];
        let data = self.read_data(addr);
        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn lrrd(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        let s = ins.base.bits(5, 7) as usize;

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        let data = self.read_data(ar);
        self.regs.addressing[s] = sub_from_addr_reg(ar, wr, 1);

        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn lrri(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        let s = ins.base.bits(5, 7) as usize;

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        let data = self.read_data(ar);
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, 1);

        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn lrrn(&mut self, ins: Ins) {
        let d = ins.base.bits(0, 5) as u8;
        let s = ins.base.bits(5, 7) as usize;

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        let ix = self.regs.indexing[s];
        let data = self.read_data(ar);
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, ix as i16);

        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn lrs(&mut self, ins: Ins) {
        let imm = ins.base.bits(0, 8) as u8;
        let d = ins.base.bits(8, 10) as u8;

        let addr = u16::from_le_bytes([imm, self.regs.config]);
        let data = self.read_data(addr);
        self.regs.set_saturate(Reg::new(d), data);
    }

    pub fn ilrr(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bit(8);

        let reg = if d { Reg::Acc40Mid1 } else { Reg::Acc40Mid0 };
        let addr = self.regs.addressing[s];
        let data = self.read_instr(addr);

        self.regs.set_saturate(reg, data);
    }

    pub fn ilrrd(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bit(8);

        let reg = if d { Reg::Acc40Mid1 } else { Reg::Acc40Mid0 };
        let ar = self.regs.addressing[s];
        let data = self.read_instr(ar);
        self.regs.set_saturate(reg, data);

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        self.regs.addressing[s] = sub_from_addr_reg(ar, wr, 1);
    }

    pub fn ilrri(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bit(8);

        let reg = if d { Reg::Acc40Mid1 } else { Reg::Acc40Mid0 };
        let ar = self.regs.addressing[s];
        let data = self.read_instr(ar);
        self.regs.set_saturate(reg, data);

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, 1);
    }

    pub fn ilrrn(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bit(8);

        let reg = if d { Reg::Acc40Mid1 } else { Reg::Acc40Mid0 };
        let ar = self.regs.addressing[s];
        let data = self.read_instr(ar);
        self.regs.set_saturate(reg, data);

        let ar = self.regs.addressing[s];
        let wr = self.regs.wrapping[s];
        let ix = self.regs.indexing[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, ix as i16);
    }

    pub fn si(&mut self, ins: Ins) {
        let offset = ins.base.bits(0, 8) as u8;
        let addr = u16::from_le_bytes([offset, 0xFF]);
        self.write_data(addr, ins.extra);
    }

    pub fn sr(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 5) as u8;
        let data = self.regs.get(Reg::new(s));
        self.write_data(ins.extra, data);
    }

    pub fn srr(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 5) as u8;
        let d = ins.base.bits(5, 7) as usize;

        let data = self.regs.get(Reg::new(s));
        let addr = self.regs.addressing[d];
        self.write_data(addr, data);
    }

    pub fn srrd(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 5) as u8;
        let d = ins.base.bits(5, 7) as usize;

        let data = self.regs.get(Reg::new(s));

        let ar = self.regs.addressing[d];
        let wr = self.regs.wrapping[d];
        self.regs.addressing[d] = sub_from_addr_reg(ar, wr, 1);

        let ar = self.regs.addressing[d];
        self.write_data(ar, data);
    }

    pub fn srri(&mut self, ins: Ins) {
        let s = ins.base.bits(0, 5) as u8;
        let d = ins.base.bits(5, 7) as usize;

        let data = self.regs.get(Reg::new(s));

        let ar = self.regs.addressing[d];
        let wr = self.regs.wrapping[d];
        self.regs.addressing[d] = add_to_addr_reg(ar, wr, 1);

        self.write_data(ar, data);
    }
}

impl Dsp {
    pub fn ext_dr(&mut self, ins: Ins, regs: &Registers) {
        let r = ins.base.bits(0, 2) as usize;

        let ar = regs.addressing[r];
        let wr = regs.wrapping[r];

        self.regs.addressing[r] = sub_from_addr_reg(ar, wr, 1i16);
    }

    pub fn ext_ir(&mut self, ins: Ins, regs: &Registers) {
        let r = ins.base.bits(0, 2) as usize;

        let ar = regs.addressing[r];
        let wr = regs.wrapping[r];

        self.regs.addressing[r] = add_to_addr_reg(ar, wr, 1i16);
    }

    pub fn ext_nr(&mut self, ins: Ins, regs: &Registers) {
        let r = ins.base.bits(0, 2) as usize;

        let ar = regs.addressing[r];
        let wr = regs.wrapping[r];
        let ir = regs.indexing[r];

        self.regs.addressing[r] = add_to_addr_reg(ar, wr, ir as i16);
    }

    pub fn ext_mv(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as u8;
        let d = ins.base.bits(2, 4) as u8;

        self.regs
            .set(Reg::new(0x18 + d), regs.get(Reg::new(0x1C + s)));
    }

    pub fn ext_l(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bits(3, 6) as u8;

        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(Reg::new(0x18 + d), data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, 1);
    }

    pub fn ext_ln(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let d = ins.base.bits(3, 6) as u8;

        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(Reg::new(0x18 + d), data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        let ix = regs.indexing[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, ix as i16);
    }

    pub fn ext_ld(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let r = ins.base.bit(4);
        let d = ins.base.bit(5);
        println!("ext_ld: {s}, {r}, {d}");

        let d = if d { Reg::Acc32High0 } else { Reg::Acc32Low0 };
        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(d, data);

        let r = if r { Reg::Acc32High1 } else { Reg::Acc32Low1 };
        let ar = regs.addressing[3];
        let data = self.read_data(ar);
        self.regs.set_saturate(r, data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, 1);

        let ar = regs.addressing[3];
        let wr = regs.wrapping[3];
        self.regs.addressing[3] = add_to_addr_reg(ar, wr, 1);
    }

    pub fn ext_ldm(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let r = ins.base.bit(4);
        let d = ins.base.bit(5);
        println!("ext_ldm: {s}, {r}, {d}");

        let d = if d { Reg::Acc32High0 } else { Reg::Acc32Low0 };
        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(d, data);

        let r = if r { Reg::Acc32High1 } else { Reg::Acc32Low1 };
        let ar = regs.addressing[3];
        let data = self.read_data(ar);
        self.regs.set_saturate(r, data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, 1);

        let ar = regs.addressing[3];
        let wr = regs.wrapping[3];
        let ix = regs.indexing[3];
        self.regs.addressing[3] = add_to_addr_reg(ar, wr, ix as i16);
    }

    pub fn ext_ldnm(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let r = ins.base.bit(4);
        let d = ins.base.bit(5);
        println!("ext_ldm: {s}, {r}, {d}");

        let d = if d { Reg::Acc32High0 } else { Reg::Acc32Low0 };
        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(d, data);

        let r = if r { Reg::Acc32High1 } else { Reg::Acc32Low1 };
        let ar = regs.addressing[3];
        let data = self.read_data(ar);
        self.regs.set_saturate(r, data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        let ix = regs.indexing[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, ix as i16);

        let ar = regs.addressing[3];
        let wr = regs.wrapping[3];
        let ix = regs.indexing[3];
        self.regs.addressing[3] = add_to_addr_reg(ar, wr, ix as i16);
    }

    pub fn ext_ldn(&mut self, ins: Ins, regs: &Registers) {
        let s = ins.base.bits(0, 2) as usize;
        let r = ins.base.bit(4);
        let d = ins.base.bit(5);
        println!("ext_ldm: {s}, {r}, {d}");

        let d = if d { Reg::Acc32High0 } else { Reg::Acc32Low0 };
        let ar = regs.addressing[s];
        let data = self.read_data(ar);
        self.regs.set_saturate(d, data);

        let r = if r { Reg::Acc32High1 } else { Reg::Acc32Low1 };
        let ar = regs.addressing[3];
        let data = self.read_data(ar);
        self.regs.set_saturate(r, data);

        let ar = regs.addressing[s];
        let wr = regs.wrapping[s];
        let ix = regs.indexing[s];
        self.regs.addressing[s] = add_to_addr_reg(ar, wr, ix as i16);

        let ar = regs.addressing[3];
        let wr = regs.wrapping[3];
        self.regs.addressing[3] = add_to_addr_reg(ar, wr, 1);
    }
}
