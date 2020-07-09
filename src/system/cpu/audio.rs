pub const SOUND_BEGIN: usize = 0xFF10;
pub const SOUND_END: usize = 0xFF3F;

pub struct APU {}

impl APU {
    pub fn read_byte(&self, address: usize) -> u8 {
        match address {
            _ => panic!("Sound controller not yet implemented!"),
        }
    }

    pub fn write_byte(&mut self, _address: usize, _value: u8) {}
}
