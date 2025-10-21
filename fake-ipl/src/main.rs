#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use core::{
    arch::global_asm,
    ffi::{CStr, c_char, c_void},
    mem::MaybeUninit,
};

use numtoa::{NumToA, numtoa_u32};

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
#[inline(never)]
pub extern "C" fn stdout_write(mut message: *const c_char) {
    let stdout = 0xCC00_7000 as *mut u8;
    unsafe {
        loop {
            let data = *message;
            if data == 0 {
                break;
            }

            stdout.write_volatile(data);
            message = message.add(1);
        }
    }
}

#[inline(never)]
extern "C" fn print(message: &'static CStr) {
    stdout_write(message.as_ptr());
}

#[inline(never)]
extern "C" fn println(message: &'static CStr) {
    stdout_write(message.as_ptr());
    stdout_write(c"\r\n".as_ptr());
}

#[inline(never)]
extern "C" fn println_dec(num: u32) {
    let mut buffer = [0u8; 32];
    stdout_write(numtoa_u32(num, 10, &mut buffer[0..24]).as_ptr().cast());
    stdout_write(c"\r\n".as_ptr());
}

#[inline(never)]
extern "C" fn println_hex(num: u32) {
    let mut buffer = [0u8; 16];
    stdout_write(numtoa_u32(num, 16, &mut buffer[0..8]).as_ptr().cast());
    stdout_write(c"\r\n".as_ptr());
}

#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn os_report_callback(message: *const c_char) {
    print(c"[APPLOADER] ");
    stdout_write(message);
}

#[unsafe(no_mangle)]
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
#[inline(never)]
pub extern "C" fn main(entry: ApploaderEntryFn) {
    println(c"[IPL] Executing apploader entry...");

    let mut init = MaybeUninit::uninit();
    let mut main = MaybeUninit::uninit();
    let mut close = MaybeUninit::uninit();
    entry(init.as_mut_ptr(), main.as_mut_ptr(), close.as_mut_ptr());

    let init = unsafe { init.assume_init() };
    let main = unsafe { main.assume_init() };
    let close = unsafe { close.assume_init() };

    println(c"[IPL] Entry executed:");
    print(c"  Init: 0x");
    println_hex(init as u32);
    print(c"  Main: 0x");
    println_hex(main as u32);
    print(c"  Close: 0x");
    println_hex(close as u32);

    println(c"[IPL] Running apploader init...");
    init(os_report_callback);
    println(c"[IPL] Init executed. Calling apploader main on a loop...");

    let mut i = 0;
    loop {
        print(c"[IPL] Loop iteration ");
        println_dec(i);

        let mut address = core::ptr::null_mut();
        let mut length = 0;
        let mut offset = 0;
        let needs_more_data = main(&raw mut address, &raw mut length, &raw mut offset);
        if !needs_more_data {
            break;
        }

        println(c"[IPL] Loading disk data:");
        print(c"  Target: 0x");
        println_hex(address as u32);
        print(c"  Offset: 0x");
        println_hex(offset as u32);
        print(c"  Length: 0x");
        println_hex(length as u32);

        disk_load(address, length, offset);

        i += 1;
    }

    println(c"[IPL] Finished running main. Closing apploader...");
    let entry = close();
    println(c"[IPL] Apploader closed! Jumping to bootfile entrypoint");

    entry();
}
