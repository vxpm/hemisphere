pub mod bus;
pub mod mmu;

#[cfg(test)]
mod test {
    use bitos::integer::{u11, u15};
    use hemicore::Address;
    use ppcjit::registers::Bat;

    pub fn translate(bats: &[Bat; 4], addr: Address) -> Option<Address> {
        for bat in bats {
            if (bat.start()..=bat.end()).contains(&addr) {
                return Some(bat.translate(addr));
            }
        }

        None
    }

    #[test]
    fn test() {
        let a = Bat::default()
            .with_effective_page_index(u15::new(0))
            .with_real_page_number(u15::new(0xFF00))
            .with_block_length_mask(u11::new(0x0000));

        dbg!(bytesize::ByteSize(a.block_length() as u64));
        dbg!(a.start()..a.end());
        dbg!(a.physical_start()..a.physical_end());

        let b = Bat::default()
            .with_effective_page_index(u15::new(1))
            .with_real_page_number(u15::new(0xFF00))
            .with_block_length_mask(u11::new(0x0000));

        dbg!(bytesize::ByteSize(b.block_length() as u64));
        dbg!(b.start()..b.end());
        dbg!(b.physical_start()..b.physical_end());

        let bats = [a, b, Bat::default(), Bat::default()];
        dbg!(translate(&bats, Address(0x1FFFF + 1)));
    }
}
