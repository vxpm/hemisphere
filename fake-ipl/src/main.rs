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

type OsReportCallbackFn = fn(*const c_char);
type AppEntry = fn() -> !;

type ApploaderInitFn = fn(OsReportCallbackFn);
type ApploaderMainFn = fn(*mut *mut c_void, *mut usize, *mut usize) -> bool;
type ApploaderCloseFn = fn() -> AppEntry;

type ApploaderEntryFn = fn(*mut ApploaderInitFn, *mut ApploaderMainFn, *mut ApploaderCloseFn);

#[unsafe(no_mangle)]
pub fn os_report_callback(_message: *const c_char) {
    // ignored for now
}

#[inline(never)]
pub fn disk_load(address: *mut c_void, length: usize, offset: usize) {
    let disk_cmd0 = core::ptr::without_provenance_mut::<usize>(0xCC00_6008);
    let disk_cmd1 = core::ptr::without_provenance_mut::<usize>(0xCC00_600C);
    let disk_cmd2 = core::ptr::without_provenance_mut::<usize>(0xCC00_6010);
    let dma_base = core::ptr::without_provenance_mut::<*mut c_void>(0xCC00_6014);
    let dma_length = core::ptr::without_provenance_mut::<usize>(0xCC00_6018);
    let disk_control = core::ptr::without_provenance_mut::<usize>(0xCC00_601C);

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
pub fn main(entry: ApploaderEntryFn) {
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
