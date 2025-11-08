use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Trait for memory primitives.
///
/// A primitive is either a byte, half-word, word or double word.
/// That is, [`u8`], [`i8`], [`u16`], [`i16`], [`u32`], [`i32`], [`u64`] or [`i64`].
pub trait Primitive:
    std::fmt::Debug
    + std::fmt::UpperHex
    + Copy
    + Immutable
    + FromBytes
    + IntoBytes
    + Default
    + Send
    + Sync
    + 'static
{
    /// Reads a value of this primitive from the bytes of a buffer (in native endian). If `buf`
    /// does not contain enough data, it's going to be completed with zeros.
    fn read_ne_bytes(buf: &[u8]) -> Self;

    /// Writes this primitive to the given buffer (in native endian). If `buf` is not big enough,
    /// remaining bytes are going to be silently dropped.
    fn write_ne_bytes(self, buf: &mut [u8]);

    /// Reads a value of this primitive from the bytes of a buffer (in little endian). If `buf`
    /// does not contain enough data, it's going to be completed with zeros.
    fn read_le_bytes(buf: &[u8]) -> Self;

    /// Writes this primitive to the given buffer (in little endian). If `buf` is not big enough,
    /// remaining bytes are going to be silently dropped.
    fn write_le_bytes(self, buf: &mut [u8]);

    /// Reads a value of this primitive from the bytes of a buffer (in big endian). If `buf` does
    /// not contain enough data, it's going to be completed with zeros.
    fn read_be_bytes(buf: &[u8]) -> Self;

    /// Writes this primitive to the given buffer (in big endian). If `buf` is not big enough,
    /// remaining bytes are going to be silently dropped.
    fn write_be_bytes(self, buf: &mut [u8]);
}

macro_rules! impl_primitive {
    ($($type:ty),*) => {
        $(
            impl Primitive for $type {
                #[inline(always)]
                fn read_ne_bytes(buf: &[u8]) -> Self {
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

                        <$type>::from_ne_bytes(read_buf)
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { read_unhappy(buf) }
                    } else {
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        <$type>::from_ne_bytes(unsafe { buf.as_ptr().cast::<[u8; SELF_SIZE]>().read_unaligned() })
                    }
                }

                #[inline]
                fn write_ne_bytes(self, buf: &mut [u8]) {
                    const SELF_SIZE: usize = size_of::<$type>();

                    /// Unhappy path for when `buf` is too small.
                    ///
                    /// # Safety
                    /// `buf` must be <= `SELF_SIZE`
                    #[cold]
                    #[inline(never)]
                    unsafe fn write_unhappy(_self: $type, buf: &mut [u8]) {
                        let bytes = _self.to_ne_bytes();
                        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_mut_ptr(), buf.len()) };
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { write_unhappy(self, buf) };
                    } else {
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        unsafe { buf.as_mut_ptr().cast::<[u8; SELF_SIZE]>().write_unaligned(self.to_ne_bytes()) };
                    }
                }

                #[inline(always)]
                fn read_le_bytes(buf: &[u8]) -> Self {
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
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        <$type>::from_le_bytes(unsafe { buf.as_ptr().cast::<[u8; SELF_SIZE]>().read_unaligned() })
                    }
                }

                #[inline]
                fn write_le_bytes(self, buf: &mut [u8]) {
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
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        unsafe { buf.as_mut_ptr().cast::<[u8; SELF_SIZE]>().write_unaligned(self.to_le_bytes()) };
                    }
                }

                #[inline(always)]
                fn read_be_bytes(buf: &[u8]) -> Self {
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

                        <$type>::from_be_bytes(read_buf)
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { read_unhappy(buf) }
                    } else {
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        <$type>::from_be_bytes(unsafe { buf.as_ptr().cast::<[u8; SELF_SIZE]>().read_unaligned() })
                    }
                }

                #[inline]
                fn write_be_bytes(self, buf: &mut [u8]) {
                    const SELF_SIZE: usize = size_of::<$type>();

                    /// Unhappy path for when `buf` is too small.
                    ///
                    /// # Safety
                    /// `buf` must be <= `SELF_SIZE`
                    #[cold]
                    #[inline(never)]
                    unsafe fn write_unhappy(_self: $type, buf: &mut [u8]) {
                        let bytes = _self.to_be_bytes();
                        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_mut_ptr(), buf.len()) };
                    }

                    if buf.len() < SELF_SIZE {
                        unsafe { write_unhappy(self, buf) };
                    } else {
                        // SAFETY: all primitives are IntoBytes, FromBytes and Immutable
                        unsafe { buf.as_mut_ptr().cast::<[u8; SELF_SIZE]>().write_unaligned(self.to_be_bytes()) };
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
    u64,

    i8,
    i16,
    i32,
    i64
}
