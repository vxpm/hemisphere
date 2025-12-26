use std::{
    ffi::CStr,
    ops::{Deref, DerefMut},
};

use crate::system::mem;

/// IPL decoding function, thanks @hazelwiss!!
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

pub struct Ipl(Vec<u8>);

impl Ipl {
    pub fn new(mut data: Vec<u8>) -> Self {
        assert_eq!(data.len(), mem::IPL_LEN);

        let ipl_message = CStr::from_bytes_until_nul(&data).unwrap();
        let pal_message = "(C) 1999-2001 Nintendo.  All rights reserved.(C) 1999 ArtX Inc.  All rights reserved.PAL  Revision 1.0  ";
        if ipl_message.to_str().is_ok_and(|s| s == pal_message) {
            tracing::info!("IPL was detected as EU/PAL.");
            decode_ipl(&mut data[0x0000_0100..0x001A_EEE8]);
        } else {
            tracing::info!("IPL was not detected as EU/PAL. Assuming USA/NTSC.");
            decode_ipl(&mut data[0x0000_0100..0x0015_EE40]);
        }

        Self(data)
    }
}

impl Deref for Ipl {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Ipl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
