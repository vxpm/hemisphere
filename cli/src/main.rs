use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
    hemicore::Address,
};
use std::{thread::sleep, time::Duration};

fn main() -> Result<()> {
    eyre_pretty::install()?;

    let dol = Dol::read(&mut std::fs::File::open("panda.dol").unwrap()).unwrap();

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 1,
    });
    hemisphere.load(&dol);

    let mut slow = false;
    loop {
        let executed = hemisphere.exec();
        println!("executed {executed} instructions");

        if hemisphere.pc == 0x8000_414c {
            slow = true;
            println!(
                "r3({}) r4({}) r5({}) ctr({})",
                Address(hemisphere.cpu.user.gpr[3]),
                Address(hemisphere.cpu.user.gpr[4]),
                Address(hemisphere.cpu.user.gpr[5]),
                hemisphere.cpu.user.ctr,
            );
            // sleep(Duration::from_millis(2000));
        }

        if slow {
            println!(
                "r3({}) r4({}) r5({}) ctr({})",
                Address(hemisphere.cpu.user.gpr[3]),
                Address(hemisphere.cpu.user.gpr[4]),
                Address(hemisphere.cpu.user.gpr[5]),
                hemisphere.cpu.user.ctr,
            );

            sleep(Duration::from_nanos(1));
        }
    }

    Ok(())
}
