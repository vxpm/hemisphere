use crate::Address;

/// Allows the usage of const values in patterns. It's a neat trick!
struct ConstTrick<const N: u16>;
impl<const N: u16> ConstTrick<N> {
    const OUTPUT: u16 = N;
}

macro_rules! mmio {
    ($($addr:expr, $size:expr, $name:ident);* $(;)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u32)]
        pub enum Mmio {
            $(
                $name = ($size << 16) | $addr
            ),*
        }

        impl Mmio {
            #[inline(always)]
            pub fn address(self) -> Address {
                Address(0x0C00_0000 | (self as u32 & 0xFFFF))
            }

            #[inline(always)]
            pub fn size(self) -> u32 {
                (self as u32) >> 16
            }

            /// Given an offset into the `0x0C00_0000` region, returns the MMIO register at that
            /// address and the offset into it.
            pub fn find(offset: u16) -> Option<(Self, usize)> {
                match offset {
                    $(
                        $addr..ConstTrick::<{ $addr + $size }>::OUTPUT => Some((Self::$name, (offset - $addr) as usize)),
                    )*
                    _ => None,
                }
            }
        }
    };
}

mmio! {
    // OFFSET, LENGTH, NAME;

    // Video Interface
    0x2000, 2, VideoVerticalTiming;
    0x2002, 2, VideoDisplayConfig;
    0x2004, 8, VideoHorizontalTiming;
    0x200C, 4, VideoOddVerticalTiming;
    0x2010, 4, VideoEvenVerticalTiming;
    0x2014, 4, VideoOddBbInterval;
    0x2018, 4, VideoEvenBbInterval;
    0x201C, 4, VideoTopBaseLeft;
    0x2020, 4, VideoTopBaseRight;
    0x2024, 4, VideoBottomBaseLeft;
    0x2028, 4, VideoBottomBaseRight;
    0x204A, 2, VideoHorizontalScaling;
    0x206C, 2, VideoClock;

    // DSP Interface
    0x5000, 4, DspDspMailbox;
    0x5004, 4, DspCpuMailbox;
    0x500A, 2, DspControl;
    0x5020, 4, DspAramDmaRamBase;
    0x5024, 4, DspAramDmaAramBase;
    0x5028, 2, DspAramDmaControl;
}
