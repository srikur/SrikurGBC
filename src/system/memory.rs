use super::cartridge;
use std::path::Path;

pub const ERAM_BEGIN: usize = 0xA000;
pub const ERAM_END: usize = 0xBFFF;
pub const HRAM_BEGIN: usize = 0xFF80;
pub const HRAM_END: usize = 0xFFFE;

pub struct MMU {
    pub bios: [u8; 0x100],
    pub wram: [u8; 0x8000],
    pub hram: [u8; 0x80],

    /* Cartridge */
    pub cartridge: cartridge::Cartridge,

    // CGB
    pub wram_bank: usize,
}

impl MMU {
    pub fn new(path: impl AsRef<Path>) -> Self {
        MMU {
            bios: [0; 0x100],
            wram: [0; 0x8000],
            hram: [0; 0x80],
            wram_bank: 0x01,
            cartridge: cartridge::Cartridge::new(path),
        }
    }
}
