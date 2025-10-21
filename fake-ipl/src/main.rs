#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use core::{
    arch::global_asm,
    ffi::{c_char, c_void},
    mem::MaybeUninit,
};

global_asm!(include_str!("init.s"));

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

type OsReportCallbackFn = extern "C" fn(*const c_char);
type AppEntryFn = extern "C" fn() -> !;

type ApploaderEntryFn =
    extern "C" fn(*mut ApploaderInitFn, *mut ApploaderMainFn, *mut ApploaderCloseFn);
type ApploaderInitFn = extern "C" fn(OsReportCallbackFn);
type ApploaderMainFn = extern "C" fn(*mut *mut c_void, *mut usize, *mut usize) -> bool;
type ApploaderCloseFn = extern "C" fn() -> AppEntryFn;

#[unsafe(no_mangle)]
pub extern "C" fn os_report_callback(_message: *const c_char) {
    // ignored for now
}

#[inline(never)]
pub extern "C" fn disk_load(address: *mut c_void, length: usize, offset: usize) {
    let disk_cmd0 = 0xCC00_6008 as *mut usize;
    let disk_cmd1 = 0xCC00_600C as *mut usize;
    let disk_cmd2 = 0xCC00_6010 as *mut usize;
    let dma_base = 0xCC00_6014 as *mut *mut c_void;
    let dma_length = 0xCC00_6018 as *mut usize;
    let disk_control = 0xCC00_601C as *mut usize;

    unsafe {
        disk_cmd0.write_volatile(0xA800_0000);
        disk_cmd1.write_volatile(offset >> 2);
        disk_cmd2.write_volatile(length);

        dma_base.write_volatile(address);
        dma_length.write_volatile(length);

        disk_control.write_volatile(3);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(entry: ApploaderEntryFn) {
    let mut init = MaybeUninit::uninit();
    let mut main = MaybeUninit::uninit();
    let mut close = MaybeUninit::uninit();
    entry(init.as_mut_ptr(), main.as_mut_ptr(), close.as_mut_ptr());

    let init = unsafe { init.assume_init() };
    let main = unsafe { main.assume_init() };
    let close = unsafe { close.assume_init() };

    init(os_report_callback);
    loop {
        let mut address = core::ptr::null_mut();
        let mut length = 0;
        let mut offset = 0;
        let needs_more_data = main(&raw mut address, &raw mut length, &raw mut offset);

        disk_load(address, length, offset);

        if !needs_more_data {
            break;
        }
    }

    let entry = close();
    entry();
}
