pub const VRAM_BEGIN: usize = 0x8000;
pub const VRAM_END: usize = 0x9FFF;
pub const VRAM_SIZE: usize = VRAM_END - VRAM_BEGIN + 1;
pub const GPU_REGS_BEGIN: usize = 0xFF40;
pub const GPU_REGS_END: usize = 0xFF4B;
pub const OAM_BEGIN: usize = 0xFE00;
pub const OAM_END: usize = 0xFE9F;

#[derive(Copy, Clone)]
pub enum TilePixelValue {
    Black,
    DarkGray,
    LightGray,
    White,
}

pub struct GPU {
    pub vram: [u8; VRAM_SIZE],

    /* Pixels for OpenGL */
    pub screen_data: [[[u8; 3]; 160]; 144],

    pub oam: [u8; 0xA0],
    pub lyc: u8, // 0xFF45

    /* LCD Control Register 0xFF40 */
    pub lcd_enabled: bool, // Bit 7
    pub window_tilemap_display_select: bool, // Bit 6
    pub window_display_enable: bool, // Bit 5
    pub bg_window_tile_data_select: bool, // Bit 4
    pub bg_tile_map_display_select: bool, // Bit 3
    pub obj_size: bool, // Bit 2
    pub obj_display_enable: bool, // Bit 1
    pub bg_window_display_priority: bool, // Bit 0

    pub stat: u8,
    pub current_line: u8, // 0xFF44

    pub window_x: u8, // 0xFF4B
    pub window_y: u8, // 0xFF4A

    /* LCD Monochrome Palettes */
    pub bg_palette: u8,
    pub obp0_palette: u8, // 0xFF48
    pub obp1_palette: u8, // 0xFF49

    /* GPU Registers */
    pub scroll_x: u8, // 0xFF43
    pub scroll_y: u8, // 0xFF42
    pub scanline_counter: i16,
}

impl GPU {
    pub fn draw_scanline(&mut self) {
        if self.lcd_enabled {
            self.render_tiles();
            self.render_sprites();
        }
    }

    fn render_sprites(&mut self) {

        if !self.obj_display_enable { return }

        for sprite in 0..=40 {
            let y_pos = self.oam[sprite * 4 - 16];
            let x_pos = self.oam[(sprite * 4 + 1) - 8];
            let tile_location = self.oam[sprite * 4 + 2];
            let attributes = self.oam[sprite * 4 + 3];
            let x_flip: bool = attributes & 0x40 != 0;
            let y_flip: bool = attributes & 0x20 != 0;
            let y_size = if self.obj_size { 16 } else { 8 };

            if (self.current_line >= y_pos) && (self.current_line < (y_pos + y_size)) {
                let mut line: i32 = i32::from(self.current_line - y_pos);
                if y_flip {
                    line -= y_size as i32;
                    line *= -1;
                }
                line *= 2;

                let data1: u8 = self.vram[i32::from(tile_location as i32 * 16 + line) as usize];
                let data2: u8 = self.vram[i32::from(tile_location as i32 * 16 + line + 1) as usize];

                for tile_pixel in 7..=0 {
                    let mut color_bit: i32 = tile_pixel;
                    if x_flip {
                        color_bit -= 7;
                        color_bit *= -1;
                    }

                    let mut color_num: i32 = if (data2 & (1 << color_bit)) != 0 { 1 } else { 0 };
                    let bit1 = if (data1 & (1 << color_bit)) != 0 { 1 } else { 0 };
                    color_num = (color_num << 1) | bit1;

                    let color = self.determine_color(color_num, if attributes & 0x10 != 0 {self.obp1_palette} else {self.obp0_palette});
                    let pix = x_pos + 7 - tile_pixel as u8;
                    
                    self.screen_data[self.current_line as usize][pix as usize][0] = match color {
                        TilePixelValue::White => 255,
                        TilePixelValue::LightGray => 204,
                        TilePixelValue::DarkGray => 119, 
                        TilePixelValue::Black => 0,
                    };
        
                    self.screen_data[self.current_line as usize][pix as usize][1] = match color {
                        TilePixelValue::White => 255,
                        TilePixelValue::LightGray => 204,
                        TilePixelValue::DarkGray => 119, 
                        TilePixelValue::Black => 0,
                    };
        
                    self.screen_data[self.current_line as usize][pix as usize][2] = match color {
                        TilePixelValue::White => 255,
                        TilePixelValue::LightGray => 204,
                        TilePixelValue::DarkGray => 119, 
                        TilePixelValue::Black => 0,
                    };
                }
            }
        }
    }

    fn render_tiles(&mut self) {

        let tile_data: u16;
        let mut using_window: bool = false;
        let window_x = self.window_x.wrapping_sub(7);
        let mut unsigned: bool = true;
        let background_memory: u16;

        if self.window_display_enable {
            if self.window_y <= self.current_line {
                using_window = true;
            }
        }

        if self.bg_window_tile_data_select {
            tile_data = 0x8000;
        } else {
            tile_data = 0x8800;
            unsigned = false;
        }

        if !using_window {
            if self.bg_tile_map_display_select {
                background_memory = 0x9C00;
            } else {
                background_memory = 0x9800;
            }
        } else {
            if self.window_tilemap_display_select {
                background_memory = 0x9C00;
            } else {
                background_memory = 0x9800;
            }
        }

        let y_pos: u8;

        if !using_window {
            y_pos = self.scroll_y + self.current_line;
        } else {
            y_pos = self.current_line - self.window_y;
        }

        let tile_row: u16 = ((y_pos as u64 / 8) * 32) as u16;

        for pixel in 0..=159 {
            let mut x_pos = pixel + self.scroll_x;

            if using_window && (pixel >= window_x) {
                x_pos = pixel - window_x;
            }

            let tile_col: u16 = (x_pos / 8) as u16;
            let tile_num: i16;

            if unsigned {
                tile_num = self.vram[(background_memory + tile_col + tile_row - VRAM_BEGIN as u16) as usize] as u8 as i16;
            } else {
                tile_num = self.vram[(background_memory + tile_row + tile_col - VRAM_BEGIN as u16) as usize] as i8 as i16;
            }

            let mut tile_location = tile_data;
            if unsigned {
                tile_location = (tile_location as i32 + (tile_num * 16) as i32) as u16;
            } else {
                tile_location = (tile_location as i32 + (tile_num + 128 * 16) as i32) as u16;
            }

            let line = (y_pos % 8) * 2;
            let data1 = self.vram[(tile_location + line as u16) as usize - VRAM_BEGIN];
            let data2 = self.vram[(tile_location + 1 + line as u16) as usize - VRAM_BEGIN];
            
            let color_bit: i32 = ((x_pos as i32 % 8) - 7) * -1;
            let mut color_num: i32 = if (data2 & (1 << color_bit)) != 0 { 1 } else { 0 };
            let bit1 = if (data1 & (1 << color_bit)) != 0 { 1 } else { 0 };
            color_num = (color_num << 1) | bit1;

            let color: TilePixelValue = self.determine_color(color_num, self.bg_palette);

            if (self.current_line > 143) || (pixel > 159) {
                panic!("Something went wrong in render_tiles()");
            }

            self.screen_data[self.current_line as usize][pixel as usize][0] = match color {
                TilePixelValue::White => 255,
                TilePixelValue::LightGray => 204,
                TilePixelValue::DarkGray => 119, 
                TilePixelValue::Black => 0,
            };

            self.screen_data[self.current_line as usize][pixel as usize][1] = match color {
                TilePixelValue::White => 255,
                TilePixelValue::LightGray => 204,
                TilePixelValue::DarkGray => 119, 
                TilePixelValue::Black => 0,
            };

            self.screen_data[self.current_line as usize][pixel as usize][2] = match color {
                TilePixelValue::White => 255,
                TilePixelValue::LightGray => 204,
                TilePixelValue::DarkGray => 119, 
                TilePixelValue::Black => 0,
            };
        }
    }

    fn determine_color(&mut self, color_num: i32, value: u8) -> TilePixelValue {
        let high: u8; 
        let low: u8;

        match color_num {
            0 => { high = 1; low = 0; },
            1 => { high = 3; low = 2; },
            2 => { high = 5; low = 4; },
            3 => { high = 7; low = 6; },
            _ => panic!(),
        }

        let mut color = if (value & (1 << high)) != 0 { 1 } else { 0 };
        color |= if (value & (1 << low)) != 0 { 1 } else { 0 };

        match color {
            0 => TilePixelValue::White,
            1 => TilePixelValue::LightGray,
            2 => TilePixelValue::DarkGray, 
            3 => TilePixelValue::Black,
            _ => panic!()
        }
    }

    pub fn read_vram(&self, address: usize) -> u8 {
        self.vram[address]
    }

    pub fn write_vram(&mut self, index: usize, value: u8) {
        self.vram[index] = value;
    }

    pub fn read_registers(&self, address: usize) -> u8 {
        match address {
            /* LCD Control */
            0xFF40 => {
                //return lcd control
                (if self.lcd_enabled { 1 } else { 0 }) << 7 | 
                (if self.window_tilemap_display_select { 1 } else { 0 }) << 6 | 
                (if self.window_display_enable { 1 } else { 0 }) << 5 |
                (if self.bg_window_tile_data_select { 1 } else { 0 }) << 4 |
                (if self.bg_tile_map_display_select { 1 } else { 0 }) << 3 |
                (if self.obj_size { 1 } else { 0 }) << 2 |
                (if self.obj_display_enable { 1 } else { 0 }) << 1 |
                (if self.bg_window_display_priority { 1 } else { 0 })
            }

            /* Scroll Y */
            0xFF42 => self.scroll_y,

            /* Scroll X */
            0xFF43 => self.scroll_x,

            /* Current scanline */
            0xFF44 => self.current_line,

            _ => panic!(
                "Have not implemented read_registers() for '{:X}' for the GPU!",
                address
            ),
        }
    }

    pub fn write_registers(&mut self, address: usize, value: u8) {
        match address {
            /* LCD Control */
            0xFF40 => {
                // set lcd control
                self.lcd_enabled = value & 0x80 != 0;
                self.window_tilemap_display_select = value & 0x40 != 0;
                self.window_display_enable = value & 0x20 != 0;
                self.bg_window_tile_data_select = value & 0x10 != 0;
                self.bg_tile_map_display_select = value & 0x08 != 0;
                self.obj_size = value & 0x04 != 0;
                self.obj_display_enable = value & 0x02 != 0;
                self.bg_window_display_priority = value & 0x01 != 0;
            }

            /* STAT */
            0xFF41 => self.stat = value,

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

            0xFF4A => self.window_y = value,

            0xFF4B => self.window_x = value,

            _ => panic!(
                "Have not yet implemented write_registers() for '{:X}' for the GPU!",
                address
            ),
        }
    }
}
