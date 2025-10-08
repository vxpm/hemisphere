//! Generic utilities.

use crate::Primitive;
use std::{collections::VecDeque, io::BufRead};

/// Returns a `Box<[T; LEN]>` filled with `elem`.
#[inline(always)]
pub fn boxed_array<T: Clone, const LEN: usize>(elem: T) -> Box<[T; LEN]> {
    vec![elem; LEN].into_boxed_slice().try_into().ok().unwrap()
}

pub trait DataProvider {
    fn as_data(&mut self) -> &[u8];
    fn consume(&mut self, amount: usize);
}

impl DataProvider for &[u8] {
    fn as_data(&mut self) -> &[u8] {
        self
    }

    fn consume(&mut self, amount: usize) {
        *self = &self[amount..];
    }
}

pub struct DataReader<'a, T> {
    provider: &'a mut T,
    read: usize,
}

impl<'a, T> DataReader<'a, T>
where
    T: DataProvider,
{
    pub fn new(data: &'a mut T) -> Self {
        Self {
            provider: data,
            read: 0,
        }
    }

    /// Reads a primitive if there is enough data for it.
    pub fn read_be<P>(&mut self) -> Option<P>
    where
        P: Primitive,
    {
        let slice = &self.provider.as_data()[self.read..];
        (slice.len() >= size_of::<P>()).then(|| {
            self.read += size_of::<P>();
            P::read_be_bytes(slice)
        })
    }

    /// Reads a sequence of `length` bytes if there is enough data for it.
    pub fn read_bytes(&mut self, length: usize) -> Option<Vec<u8>> {
        let slice = &self.provider.as_data()[self.read..];
        (slice.len() >= length).then(|| {
            self.read += length;
            slice[..length].to_vec()
        })
    }

    /// Returns how many bytes of data are remaining in the data.
    pub fn remaining(&mut self) -> usize {
        self.provider.as_data().len() - self.read
    }

    /// Consumes the read bytes returns how many bytes were read
    pub fn finish(self) -> usize {
        self.read
    }
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
    pub fn read(&mut self) -> DataReader<'_, DataStream> {
        DataReader::new(self)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DataProvider for DataStream {
    fn as_data(&mut self) -> &[u8] {
        self.data.make_contiguous()
    }

    fn consume(&mut self, amount: usize) {
        self.data.consume(amount);
    }
}
