mod primitive;

pub mod arch;

pub use primitive::Primitive;

/// A memory address. This is a thin wrapper around a [`u32`].
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Address(pub u32);

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "0x{:04X}_{:04X}",
            (self.0 & 0xFFFF_0000) >> 16,
            self.0 & 0xFFFF
        )
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Address {
    /// Returns the value of this address. Equivalent to `self.0`.
    #[inline(always)]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Returns `true` if this address is aligned to the given alignment.
    #[inline(always)]
    pub const fn is_aligned(self, alignment: u32) -> bool {
        self.0.is_multiple_of(alignment)
    }
}

impl std::ops::Add<u32> for Address {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0.wrapping_add(rhs))
    }
}

impl std::ops::Add<i32> for Address {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self(self.0.wrapping_add_signed(rhs))
    }
}

impl std::ops::AddAssign<u32> for Address {
    fn add_assign(&mut self, rhs: u32) {
        *self = *self + rhs;
    }
}

impl std::ops::AddAssign<i32> for Address {
    fn add_assign(&mut self, rhs: i32) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub<u32> for Address {
    type Output = Self;

    fn sub(self, rhs: u32) -> Self::Output {
        Self(self.0.wrapping_sub(rhs))
    }
}

impl std::ops::Sub<i32> for Address {
    type Output = Self;

    fn sub(self, rhs: i32) -> Self::Output {
        Self(self.0.wrapping_add_signed(-rhs))
    }
}

impl std::ops::SubAssign<u32> for Address {
    fn sub_assign(&mut self, rhs: u32) {
        *self = *self - rhs;
    }
}

impl std::ops::SubAssign<i32> for Address {
    fn sub_assign(&mut self, rhs: i32) {
        *self = *self - rhs;
    }
}

impl PartialEq<u32> for Address {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl From<u32> for Address {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
