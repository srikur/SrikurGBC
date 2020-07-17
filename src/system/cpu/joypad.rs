pub const JOYPAD_INPUT: usize = 0xFF00;

pub struct Joypad {
    pub matrix: u8,
    pub select: u8,
}

pub enum Keys {
    Right  = 0x01,
    Left   = 0x02,
    Up     = 0x04,
    Down   = 0x08,
    A      = 0x10,
    B      = 0x20,
    Select = 0x40,
    Start  = 0x80,
}

impl Joypad {
    pub fn get_joypad_state(&self) -> u8 {
        if (self.select & 0x10) == 0x00 {
            return self.select | (self.matrix & 0x0f);
        }
        if (self.select & 0x20) == 0x00 {
            return self.select | (self.matrix >> 4);
        }
        self.select
    }

    pub fn set_joypad_state(&mut self, value: u8) {
        self.select = value;
    }
}
