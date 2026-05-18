//! `Header` and `Version` parsers, mirroring `nanovdb::io::Header`
//! (util/IO.h:112-125) and `nanovdb::Version` (NanoVDB.h:540).

use crate::error::Error;

/// `"NanoVDB0"` packed as little-endian uint64.
pub const NANOVDB_MAGIC_NUMBER: u64 = 0x304244566f6e614e;

/// 11/11/10-bit packed major/minor/patch triple used both in the segment
/// header and inside each `GridMetaData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version(pub u32);

impl Version {
    pub fn major(self) -> u32 {
        (self.0 >> 21) & ((1 << 11) - 1)
    }
    pub fn minor(self) -> u32 {
        (self.0 >> 10) & ((1 << 11) - 1)
    }
    pub fn patch(self) -> u32 {
        self.0 & ((1 << 10) - 1)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major(), self.minor(), self.patch())
    }
}

/// File compression codec. Matches `nanovdb::io::Codec` (util/IO.h:61).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Codec {
    None = 0,
    Zip = 1,
    Blosc = 2,
    Other = 0xffff,
}

impl Codec {
    pub fn from_raw(v: u16) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Zip,
            2 => Self::Blosc,
            _ => Self::Other,
        }
    }
}

/// One segment header (16 bytes). One per contiguous block of grids; a
/// `.nvdb` file may contain multiple segments concatenated.
#[derive(Debug, Clone, Copy)]
pub struct SegmentHeader {
    pub magic: u64,
    pub version: Version,
    pub grid_count: u16,
    pub codec: Codec,
}

impl SegmentHeader {
    pub const BYTE_SIZE: usize = 16;

    pub fn parse(bytes: &[u8], file_offset: u64) -> Result<Self, Error> {
        if bytes.len() < Self::BYTE_SIZE {
            return Err(Error::Truncated {
                offset: file_offset,
                wanted: Self::BYTE_SIZE,
                actual: bytes.len(),
            });
        }
        let magic = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        if magic != NANOVDB_MAGIC_NUMBER {
            return Err(Error::BadMagic(magic));
        }
        let version = Version(u32::from_le_bytes(bytes[8..12].try_into().unwrap()));
        let grid_count = u16::from_le_bytes(bytes[12..14].try_into().unwrap());
        let codec_raw = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
        Ok(SegmentHeader {
            magic,
            version,
            grid_count,
            codec: Codec::from_raw(codec_raw),
        })
    }
}
