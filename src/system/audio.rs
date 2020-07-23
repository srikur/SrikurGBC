pub const SOUND_BEGIN: usize = 0xFF10;
pub const SOUND_END: usize = 0xFF3F;

pub struct APU {
    pub sound_data: [u8; SOUND_END - SOUND_BEGIN + 1],
}

impl APU {

    pub fn new() -> Self {
        APU {
            sound_data: [0; 0x30],
        }
    }
    pub fn read_byte(&self, address: usize) -> u8 {
        match address {
            _ => self.sound_data[address - SOUND_BEGIN],
        }
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        self.sound_data[address - SOUND_BEGIN] = value;
    }
}
