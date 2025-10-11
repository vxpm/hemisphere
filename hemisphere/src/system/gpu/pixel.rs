use bitos::{bitos, integer::u2};

// NOTE: might be wrong
#[bitos(3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    RGB8Z24 = 0x0,
    RGBA6ZZ24 = 0x1,
    RGB565Z16 = 0x2,
    Z24 = 0x3,
    Y8 = 0x4,
    U8 = 0x5,
    V8 = 0x6,
    YUV420 = 0x7,
}

#[bitos(32)]
#[derive(Debug, Default)]
pub struct CopyCmd {
    #[bits(0..2)]
    pub clamp: u2,
    #[bits(4..7)]
    pub format: Format,
    #[bits(7..9)]
    pub gamma: u2,
    #[bits(11)]
    pub clear: bool,
    #[bits(14)]
    pub to_xfb: bool,
}

#[bitos(16)]
#[derive(Debug, Default)]
pub struct InterruptStatus {
    #[bits(2)]
    pub token: bool,
    #[bits(3)]
    pub finish: bool,
}

#[derive(Debug, Default)]
pub struct Interface {
    pub interrupt: InterruptStatus,
}

impl Interface {
    pub fn write_interrupt(&mut self, status: u16) {
        self.interrupt = InterruptStatus::from_bits(self.interrupt.to_bits() & !status)
    }
}
