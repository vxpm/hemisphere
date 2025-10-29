use crate::{Dsp, Ins};

impl Dsp {
    pub fn halt(&mut self, _: Ins) {
        self.control.halt = true;
    }
}
