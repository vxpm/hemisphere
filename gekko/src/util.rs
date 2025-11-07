//! Generic utilities.

/// Returns a `Box<[T; LEN]>` filled with `elem`.
#[inline(always)]
pub fn boxed_array<T: Clone, const LEN: usize>(elem: T) -> Box<[T; LEN]> {
    vec![elem; LEN].into_boxed_slice().try_into().ok().unwrap()
}
