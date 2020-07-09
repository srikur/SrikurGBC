pub const WRAM_BEGIN: usize = 0xC000;
pub const WRAM_END: usize = 0xDFFF;
pub const ERAM_BEGIN: usize = 0xA000;
pub const ERAM_END: usize = 0xBFFF;
pub const ZRAM_BEGIN: usize = 0xFF80;
pub const ZRAM_END: usize = 0xFFFE;
pub const INTERRUPT_FLAG: usize = 0xFF0F;
pub const INTERRUPT_ENABLE: usize = 0xFFFF;
pub const SWITCHABLE_BANK_BEGIN: usize = 0x4000;

pub struct MMU {
    pub game_rom: Vec<u8>,
    pub bios: [u8; 0x100],
    pub wram: [u8; 0x2000],
    pub zram: [u8; 0x80],
    pub interrupt_enable: u8,
    pub interrupt_flag: u8,
    pub cartridge_type: u8,

    /* Banking */
    pub current_rom_bank: u8,
    pub ram_enabled: bool,
    pub mbc1: bool,
    pub mbc2: bool,
    pub ram_banks: [u8; 0x8000],
    pub current_ram_bank: u8,
}
