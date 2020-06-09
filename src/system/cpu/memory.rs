pub const WRAM_BEGIN: usize = 0xC000;
pub const WRAM_END: usize = 0xFDFF;
pub const ERAM_BEGIN: usize = 0xA000;
pub const ERAM_END: usize = 0xBFFF;
pub const ZRAM_BEGIN: usize = 0xFF80;
pub const ZRAM_END: usize = 0xFFFE;

pub struct MMU {
    pub bios: [u8; 0x100],
    pub rom: Vec<u8>, 
    pub wram: [u8; 0x2000],
    pub eram: [u8; 0x2000], 
    pub zram: [u8; 0x80],
    pub interrupt_enable: u8,
    pub interrupt_flag: u8,
    pub cartridge_type: u8
}