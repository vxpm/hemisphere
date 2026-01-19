use std::borrow::Cow;
use std::path::Path;

use addr2line::gimli;
use lazuli::Address;
use lazuli::modules::debug::{DebugModule, Location};
use mapfile_parser::MapFile;

fn demangle(s: &str) -> String {
    let cw_options = cwdemangle::DemangleOptions {
        omit_empty_parameters: true,
        mw_extensions: false,
    };
    if let Some(demangled) = cwdemangle::demangle(s, &cw_options) {
        return demangled;
    }

    addr2line::demangle_auto(Cow::Borrowed(s), Some(gimli::DW_LANG_C_plus_plus)).into_owned()
}

pub struct Addr2LineModule(addr2line::Loader);

impl Addr2LineModule {
    pub fn new(path: impl AsRef<Path>) -> Option<Self> {
        addr2line::Loader::new(path).ok().map(Self)
    }
}

impl DebugModule for Addr2LineModule {
    fn find_symbol(&self, addr: Address) -> Option<String> {
        self.0.find_symbol(addr.value() as u64).map(demangle)
    }

    fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.0
            .find_location(addr.value() as u64)
            .ok()
            .flatten()
            .map(|l| Location {
                file: l.file.map(Cow::Borrowed),
                line: l.line,
                column: l.column,
            })
    }
}

pub struct MapFileModule(MapFile);

impl MapFileModule {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self(MapFile::new_from_map_file(path.as_ref()))
    }
}

impl DebugModule for MapFileModule {
    fn find_symbol(&self, addr: Address) -> Option<String> {
        self.0
            .find_symbol_by_vram(addr.0 as u64)
            .0
            .map(|s| demangle(&s.symbol.name))
    }

    fn find_location(&self, addr: Address) -> Option<Location<'_>> {
        self.0
            .find_symbol_by_vram(addr.0 as u64)
            .0
            .map(|s| Location {
                file: Some(s.section.filepath.to_string_lossy()),
                line: None,
                column: None,
            })
    }
}
