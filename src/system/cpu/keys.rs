pub struct Keys {
    pub rows: [u8; 2],
    pub column: u8,
}

impl Keys {
    pub fn reset_keys(&mut self) {
        self.rows = [0xF, 0xF];
    }

    pub fn read_key(&self) -> u8 {
        match self.column {
            0x10 => self.rows[0], 
            0x20 => self.rows[1], 
            _ => 0
        }
    }

    pub fn write_key(&mut self, value: u8) {
        self.column = value & 0x30; 
    }
}