use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Trait for memory primitives.
///
/// A primitive is either a byte, half-word or word.
/// That is, [`u8`], [`i8`], [`u16`], [`i16`], [`u32`] or [`i32`].
pub trait Primitive:
    std::fmt::Debug
    + std::fmt::UpperHex
    + Copy
    + Immutable
    + FromBytes
    + IntoBytes
    + Send
    + Sync
    + 'static
{
    /// The alignment of this primitive.
    const ALIGNMENT: u32;

    /// Reads a value of this primitive from a buffer. If `buf` does not contain enough data, it's
    /// going to be completed with zeros.
    fn read_from_buf(buf: &[u8]) -> Self;

    /// Writes this primitive to the given buffer. If `buf` is not big enough, remaining bytes are
    /// going to be silently dropped.
    fn write_to_buf(self, buf: &mut [u8]);
}

macro_rules! impl_primitive {
    ($($type:ty),*) => {
        $(
            impl Primitive for $type {
                const ALIGNMENT: u32 = align_of::<Self>() as u32;

                #[inline(always)]
                fn read_from_buf(buf: &[u8]) -> Self {
                    const SELF_SIZE: usize = size_of::<$type>();

                    /// Unhappy path for when `buf` is too small.
                    ///
                    /// # Safety
                    /// `buf` must be <= `SELF_SIZE`
                    #[cold]
                    #[inline(never)]
                    unsafe fn read_unhappy(buf: &[u8]) -> $type {
                        let mut read_buf = [0u8; SELF_SIZE];
                        unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), read_buf.as_mut_ptr(), buf.len()) };
                        <$type>::from_le_bytes(read_buf)
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { read_unhappy(buf) }
                    } else {
                        // TODO: seal the trait to enforce this
                        // SAFETY: this is safe because all primitives are integers, which are POD
                        unsafe { buf.as_ptr().cast::<Self>().read_unaligned() }
                    }
                }

                #[inline]
                fn write_to_buf(self, buf: &mut [u8]) {
                    const SELF_SIZE: usize = size_of::<$type>();

                    /// Unhappy path for when `buf` is too small.
                    ///
                    /// # Safety
                    /// `buf` must be <= `SELF_SIZE`
                    #[cold]
                    #[inline(never)]
                    unsafe fn write_unhappy(_self: $type, buf: &mut [u8]) {
                        let bytes = _self.to_le_bytes();
                        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_mut_ptr(), buf.len()) };
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { write_unhappy(self, buf) };
                    } else {
                        // TODO: seal the trait to enforce this
                        // SAFETY: this is safe because all primitives are integers, which are POD
                        unsafe { buf.as_mut_ptr().cast::<$type>().write_unaligned(self) };
                    }
                }
            }
        )*
    };
}

impl_primitive! {
    u8,
    u16,
    u32,
    i8,
    i16,
    i32
}
