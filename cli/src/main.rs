use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
};
use tinylog::{drain::fmt::FmtDrain, info, logger::LoggerFamily};

fn main() -> Result<()> {
    eyre_pretty::install()?;

    let dol = Dol::read(&mut std::fs::File::open("panda.dol").unwrap()).unwrap();

    let family = LoggerFamily::builder()
        .with_drain(FmtDrain::new(std::io::stdout(), true))
        .build();
    let root = family.logger("cli", tinylog::Level::Trace);

    let mut hemisphere = Hemisphere::new(Config {
        instructions_per_block: 64,
        logger: root.child("core", tinylog::Level::Trace),
    });
    hemisphere.load(&dol);

    loop {
        info!(root, "==> executing at {}", hemisphere.pc);
        let executed = hemisphere.exec();
        info!(root, "executed {executed} instructions");

        if hemisphere.pc == 0x8000_4010 {
            break;
        }
    }

    info!(root, "{}", format!("{:?}", hemisphere.bus.video));

    Ok(())
}
