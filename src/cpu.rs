

struct Registers {
    a: u8, 
    b: u8, 
    c: u8, 
    d: u8, 
    e: u8, 
    f: u8, 
    h: u8, 
    l: u8
}

struct FlagsRegister {
    zero: bool
    subtract: bool
    half_carry: bool
    carry: bool
}

impl std::convert::From<FlagsRegister> for u8 {
    fn from(flag: FlagsRegister) -> u8{
        (if flag.zero {1} else {0}) << 7 |
        (if flag.subtract {1} else {0}) << 6 | 
        (if flag.half_carry {1} else {0}) | 
        (if flag.carry {1} else {0}) |
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
            carry
        }
    }
}

impl Registers {

    fn get_af(&self) -> u16 {
        return (self.a as u16) << 8 | (self.f as u16)
    }

    fn get_bc(&self) -> u16 {
        return (self.b as u16) << 8 | (self.c as u16);
    }

    fn get_de(&self) -> u16 {
        return (self.d as u16 ) << 8 | (self.e as u16);
    }

    fn get_hl(&self) -> u16 {
        return (self.h as u16) << 8 | (self.l as u16);
    }

    fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = (value & 0xFF) as u8;
    }

    fn set_bc(&mut self, value: u16) {
        self.b = ((value & 0xFF00) >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    fn set_de(&mut self, value: u16) {
        self.d = ((value & 0xFF00) >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    fn set_hl(&mut self, value: u16) {
        self.h = ((value & 0xFF00) >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }
}