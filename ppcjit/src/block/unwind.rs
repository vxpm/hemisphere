use cranelift::codegen::gimli::{
    RunTimeEndian,
    write::{Address, EhFrame, EndianVec, FrameTable, Writer},
};
use cranelift::codegen::isa::{TargetIsa, unwind::UnwindInfo};

unsafe extern "C" {
    fn __register_frame(fde: *const u8);
    fn __deregister_frame(fde: *const u8);
}

pub struct UnwindGuard(Box<[u8]>);

impl Drop for UnwindGuard {
    fn drop(&mut self) {
        unsafe {
            __deregister_frame(self.0.as_ptr() as _);
        }
    }
}

pub fn register(isa: &dyn TargetIsa, addr: usize, info: &UnwindInfo) -> UnwindGuard {
    let fde = match info {
        UnwindInfo::SystemV(info) => info.to_fde(Address::Constant(addr as u64)),
        _ => panic!("unsupported unwind information"),
    };

    let mut table = FrameTable::default();
    let cie_id = table.add_cie(match isa.create_systemv_cie() {
        Some(cie) => cie,
        None => panic!("ISA does not support System V unwind information"),
    });

    table.add_fde(cie_id, fde);

    let mut eh_frame = EhFrame(EndianVec::new(RunTimeEndian::default()));
    table.write_eh_frame(&mut eh_frame).unwrap();

    if cfg!(any(
        all(target_os = "linux", target_env = "gnu"),
        target_os = "freebsd"
    )) {
        // libgcc expects a terminating "empty" length, so write a 0 length at the end of the table.
        eh_frame.0.write_u32(0).unwrap();
    }

    let frame_table = eh_frame.0.into_vec().into_boxed_slice();
    if cfg!(any(
        all(target_os = "linux", target_env = "gnu"),
        target_os = "freebsd"
    )) {
        // On gnu (libgcc), `__register_frame` will walk the FDEs until an entry of length 0
        let ptr = frame_table.as_ptr();
        unsafe { __register_frame(ptr) };
    } else {
        // For libunwind, `__register_frame` takes a pointer to a single FDE
        let start = frame_table.as_ptr();
        let end = unsafe { start.add(frame_table.len()) };
        let mut current = start;

        // Walk all of the entries in the frame table and register them
        while current < end {
            let len = unsafe { std::ptr::read::<u32>(current as *const u32) } as usize;

            // Skip over the CIE
            if current != start {
                unsafe { __register_frame(current) };
            }

            // Move to the next table entry (+4 because the length itself is not inclusive)
            current = unsafe { current.add(len + 4) };
        }
    }

    UnwindGuard(frame_table)
}
