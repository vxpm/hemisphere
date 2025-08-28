mod gdb;

use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};

struct Emulator {
    hemisphere: Hemisphere,
}

fn main() -> Result<()> {
    eyre_pretty::install()?;

    let dol = Dol::read(&mut std::fs::File::open("panda.dol").unwrap()).unwrap();

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 128,
    });
    hemisphere.load(&dol);

    loop {
        let executed = hemisphere.exec();
        if hemisphere.cpu.pc == 0x8000_4010 {
            break;
        }
    }

    Ok(())
}
