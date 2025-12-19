//! A simple GameCube .iso file parser using [`binrw`].
pub mod apploader;
pub mod filesystem;

use binrw::{BinRead, BinWrite, NullString};
use easyerr::{Error, ResultExt};
use std::io::{Read, Seek, SeekFrom};

pub use binrw;

use crate::filesystem::FileSystem;

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(big, magic = 0xC233_9F3D_u32)]
pub struct MagicWord;

/// The header of a GameCube .iso file.
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(big)]
pub struct Header {
    pub console_id: u8,
    pub game_id: u16,
    pub country_code: u8,
    pub maker_code: u16,
    pub disk_id: u8,
    pub version: u8,
    pub audio_streaming: u8,
    pub stream_buffer_size: u8,
    #[brw(pad_before = 0x12)]
    pub magic: MagicWord,
    #[brw(pad_size_to = 0x3E0)]
    #[brw(assert(game_name.len() <= 0x3E0))]
    pub game_name: NullString,
    pub debug_monitor_offset: u32,
    pub debug_monitor_target: u32,
    #[brw(pad_before = 0x18)]
    pub bootfile_offset: u32,
    pub filesystem_offset: u32,
    pub filesystem_size: u32,
    pub max_filesystem_size: u32,
    pub user_position: u32,
    pub user_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Console {
    GameCube,
    Wii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Country {
    Japan,
    Pal,
    Usa,
}

impl Header {
    pub fn game_code(&self) -> u32 {
        let game = self.game_id.to_be_bytes();
        u32::from_be_bytes([self.console_id, game[0], game[1], self.country_code])
    }

    pub fn game_code_str(&self) -> Option<String> {
        String::from_utf8(self.game_code().to_be_bytes().into()).ok()
    }

    pub fn console(&self) -> Option<Console> {
        Some(match self.console_id {
            b'G' => Console::GameCube,
            b'R' => Console::Wii,
            _ => return None,
        })
    }

    pub fn country(&self) -> Option<Country> {
        Some(match self.country_code {
            b'J' => Country::Japan,
            b'P' => Country::Pal,
            b'E' => Country::Usa,
            _ => return None,
        })
    }

    pub fn audio_streaming(&self) -> Option<bool> {
        Some(match self.audio_streaming {
            0 => false,
            1 => true,
            _ => return None,
        })
    }
}

/// An apploader program in a .iso.
#[derive(Debug, BinRead, BinWrite)]
#[brw(big)]
pub struct Apploader {
    #[brw(pad_size_to = 0x10)]
    #[brw(assert(version.len() <= 0x10))]
    pub version: NullString,
    pub entrypoint: u32,
    pub size: u32,
    pub trailer_size: u32,
    #[brw(pad_before = 0x4)]
    #[br(count = size)]
    pub data: Vec<u8>,
}

/// A GameCube .iso file.
#[derive(Debug)]
pub struct Iso<R> {
    /// Header of the ISO.
    header: Header,
    /// Reader of the contents.
    reader: R,
}

#[derive(Debug, Error)]
pub enum ReadError {
    #[error(transparent)]
    Io { source: std::io::Error },
    #[error(transparent)]
    Format { source: binrw::Error },
}

impl<R> Iso<R>
where
    R: Read + Seek,
{
    pub fn new(mut reader: R) -> Result<Self, binrw::Error> {
        let header = Header::read(&mut reader)?;
        Ok(Self { header, reader })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn reader(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn bootfile(&mut self) -> Result<dol::Dol, ReadError> {
        self.reader
            .seek(SeekFrom::Start(self.header.bootfile_offset as u64))
            .context(ReadCtx::Io)?;

        dol::Dol::read(&mut self.reader).context(ReadCtx::Format)
    }

    pub fn apploader(&mut self) -> Result<Apploader, ReadError> {
        self.reader
            .seek(SeekFrom::Start(0x2440))
            .context(ReadCtx::Io)?;

        Apploader::read(&mut self.reader).context(ReadCtx::Format)
    }

    pub fn filesystem(&mut self) -> Result<FileSystem, ReadError> {
        self.reader
            .seek(SeekFrom::Start(self.header.filesystem_offset as u64))
            .context(ReadCtx::Io)?;

        FileSystem::read(&mut self.reader).context(ReadCtx::Format)
    }
}
