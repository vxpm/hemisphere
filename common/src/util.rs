//! Generic utilities.

use std::{collections::VecDeque, io::BufRead};

use crate::Primitive;

/// Returns a `Box<[T; LEN]>` filled with `elem`.
#[inline(always)]
pub fn boxed_array<T: Clone, const LEN: usize>(elem: T) -> Box<[T; LEN]> {
    vec![elem; LEN].into_boxed_slice().try_into().ok().unwrap()
}

/// A data stream.
#[derive(Debug, Clone, Default)]
pub struct DataStream {
    data: VecDeque<u8>,
}

impl DataStream {
    pub fn push_be<P>(&mut self, value: P)
    where
        P: Primitive,
    {
        let mut bytes = [0; 8];
        value.write_be_bytes(&mut bytes);

        for byte in &bytes[0..size_of::<P>()] {
            self.data.push_back(*byte);
        }
    }

    /// Start a reading operation.
    pub fn read(&mut self) -> ReadOp<'_> {
        self.data.make_contiguous();
        ReadOp {
            data: &mut self.data,
            read: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct ReadOp<'a> {
    data: &'a mut VecDeque<u8>,
    read: usize,
}

impl ReadOp<'_> {
    /// Reads a primitive from the stream if there are enough bytes for it.
    pub fn read_be<P>(&mut self) -> Option<P>
    where
        P: Primitive,
    {
        let (data, _) = self.data.as_slices();
        let slice = &data[self.read..];

        (slice.len() >= size_of::<P>()).then(|| {
            self.read += size_of::<P>();
            P::read_be_bytes(slice)
        })
    }

    /// Consumes the read bytes.
    pub fn consume(self) {
        self.data.consume(self.read);
    }
}
