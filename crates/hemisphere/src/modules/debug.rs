//! Debug module interface.

use gekko::Address;
use std::borrow::Cow;

pub struct Location<'a> {
    pub file: Option<Cow<'a, str>>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl<'a> std::fmt::Display for Location<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.file.as_deref().unwrap_or("<unknown>"),
            self.line.unwrap_or(0),
            self.column.unwrap_or(0)
        )
    }
}

impl<'a> Location<'a> {
    pub fn into_owned(self) -> Location<'static> {
        Location {
            file: self.file.map(|s| Cow::Owned(s.into_owned())),
            line: self.line,
            column: self.column,
        }
    }
}

/// Trait for debug info modules.
pub trait DebugModule: Send {
    fn find_symbol(&self, addr: Address) -> Option<String>;
    fn find_location(&self, addr: Address) -> Option<Location<'_>>;
}

/// An implementation of [`AudioModule`] which does nothing.
#[derive(Debug, Clone, Copy)]
pub struct NopDebugModule;

impl DebugModule for NopDebugModule {
    fn find_symbol(&self, _: Address) -> Option<String> {
        None
    }

    fn find_location(&self, _: Address) -> Option<Location<'_>> {
        None
    }
}
