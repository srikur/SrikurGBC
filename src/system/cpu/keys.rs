pub const JOYPAD_INPUT: usize = 0xFF00;

pub struct Keys {
    pub joypad_state: u8,
    pub joypad_register: u8,
}

impl Keys {
    pub fn get_joypad_state(&self) -> u8 {
        let mut res = self.joypad_register;
        res ^= 0xFF;

        if res & 0x10 == 0 {
            let top = (self.joypad_state >> 4) | 0xF0;
            res &= top;
        } else if res & 0x20 == 0 {
            let bot = (self.joypad_state & 0xF) | 0xF0;
            res &= bot;
        }

        res
    }
}
