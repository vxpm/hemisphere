use eyre_pretty::eyre::Result;
use hemisphere::{
    Config, Hemisphere,
    dolfile::{Dol, binrw::BinRead},
    hemicore::Address,
};
use image::{Rgb, RgbImage};

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
        // println!("executed {executed} instructions");

        if hemisphere.pc == 0x8000_4010 {
            break;
        }
    }

    fn conv(y: u8, cb: u8, cr: u8) -> [u8; 3] {
        let (y, cb, cr) = (y as f32, cb as f32 - 128.0, cr as f32 - 128.0);
        let r = y + 1.371 * cr;
        let g = y - 0.698 * cr - 0.336 * cb;
        let b = y + 1.732 * cb;

        [
            r.clamp(0.0, 255.0) as u8,
            g.clamp(0.0, 255.0) as u8,
            b.clamp(0.0, 255.0) as u8,
        ]
    }

    dbg!(&hemisphere.bus.video);
    let xfb = 0x8000_0000 + (hemisphere.bus.video.regs.tfbl.xfb_address().value() as u32) << 9;
    let len = 640 * 480 * 4;
    let data = (xfb..xfb + len)
        .step_by(4)
        .map(|i| hemisphere.bus.read::<u32>(Address(i)))
        .collect::<Vec<_>>();

    let mut img = RgbImage::new(640 * 2, 480);
    for (index, data) in data.into_iter().enumerate() {
        let [cr, y1, cb, y0] = data.to_le_bytes();

        let mut pixel_index = index as u32 * 2;
        let pixel = img.get_pixel_mut(pixel_index % (640 * 2), pixel_index / (640 * 2));
        *pixel = Rgb(conv(y0, cb, cr));

        pixel_index += 1;
        let pixel = img.get_pixel_mut(pixel_index % (640 * 2), pixel_index / (640 * 2));
        *pixel = Rgb(conv(y1, cb, cr));
    }

    img.save("out.png");

    Ok(())
}
