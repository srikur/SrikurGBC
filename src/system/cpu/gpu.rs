pub const VRAM_BEGIN: usize = 0x8000;
pub const VRAM_END: usize = 0x9FFF;
pub const VRAM_SIZE: usize = VRAM_END - VRAM_BEGIN + 1;

pub const GPU_REGS_BEGIN: usize = 0xFF40;
pub const GPU_REGS_END: usize = 0xFF7F;

pub const OAM_BEGIN: usize = 0xFE00;
pub const OAM_END: usize = 0xFE9F;

#[derive(Copy, Clone)]
pub enum TilePixelValue {
    Zero,
    One,
    Two,
    Three,
}

pub type Tile = [[TilePixelValue; 8]; 8];
pub fn empty_tile() -> Tile {
    [[TilePixelValue::Zero; 8]; 8]
}

pub struct GPU {
    pub vram: [u8; VRAM_SIZE],
    pub tileset: [Tile; 384],
    pub screen_graphics: bool,
    pub oam: [u8; 0xA0],
    
    pub stat: u8,
    pub current_line: u8,

    /* LCD Monochrome Palettes */
    pub bg_palette: [u8; 4],
    pub obp0_palette: [u8; 4],
    pub obp1_palette: [u8; 4],

    /* GPU Registers */
    pub scroll_x: u8,
    pub scroll_y: u8,
    pub scanline_counter: u8,
    pub background_palette: BackgroundPalette,
}

#[derive(Copy, Clone)]
pub struct BackgroundPalette {
    pub switch_background: bool,
    pub sprites: bool,
    pub sprite_size: bool,
    pub tile_map: bool,
    pub tile_set: bool,
    pub window: bool,
    pub window_tile_map: bool,
    pub display: bool,
}

impl GPU {
    pub fn read_vram(&self, address: usize) -> u8 {
        self.vram[address]
    }

    pub fn read_registers(&self, address: usize) -> u8 {
        match address {
            /* LCD Control */
            0xFF40 => {
                panic!("Have not implemented reading LCD control registers!")
            }

            /* Scroll Y */
            0xFF42 => {
                self.scroll_y
            }

            /* Scroll X */
            0xFF43 => {
                self.scroll_x
            }

            /* Current scanline */
            0xFF44 => {
                self.current_line
            }

            _ => panic!("Have not implemented read_registers() for '{:X}' for the GPU!", address)
        }
    }

    pub fn write_registers(&mut self, address: usize, value: u8) {
        match address {
            /* LCD Control */
            0xFF40 => {
                self.background_palette.switch_background =
                    if value & 0x01 != 0 { true } else { false };
                self.background_palette.tile_map = if value & 0x08 != 0 { true } else { false };
                self.background_palette.tile_set = if value & 0x10 != 0 { true } else { false };
                self.background_palette.switch_background = if value & 0x80 != 0 { true } else { false };
            }

            /* Scroll Y */
            0xFF42 => self.scroll_y = value,

            /* Scroll X */
            0xFF43 => self.scroll_x = value,


            0xFF45 => self.stat = value,

            /* BG Palette Data */
            0xFF47 => {
                for i in 0..4 {
                    match (value >> (i * 2)) & 3 {
                        0 => self.bg_palette[i] = 255,
                        1 => self.bg_palette[i] = 192,
                        2 => self.bg_palette[i] = 96,
                        3 => self.bg_palette[i] = 0,
                        _ => panic!("Something went wrong while writing the bg palette data!")
                    }
                }
            }

            /* OBP0 Palette Data */
            0xFF48 => {
                for i in 0..4 {
                    match (value >> (i * 2)) & 3 {
                        0 => self.obp0_palette[i] = 255,
                        1 => self.obp0_palette[i] = 192,
                        2 => self.obp0_palette[i] = 96,
                        3 => self.obp0_palette[i] = 0,
                        _ => panic!("Something went wrong while writing the bg palette data!")
                    }
                }
            }

            /* OBP1 Palette Data */
            0xFF49 => {
                for i in 0..4 {
                    match (value >> (i * 2)) & 3 {
                        0 => self.obp1_palette[i] = 255,
                        1 => self.obp1_palette[i] = 192,
                        2 => self.obp1_palette[i] = 96,
                        3 => self.obp1_palette[i] = 0,
                        _ => panic!("Something went wrong while writing the bg palette data!")
                    }
                }
            }

            _ => panic!("Have not yet implemented write_registers() for '{:X}' for the GPU!", address)
        }
    }

    pub fn write_vram(&mut self, index: usize, value: u8) {
        self.vram[index] = value;
        if index >= 0x1800 {
            return;
        }

        let normalized_index = index & 0xFFFE;
        let byte1 = self.vram[normalized_index];
        let byte2 = self.vram[normalized_index];

        let tile_index = index / 16;
        let row_index = (index % 16) / 2;

        for pixel_index in 0..8 {
            let mask = 1 << (7 - pixel_index);
            let lsb = byte1 & mask;
            let msb = byte2 & mask;

            let value = match (lsb != 0, msb != 0) {
                (true, true) => TilePixelValue::Three,
                (false, true) => TilePixelValue::Two,
                (true, false) => TilePixelValue::One,
                (false, false) => TilePixelValue::Zero,
            };

            self.tileset[tile_index][row_index][pixel_index] = value;
        }
    }
}