pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: FlagsRegister,
    pub h: u8,
    pub l: u8,
}

#[derive(Copy, Clone)]
pub struct FlagsRegister {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
}

impl std::convert::From<FlagsRegister> for u8 {
    fn from(flag: FlagsRegister) -> u8 {
        return (if flag.zero { 1 } else { 0 }) << 7
            | (if flag.subtract { 1 } else { 0 }) << 6
            | (if flag.half_carry { 1 } else { 0 }) << 5
            | (if flag.carry { 1 } else { 0 }) << 4;
    }
}

impl std::convert::From<u8> for FlagsRegister {
    fn from(byte: u8) -> Self {
        let zero = ((byte >> 7) & 0x01) != 0;
        let subtract = ((byte >> 6) & 0x01) != 0;
        let half_carry = ((byte >> 5) & 0x01) != 0;
        let carry = ((byte >> 4) & 0x01) != 0;

        return FlagsRegister {
            zero,
            subtract,
            half_carry,
            carry,
        };
    }
}

impl Registers {

    pub fn new() -> Self {
        Registers {
            a: 0x00,
            b: 0x00,
            c: 0x00,
            d: 0x00,
            e: 0x00,
            f: FlagsRegister {
                zero: false,
                subtract: false,
                half_carry: false,
                carry: false,
            },
            h: 0x00,
            l: 0x00,
        }
    }

    pub fn get_af(&self) -> u16 {
        return (self.a as u16) << 8 | (u8::from(self.f) as u16);
    }

    pub fn get_bc(&self) -> u16 {
        return (self.b as u16) << 8 | (self.c as u16);
    }

    pub fn get_de(&self) -> u16 {
        return (self.d as u16) << 8 | (self.e as u16);
    }

    pub fn get_hl(&self) -> u16 {
        return (self.h as u16) << 8 | (self.l as u16);
    }

    pub fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = FlagsRegister::from((value & 0xFF) as u8);
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    pub fn set_de(&mut self, value: u16) {
        self.d = ((value & 0xFF00) >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    pub fn set_hl(&mut self, value: u16) {
        self.h = ((value & 0xFF00) >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }
}
