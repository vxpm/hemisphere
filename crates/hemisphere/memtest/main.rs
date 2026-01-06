use std::ops::RangeInclusive;

use gekko::{Address, MemoryManagement};
use hemisphere::{
    modules::{
        audio::NopAudioModule, debug::NopDebugModule, disk::NopDiskModule, input::NopInputModule,
        render::NopRenderModule,
    },
    system::{
        self, Modules, System,
        mem::{RAM_END, RAM_LEN, RAM_START},
    },
};
use indicatif::ProgressBar;

fn test_inner(sys: &mut System, range: RangeInclusive<u32>) {
    let bar = ProgressBar::new(RAM_LEN as u64);
    for addr in (range).step_by(4) {
        let addr = Address(addr);

        sys.write_fast(addr, 0xDEAD_BEEFu32);
        assert_eq!(sys.read_slow(addr), Some(0xDEAD_BEEFu32));
        assert_eq!(sys.read_fast(addr), Some(0xDEAD_BEEFu32));
        sys.write_fast(addr, 0u32);

        bar.inc(4);
    }
    bar.finish();
}

/// Tests physical memory, i.e. no BATs.
fn test_physical(sys: &mut System) {
    println!("=> testing physical");

    let mman = MemoryManagement::default();
    sys.mem.build_bat_lut(&mman);
    sys.cpu
        .supervisor
        .config
        .msr
        .set_data_addr_translation(false);

    test_inner(sys, RAM_START..=RAM_END);
}

/// Tests default logical memory.
fn test_logical(sys: &mut System) {
    println!("=> testing logical");
    let mut mman = MemoryManagement::default();
    mman.setup_default_bats();
    sys.mem.build_bat_lut(&mman);
    sys.cpu
        .supervisor
        .config
        .msr
        .set_data_addr_translation(false);

    println!("physical");
    test_inner(sys, RAM_START..=RAM_END);

    sys.cpu
        .supervisor
        .config
        .msr
        .set_data_addr_translation(true);

    println!("cached ram");
    test_inner(sys, 0x8000_0000 + RAM_START..=0x8000_0000 + RAM_END);

    println!("uncached ram");
    test_inner(sys, 0xC000_0000 + RAM_START..=0xC000_0000 + RAM_END);
}

fn main() {
    let modules = Modules {
        audio: Box::new(NopAudioModule),
        debug: Box::new(NopDebugModule),
        disk: Box::new(NopDiskModule),
        input: Box::new(NopInputModule),
        render: Box::new(NopRenderModule),
    };

    let mut system = System::new(
        modules,
        system::Config {
            ipl: None,
            sideload: None,
            force_ipl: false,
        },
    );

    test_physical(&mut system);
    test_logical(&mut system);
}
