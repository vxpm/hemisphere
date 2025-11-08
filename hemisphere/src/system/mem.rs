pub const RAM_LEN: u32 = 24 * bytesize::MIB as u32;
pub const L2C_LEN: u32 = 16 * bytesize::KIB as u32;
pub const IPL_LEN: u32 = 2 * bytesize::MIB as u32;
pub const SRAM_LEN: u32 = 64;

pub struct Memory {
    pub ram: Box<[u8; RAM_LEN as usize]>,
    pub l2c: Box<[u8; L2C_LEN as usize]>,
    pub ipl: Box<[u8; IPL_LEN as usize]>,
    pub sram: Box<[u8; SRAM_LEN as usize]>,
}

/// IPL decoding function, thanks hazelwiss!!
fn decode_ipl(ipl: &mut [u8]) {
    let mut acc = 0u8;
    let mut nacc = 0u8;

    let mut t = 0x2953u16;
    let mut u = 0xD9C2u16;
    let mut v = 0x3FF1u16;

    let mut x = 1u8;

    let mut it = 0;
    while it < ipl.len() {
        let t0 = t as u8 & 1;
        let t1 = (t as u8 >> 1) & 1;
        let u0 = u as u8 & 1;
        let u1 = (u as u8 >> 1) & 1;
        let v0 = v as u8 & 1;

        x ^= t1 ^ v0;
        x ^= u0 | u1;
        x ^= (t0 ^ u1 ^ v0) & (t0 ^ u0);

        if t0 == u0 {
            v >>= 1;
            if v0 != 0 {
                v ^= 0xb3d0;
            }
        }

        if t0 == 0 {
            u >>= 1;
            if u0 != 0 {
                u ^= 0xfb10;
            }
        }

        t >>= 1;
        if t0 != 0 {
            t ^= 0xa740;
        }

        nacc += 1;
        acc = acc.wrapping_mul(2).wrapping_add(x);
        if nacc == 8 {
            ipl[it] ^= acc;
            it += 1;
            nacc = 0;
        }
    }
}

impl Memory {
    pub fn new(mut ipl: Vec<u8>) -> Self {
        assert_eq!(ipl.len(), IPL_LEN as usize);

        // TODO: the range depends on the IPL ROM! this is hardcoded for PAL
        decode_ipl(&mut ipl[0x0000_0100..0x001A_EEE8]);

        Self {
            ram: util::boxed_array(0x00),
            l2c: util::boxed_array(0x00),
            ipl: ipl.try_into().unwrap(),
            sram: util::boxed_array(0x00),
        }
    }
}
