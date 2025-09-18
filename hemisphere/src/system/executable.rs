use std::path::Path;

use addr2line::Location;
use common::Address;
use dol::{
    Dol,
    binrw::{BinRead, io::BufReader},
};
use easyerr::{Error, ResultExt};

pub enum Code {
    Dol(Dol),
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("executable has an unknown format")]
    UnknownFormat,
    #[error("failed to load debug info file")]
    DebugInfo,
    #[error(transparent)]
    Io { source: std::io::Error },
}

pub struct Executable {
    code: Code,
    debug: Option<addr2line::Loader>,
}

impl Executable {
    pub fn open(exec: &Path, debug: Option<&Path>) -> Result<Self, OpenError> {
        let exec_file = std::fs::File::open(exec).context(OpenCtx::Io)?;
        let code = match exec.extension().and_then(|s| s.to_str()) {
            Some("dol") => Code::Dol(Dol::read(&mut BufReader::new(exec_file)).unwrap()),
            _ => return Err(OpenError::UnknownFormat),
        };

        let debug = if let Some(debug) = debug {
            let loader = addr2line::Loader::new(debug).map_err(|_| OpenError::DebugInfo)?;
            Some(loader)
        } else {
            None
        };

        Ok(Self { code, debug })
    }

    pub fn code(&self) -> &Code {
        &self.code
    }

    pub fn find_symbol(&self, addr: Address) -> Option<&str> {
        self.debug
            .as_ref()
            .and_then(|d| d.find_symbol(addr.value() as u64))
    }

    pub fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.debug
            .as_ref()
            .and_then(|d| d.find_location(addr.value() as u64).ok())
            .flatten()
    }
}
