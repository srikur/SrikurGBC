use super::cartridge;
use std::path::{Path};

pub const WRAM_BEGIN: usize = 0xC000;
pub const WRAM_END: usize = 0xDFFF;
pub const ERAM_BEGIN: usize = 0xA000;
pub const ERAM_END: usize = 0xBFFF;
pub const ZRAM_BEGIN: usize = 0xFF80;
pub const ZRAM_END: usize = 0xFFFE;

pub struct MMU {
    pub bios: [u8; 0x100],
    pub wram: [u8; 0x2000],
    pub zram: [u8; 0x80],

    /* Cartridge */
    pub cartridge: cartridge::Cartridge,
}

impl MMU {
    pub fn new(path: impl AsRef<Path>) -> Self {
        MMU {
            bios: [0; 0x100],
            wram: [0; 0x2000],
            zram: [0; 0x80],
            cartridge: cartridge::Cartridge::new(path),
        }
    }
}