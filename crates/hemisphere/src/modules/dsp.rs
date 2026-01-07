use crate::system::dspi::Dsp;
use std::{
    ops::{Deref, DerefMut},
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

pub enum DspRef<'a> {
    Raw(&'a Dsp),
    RwLock(RwLockReadGuard<'a, Dsp>),
}

impl<'a> Deref for DspRef<'a> {
    type Target = Dsp;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Raw(r) => r,
            Self::RwLock(r) => r,
        }
    }
}

pub enum DspMut<'a> {
    Raw(&'a mut Dsp),
    RwLock(RwLockWriteGuard<'a, Dsp>),
}

impl<'a> Deref for DspMut<'a> {
    type Target = Dsp;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Raw(r) => r,
            Self::RwLock(r) => r,
        }
    }
}

impl<'a> DerefMut for DspMut<'a> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Raw(r) => r,
            Self::RwLock(r) => r,
        }
    }
}

pub trait DspModule: Send {
    fn prepare(&mut self, ram: &mut [u8]);

    /// Drives the DSP core forward by _at most_ the specified amount of instructions. The actual
    /// number of instructions executed is returned.
    fn exec(&mut self, instructions: u32) -> u32;

    /// Returns a reference to the DSP state.
    fn state(&self) -> DspRef<'_>;

    /// Returns a mutable reference to the DSP state.
    fn state_mut(&mut self) -> DspMut<'_>;
}
