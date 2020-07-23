pub const VRAM_BEGIN: usize = 0x8000;
pub const VRAM_END: usize = 0x9FFF;
pub const VRAM_SIZE: usize = 0x2000;
pub const GPU_REGS_BEGIN: usize = 0xFF40;
pub const GPU_REGS_END: usize = 0xFF4B;
pub const OAM_BEGIN: usize = 0xFE00;
pub const OAM_END: usize = 0xFE9F;
pub const EXTRA_SPACE_BEGIN: usize = 0xFF01;
pub const EXTRA_SPACE_END: usize = 0xFF3F;

use super::interrupts::{Interrupt, Interrupts};
use std::cell::RefCell;
use std::rc::Rc;
use std::cmp::{Ord, Ordering};

pub struct Lcdc {
    pub data: u8,
}

struct SpriteAttributes {
    priority: bool,
    yflip: bool,
    xflip: bool,
    palette_number: bool,
}

impl From<u8> for SpriteAttributes {
    fn from(uint: u8) -> Self {
        Self {
            priority: uint & (1 << 7) != 0,
            yflip: uint & (1 << 6) != 0,
            xflip: uint & (1 << 5) != 0,
            palette_number: uint & (1 << 4) != 0,
        }
    }
}

impl Lcdc {
    // LCD Display Enable
    pub fn bit7(&self) -> bool {
        self.data & 0x80 != 0
    }

    // Window Tile Map Data Select
    pub fn bit6(&self) -> bool {
        self.data & 0x40 != 0
    }

    // Window Display Enable
    pub fn bit5(&self) -> bool {
        self.data & 0x20 != 0
    }

    // BG & Window Tile Data Select
    pub fn bit4(&self) -> bool {
        self.data & 0x10 != 0
    }

    // BG Tile Map Data Select
    pub fn bit3(&self) -> bool {
        self.data & 0x08 != 0
    }

    // OBJ Size
    pub fn bit2(&self) -> bool {
        self.data & 0x04 != 0
    }

    // OBJ Display Enable
    pub fn bit1(&self) -> bool {
        self.data & 0x02 != 0
    }

    // BG/Window Display/Priority
    pub fn bit0(&self) -> bool {
        self.data & 0x01 != 0
    }

}

pub struct Stat {
    pub enable_ly_interrupt: bool,
    pub enable_m2_interrupt: bool,
    pub enable_m1_interrupt: bool,
    pub enable_m0_interrupt: bool,
    pub mode: u8,
}

pub struct GPU {

    pub intref: Rc<RefCell<Interrupt>>,

    /* VRAM */
    pub vram: [u8; VRAM_SIZE],

    /* Pixels for OpenGL */
    pub screen_data: [[[u8; 3]; 160]; 144],

    pub oam: [u8; 0xA0],
    pub lyc: u8, // 0xFF45

    // Extra Space??
    pub extra: [u8; 0x3F],

    pub priority: [u8; 160],

    /* LCD Control Register 0xFF40 */
    pub lcdc: Lcdc,

    pub stat: Stat,         // 0xFF41
    pub current_line: u8, // 0xFF44

    pub window_x: u8, // 0xFF4B
    pub window_y: u8, // 0xFF4A

    /* LCD Monochrome Palettes */
    pub bg_palette: u8,   // 0xFF47
    pub obp0_palette: u8, // 0xFF48
    pub obp1_palette: u8, // 0xFF49

    /* GPU Registers */
    pub scroll_x: u8, // 0xFF43
    pub scroll_y: u8, // 0xFF42
    pub scanline_counter: u32,

    // Graphics
    pub vblank: bool,
}

impl GPU {

    pub fn new(intref: Rc<RefCell<Interrupt>>) -> Self {
        GPU {
            intref: intref,
            screen_data: [[[0xFFu8; 3]; 160]; 144],
            vram: [0; VRAM_SIZE],
            oam: [0; 0xA0],
            stat: Stat {
                enable_ly_interrupt: false,
                enable_m2_interrupt: false,
                enable_m1_interrupt: false,
                enable_m0_interrupt: false,
                mode: 0,
            },
            priority: [0; 160],
            extra: [0; 0x3F],
            bg_palette: 0,
            obp0_palette: 0,
            obp1_palette: 0,
            lcdc: Lcdc {
                data: 0,
            },
            scroll_x: 0,
            scroll_y: 0,
            lyc: 0,
            window_x: 0,
            window_y: 0,
            current_line: 0,
            scanline_counter: 456,
            vblank: false,
        }
    }

    pub fn update_graphics(&mut self, cycles: u32) {
        if !self.lcdc.bit7() {
            return;
        }

        if cycles == 0 {
            return;
        }

        let c = (cycles - 1) / 80 + 1;
        for i in 0..c {
            if i == (c - 1) {
                self.scanline_counter += cycles % 80
            } else {
                self.scanline_counter += 80
            }
            let d = self.scanline_counter;
            self.scanline_counter %= 456;
            if d != self.scanline_counter {
                self.current_line = (self.current_line + 1) % 154;
                if self.stat.enable_ly_interrupt && self.current_line == self.lyc {
                    self.intref.borrow_mut().set_interrupt(Interrupts::LCDStat);
                }
            }
            if self.current_line >= 144 {
                if self.stat.mode == 1 {
                    continue;
                }
                self.stat.mode = 1;
                self.vblank = true;
                self.intref.borrow_mut().set_interrupt(Interrupts::VBlank);
                if self.stat.enable_m1_interrupt {
                    self.intref.borrow_mut().set_interrupt(Interrupts::LCDStat);
                }
            } else if self.scanline_counter <= 80 {
                if self.stat.mode == 2 {
                    continue;
                }
                self.stat.mode = 2;
                if self.stat.enable_m2_interrupt {
                    self.intref.borrow_mut().set_interrupt(Interrupts::LCDStat);
                }
            } else if self.scanline_counter <= (80 + 172) {
                self.stat.mode = 3;
            } else {
                if self.stat.mode == 0 {
                    continue;
                }
                self.stat.mode = 0;
                if self.stat.enable_m0_interrupt {
                    self.intref.borrow_mut().set_interrupt(Interrupts::LCDStat);
                }
                self.draw_scanline();
            }
        }
    }

    pub fn draw_scanline(&mut self) {
        if self.lcdc.bit0() {
            self.render_tiles();
        }

        if self.lcdc.bit1() {
            self.render_sprites();
        }
    }

    fn render_sprites(&mut self) {
        let sprite_size = if self.lcdc.bit2() { 16 } else { 8 };

        for sprite in 0..40 {
            let sprite_address = (sprite as u16) * 4;
            let y_pos = self.oam[sprite_address as usize].wrapping_sub(16);
            let x_pos = self.oam[sprite_address as usize + 1].wrapping_sub(8);
            let tile_number = self.oam[sprite_address as usize + 2] & if self.lcdc.bit2() { 0xFE } else { 0xFF };
            let tile_attribute = SpriteAttributes::from(self.oam[sprite_address as usize + 3]);

            if y_pos <= 0xFF - sprite_size + 1 {
                if self.current_line < y_pos || self.current_line > y_pos + sprite_size - 1 {
                    continue;
                }
            } else {
                if self.current_line > y_pos.wrapping_add(sprite_size) - 1 {
                    continue;
                }
            }
            if x_pos >= (160 as u8) && x_pos <= (0xFF - 7) {
                continue;
            }

            let tile_y = if tile_attribute.yflip {
                sprite_size - 1 - self.current_line.wrapping_sub(y_pos)
            } else {
                self.current_line.wrapping_sub(y_pos)
            };
            let tile_y_addr = u16::from(tile_number) * 16 + u16::from(tile_y) * 2;
            let tile_y_data: [u8; 2] = {
                let b1 = self.vram[tile_y_addr as usize];
                let b2 = self.vram[tile_y_addr as usize + 1];
                [b1, b2]
            };

            for pixel in 0..8 {
                if x_pos.wrapping_add(pixel) >= (160 as u8) {
                    continue;
                }
                let tile_x = if tile_attribute.xflip { 7 - pixel } else { pixel };

                let color_l = if tile_y_data[0] & (0x80 >> tile_x) != 0 { 1 } else { 0 };
                let color_h = if tile_y_data[1] & (0x80 >> tile_x) != 0 { 2 } else { 0 };
                let color = color_h | color_l;
                if color == 0 {
                    continue;
                }

                let sprite_prio = self.priority[x_pos.wrapping_add(pixel) as usize];
                if !tile_attribute.priority {
                    if (sprite_prio != 0) && (color <= sprite_prio) {
                        continue;
                    }
                } else if tile_attribute.priority {
                    if sprite_prio != 0 {
                        continue;
                    }
                }

                /*
                let sprite_prio = self.priority[x_pos.wrapping_add(pixel) as usize];
                let skip: bool = sprite_prio != 0;
                if skip {
                    continue;
                }
                */

                let color = if tile_attribute.palette_number {
                    match self.obp1_palette >> (2 * color) & 0x03 {
                        0x00 => 255,
                        0x01 => 192,
                        0x02 => 96,
                        _ => 0,
                    }
                } else {
                    match self.obp0_palette >> (2 * color) & 0x03 {
                        0x00 => 255,
                        0x01 => 192,
                        0x02 => 96,
                        _ => 0,
                    }
                };

                self.screen_data[self.current_line as usize][x_pos.wrapping_add(pixel) as usize] = [color, color, color];
            }
        }
    }

    fn render_tiles(&mut self) {
        let using_window = self.lcdc.bit5() && (self.window_y <= self.current_line);
        let tile_data: u16 = if self.lcdc.bit4() { 0x8000 } else { 0x8800 };
        let window_x = self.window_x.wrapping_sub(7);

        let y_pos: u8 = if !using_window {
            self.scroll_y.wrapping_add(self.current_line as u8)
        } else {
            self.current_line.wrapping_sub(self.window_y)
        };
        let tile_row = (u16::from(y_pos) >> 3) & 0x1F;

        for pixel in 0..160 {
            let x_pos = if using_window && pixel as u8 >= window_x {
                pixel as u8 - window_x
            } else {
                self.scroll_x.wrapping_add(pixel as u8)
            };
            let tile_col = (u16::from(x_pos) >> 3) & 0x1F;

            let background_memory: u16 = if using_window && pixel as u8 >= window_x {
                if self.lcdc.bit6() {
                    0x9c00
                } else {
                    0x9800
                }
            } else if self.lcdc.bit3() {
                0x9c00
            } else {
                0x9800
            };

            let tile_address = background_memory + (tile_row * 32) + tile_col;
            let tile_number = self.vram[tile_address as usize - 0x8000];
            let mut tile_offset = if self.lcdc.bit4() {
                i16::from(tile_number)
            } else {
                i16::from(tile_number as i8) + 128
            } as u16;
            tile_offset *= 16;
            let tile_location = tile_data + tile_offset;

            let tile_y = y_pos % 8; 
            let tile_y_data: [u8; 2] = {
                let a = self.vram[(tile_location + u16::from(tile_y * 2)) as usize - 0x8000];
                let b = self.vram[(tile_location + u16::from(tile_y * 2) + 1) as usize - 0x8000];
                [a, b]
            };
            let tile_x = x_pos % 8;

            let color_low = if tile_y_data[0] & (0x80 >> tile_x) != 0 { 1 } else { 0 };
            let color_high = if tile_y_data[1] & (0x80 >> tile_x) != 0 { 2 } else { 0 };
            let color = color_high | color_low;

            self.priority[pixel] = color;

            let color = match self.bg_palette >> (2 * color) & 0x03 {
                0x00 => 255,
                0x01 => 192,
                0x02 => 96,
                _ => 0,
            };

            self.screen_data[self.current_line as usize][pixel] = [color, color, color];
        }
    }

    pub fn read_vram(&self, address: usize) -> u8 {
        self.vram[address]
    }

    pub fn write_vram(&mut self, index: usize, value: u8) {
        self.vram[index] = value;
    }

    #[rustfmt::skip]
    pub fn read_registers(&self, address: usize) -> u8 {
        match address {
            /* LCD Control */
            0xFF40 => self.lcdc.data, 
            
            /* STAT */
            0xFF41 => {
                let bit6 = if self.stat.enable_ly_interrupt { 0x40 } else { 0x00 };
                let bit5 = if self.stat.enable_m2_interrupt { 0x20 } else { 0x00 };
                let bit4 = if self.stat.enable_m1_interrupt { 0x10 } else { 0x00 };
                let bit3 = if self.stat.enable_m0_interrupt { 0x08 } else { 0x00 };
                let bit2 = if self.current_line == self.lyc { 0x04 } else { 0x00 };
                bit6 | bit5 | bit4 | bit3 | bit2 | self.stat.mode
            }

            /* Scroll Y */
            0xFF42 => self.scroll_y,

            /* Scroll X */
            0xFF43 => self.scroll_x,

            /* Current scanline */
            0xFF44 => self.current_line,

            /* LY Compare */
            0xFF45 => self.lyc,

            /* BG Palette Data */
            0xFF47 => self.bg_palette,

            /* OBP0 Palette Data */
            0xFF48 => self.obp0_palette,

            /* OBP1 Palette Data */
            0xFF49 => self.obp1_palette,

            /* Window Y */
            0xFF4A => self.window_y,

            /* Window X */
            0xFF4B => self.window_x,

            _ => panic!("Unimplemented GPU Register: {:X}", address),
        }
    }

    pub fn write_registers(&mut self, address: usize, value: u8) {
        match address {
            /* LCD Control */
            0xFF40 => {
                self.lcdc.data = value;

                if !self.lcdc.bit7() {
                    self.scanline_counter = 0;
                    self.current_line = 0;
                    self.stat.mode = 0;
                    self.screen_data = [[[0xFF; 3]; 160]; 144];
                    self.vblank = true;
                }
            }

            /* STAT */
            0xFF41 => {
                self.stat.enable_ly_interrupt = value & 0x40 != 0x00;
                self.stat.enable_m2_interrupt = value & 0x20 != 0x00;
                self.stat.enable_m1_interrupt = value & 0x10 != 0x00;
                self.stat.enable_m0_interrupt = value & 0x08 != 0x00;
            },

            /* Scroll Y */
            0xFF42 => self.scroll_y = value,

            /* Scroll X */
            0xFF43 => self.scroll_x = value,

            /* Current Scanline */
            0xFF44 => self.current_line = 0,

            /* LY Compare */
            0xFF45 => self.lyc = value,

            /* BG Palette Data */
            0xFF47 => self.bg_palette = value,

            /* OBP0 Palette Data */
            0xFF48 => self.obp0_palette = value,

            /* OBP1 Palette Data */
            0xFF49 => self.obp1_palette = value,

            /* Window Y */
            0xFF4A => self.window_y = value,

            /* Window X */
            0xFF4B => self.window_x = value,

            _ => panic!("Unimplemented GPU Register: {:X}", address),
        }
    }
}
