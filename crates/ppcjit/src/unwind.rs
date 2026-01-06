#[cfg(unix)]
mod unix {
    use cranelift::codegen::gimli::{
        RunTimeEndian,
        write::{Address, EhFrame, EndianVec, FrameTable, Writer},
    };
    use cranelift::codegen::isa::{TargetIsa, unwind::UnwindInfo};

    unsafe extern "C" {
        fn __register_frame(fde: *const u8);
        fn __deregister_frame(fde: *const u8);
    }

    const USING_LIBGCC: bool = cfg!(any(
        all(target_os = "linux", target_env = "gnu"),
        target_os = "freebsd"
    ));

    fn frame_table(isa: &dyn TargetIsa, addr: usize, info: &UnwindInfo) -> Option<Box<[u8]>> {
        let fde = match info {
            UnwindInfo::SystemV(info) => info.to_fde(Address::Constant(addr as u64)),
            _ => return None,
        };

        let mut table = FrameTable::default();
        let cie_id = table.add_cie(isa.create_systemv_cie()?);
        table.add_fde(cie_id, fde);

        let mut eh_frame = EhFrame(EndianVec::new(RunTimeEndian::default()));
        table.write_eh_frame(&mut eh_frame).unwrap();

        if USING_LIBGCC {
            // libgcc expects an end marker (zero length)
            eh_frame.0.write_u32(0).unwrap();
        }

        Some(eh_frame.0.into_vec().into_boxed_slice())
    }

    pub struct UnwindHandle(Box<[u8]>);

    impl UnwindHandle {
        pub unsafe fn new(isa: &dyn TargetIsa, addr: usize, info: &UnwindInfo) -> Option<Self> {
            let frame_table = frame_table(isa, addr, info)?;
            unsafe { __register_frame(frame_table.as_ptr()) };

            Some(UnwindHandle(frame_table))
        }
    }

    impl Drop for UnwindHandle {
        fn drop(&mut self) {
            unsafe {
                __deregister_frame(self.0.as_ptr());
            }
        }
    }
}

#[cfg(not(unix))]
mod dummy {
    use cranelift::codegen::isa::{TargetIsa, unwind::UnwindInfo};

    pub struct UnwindHandle;

    impl UnwindHandle {
        pub unsafe fn new(_: &dyn TargetIsa, _: usize, _: &UnwindInfo) -> Option<Self> {
            None
        }
    }
}

#[cfg(unix)]
pub type UnwindHandle = unix::UnwindHandle;
#[cfg(not(unix))]
pub type UnwindHandle = dummy::UnwindHandle;
