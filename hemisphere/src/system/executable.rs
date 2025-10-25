use common::Address;
use dol::{
    Dol,
    binrw::{BinRead, io::BufReader},
};
use easyerr::{Error, ResultExt};
use std::path::Path;

pub struct Location<'a> {
    pub file: Option<&'a str>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

pub trait DebugInfo: Send {
    fn find_symbol(&self, addr: Address) -> Option<String>;
    fn find_location(&self, addr: Address) -> Option<Location<'_>>;
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("executable has an unknown format")]
    UnknownFormat,
    #[error(transparent)]
    Io { source: std::io::Error },
}

pub enum Executable {
    Dol(Dol),
}

impl Executable {
    pub fn open(exec: &Path) -> Result<Self, OpenError> {
        let exec_file = std::fs::File::open(exec).context(OpenCtx::Io)?;
        Ok(match exec.extension().and_then(|s| s.to_str()) {
            Some("dol") => Executable::Dol(Dol::read(&mut BufReader::new(exec_file)).unwrap()),
            _ => return Err(OpenError::UnknownFormat),
        })
    }
}
