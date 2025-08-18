use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};

fn main() -> Result<()> {
    eyre_pretty::install()?;

    let dol = Dol::read(&mut std::fs::File::open("panda.dol").unwrap()).unwrap();

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 2,
    });
    hemisphere.load(&dol);

    loop {
        let executed = hemisphere.exec();
        println!("executed {executed} instructions");
        dbg!(&hemisphere.cpu.user.gpr[1]);
    }

    Ok(())
}
