use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};

fn main() -> Result<()> {
    eyre_pretty::install()?;

    let dol = Dol::read(&mut std::fs::File::open("panda.dol").unwrap()).unwrap();

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 64,
    });
    hemisphere.load(&dol);

    loop {
        println!("==> executing at {}", hemisphere.pc);
        let executed = hemisphere.exec();
        println!("executed {executed} instructions");
    }

    Ok(())
}
