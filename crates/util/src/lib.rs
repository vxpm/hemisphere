/// Returns a `Box<[T; LEN]>` filled with `elem`.
#[inline(always)]
pub fn boxed_array<T: Clone, const LEN: usize>(elem: T) -> Box<[T; LEN]> {
    vec![elem; LEN].into_boxed_slice().try_into().ok().unwrap()
}

/// Like offset_of, except it also supports indexing arrays
#[macro_export]
macro_rules! offset_of {
    ($t:ty, $($path:tt)+) => {{
        const OFFSET: usize = {
            let data = core::mem::MaybeUninit::<$t>::uninit();
            let ptr = data.as_ptr();
            unsafe { (&raw const (*ptr).$($path)+).byte_offset_from(ptr) as usize }
        };

        OFFSET
    }}
}
