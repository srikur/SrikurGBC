struct CPU {
    regs: Registers,
    pc: u16,
    sp: u16,
    bus: MemoryBus,
    is_halted: bool,
}

struct MemoryBus {
    memory: [u8; 0xFFFF],
}

enum LoadByteTarget {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    HLI,
    HLD,
    HL,
    BC,
    DE,
    A16,
}
enum LoadByteSource {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    D8,
    HL,
    HLI,
    HLD,
    A16,
    BC,
    DE,
}

enum LoadWordSource {
    D16,
    HL,
    SP,
    SPr8,
}

enum LoadWordTarget {
    BC,
    DE,
    HL,
    SP,
    A16,
}

enum LoadOtherTarget {
    A,
    A8,
    CAddress,
}

enum LoadOtherSource {
    A,
    A8,
    CAddress,
}

enum StackTarget {
    BC,
    DE,
    HL,
    AF,
}

enum LoadType {
    Byte(LoadByteTarget, LoadByteSource),
    Word(LoadWordTarget, LoadWordSource),
    Other(LoadOtherTarget, LoadOtherSource),
}

enum Instructions {
    NOP(),
    HALT(),
    LD(LoadType),
    LDH(LoadType),
    INC(IncDecTarget),
    DEC(IncDecTarget),
    ADD(ArithmeticTarget, ArithmeticSource),
    ADD16(Arithmetic16Target),
    SUB(ArithmeticTarget, ArithmeticSource),
    JR(JumpTest),
    JP(JumpTest),
    ADC(ArithmeticTarget, ArithmeticSource),
    SBC(ArithmeticTarget, ArithmeticSource),
    AND(ArithmeticTarget, ArithmeticSource), 
    XOR(ArithmeticTarget, ArithmeticSource),
    OR(ArithmeticTarget, ArithmeticSource), 
    CP(ArithmeticTarget, ArithmeticSource), 
    RST(RSTTargets),
    CPL(),
    JPHL(),
    CCF(),
    SCF(),
    PUSH(StackTarget),
    POP(StackTarget),
    CALL(JumpTest),
    RET(JumpTest),
    RLCA(),
    RRCA(),
    RLA(),
    RRA(),
}

enum RSTTargets {
    H00, H10, H20, H30, H08, H18, H28, H38
}

enum IncDecTarget {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    HL,
    HLAddr,
    BC,
    DE,
    SP,
}

enum Arithmetic16Target {
    HL,
    BC,
    DE,
    SP,
}

enum ArithmeticTarget {
    A,
    SP,
}

enum ArithmeticSource {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    U8,
    HLAddr,
    I8,
}

enum JumpTest {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    Always,
}

struct Registers {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: FlagsRegister,
    h: u8,
    l: u8,
}

#[derive(Copy, Clone)]
struct FlagsRegister {
    zero: bool,
    subtract: bool,
    half_carry: bool,
    carry: bool,
}

impl MemoryBus {
    fn read_byte(&self, address: u16) -> u8 {
        return self.memory[address as usize];
    }

    fn read_word(&self, address: u16) -> u16 {
        let lower = self.memory[address as usize] as u16;
        let higher = self.memory[(address + 1) as usize] as u16;
        (higher << 8) | lower
    }

    fn write_byte(&self, address: u16, byte: u8) {}
    fn write_word(&self, address: u16, word: u16) {}
}

impl Instructions {
    fn from_byte(byte: u8, prefixed: bool) -> Option<Instructions> {
        if prefixed {
            Instructions::from_byte_prefixed(byte)
        } else {
            Instructions::from_byte_not_prefixed(byte)
        }
    }

    fn from_byte_prefixed(byte: u8) -> Option<Instructions> {
        match byte {
            _ => None,
        }
    }

    fn from_byte_not_prefixed(byte: u8) -> Option<Instructions> {
        match byte {
            0x80 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x81 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x82 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x83 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x84 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x85 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x87 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            _ => None,
        }
    }
}

impl CPU {
    fn emulate_cycle(&mut self) {
        let mut instruction = self.bus.read_byte(self.pc);
        let prefixed = instruction == 0xCB;
        if prefixed {
            instruction = self.bus.read_byte(self.pc + 1);
        }
        let next = if let Some(instruction) = Instructions::from_byte(instruction, prefixed) {
            self.decode_instruction(instruction)
        } else {
            let description = format!("0x{}{:x}", if prefixed { "CB" } else { "" }, instruction);

            panic!("Unknown instruction found! Opcode: {}", description);
        };

        self.pc = next;
    }

    fn decode_instruction(&mut self, instruction: Instructions) -> u16 {
        if self.is_halted {
            return self.pc;
        }

        match instruction {
            Instructions::HALT() => self.pc,

            Instructions::RST(target) => {
                let location = match target {
                    RSTTargets::H00 => 0x00, 
                    RSTTargets::H08 => 0x08, 
                    RSTTargets::H10 => 0x10, 
                    RSTTargets::H18 => 0x18, 
                    RSTTargets::H20 => 0x20, 
                    RSTTargets::H28 => 0x28, 
                    RSTTargets::H30 => 0x30, 
                    RSTTargets::H38 => 0x38, 
                };

                self.sp -= 2;
                self.bus.write_word(self.sp, self.pc + 1);
                location
            }

            Instructions::CALL(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.regs.f.zero,
                    JumpTest::Zero => self.regs.f.zero, 
                    JumpTest::Carry => self.regs.f.carry, 
                    JumpTest::NotCarry => !self.regs.f.carry, 
                    JumpTest::Always => true
                };
                self.call(jump_condition)
            }

            Instructions::RLCA() => {
                self.regs.f.carry = self.regs.a & 0x80 == 0x80;
                self.regs.a = (self.regs.a << 1) | !!(self.regs.a & 0x80);
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.half_carry = false;
                self.regs.f.subtract = false;
                self.pc.wrapping_add(1)
            }

            Instructions::RLA() => {
                self.regs.f.carry = self.regs.a & 0x80 == 0x80;
                self.regs.a = (self.regs.a << 1) | !!(u8::from(self.regs.f) & 0x10);
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.half_carry = false;
                self.regs.f.subtract = false;
                self.pc.wrapping_add(1)
            }

            Instructions::CCF() => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = !self.regs.f.carry;
                self.pc.wrapping_add(1)
            }

            Instructions::CPL() => {
                self.regs.f.half_carry = true;
                self.regs.f.subtract = true;
                self.regs.a = !self.regs.a;
                self.pc.wrapping_add(1)
            }

            Instructions::SCF() => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = true;
                self.pc.wrapping_add(1)
            }

            Instructions::RRCA() => {
                self.regs.f.carry = self.regs.a & 0x01 == 0x01;
                self.regs.a = (self.regs.a >> 1) | ((self.regs.a & 0x01) << 7);
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.pc.wrapping_add(1)
            }

            Instructions::RRA() => {
                self.regs.f.carry = self.regs.a & 0x01 == 0x01;
                self.regs.a = (self.regs.a >> 1) | ((!!(u8::from(self.regs.f) & 0x10) & 0x01) << 7);
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.pc.wrapping_add(1)
            }

            Instructions::RET(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.regs.f.zero,
                    JumpTest::Zero => self.regs.f.zero, 
                    JumpTest::Carry => self.regs.f.carry, 
                    JumpTest::NotCarry => !self.regs.f.carry, 
                    JumpTest::Always => true
                };
                self.return_(jump_condition)
            }

            Instructions::JR(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.regs.f.zero,
                    JumpTest::NotCarry => !self.regs.f.carry,
                    JumpTest::Zero => self.regs.f.zero,
                    JumpTest::Carry => self.regs.f.carry,
                    JumpTest::Always => true,
                };

                self.jump_relative(jump_condition)
            }

            Instructions::JP(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.regs.f.zero,
                    JumpTest::NotCarry => !self.regs.f.carry,
                    JumpTest::Zero => self.regs.f.zero,
                    JumpTest::Carry => self.regs.f.carry,
                    JumpTest::Always => true,
                };

                self.jump(jump_condition)
            }

            Instructions::JPHL() => self.regs.get_hl(),

            Instructions::NOP() => {
                return self.pc.wrapping_add(1);
            }

            Instructions::DEC(target) => {
                let new_value: u8;

                match target {
                    IncDecTarget::A => {
                        let reg = self.regs.a;
                        new_value = self.dec(&reg);
                        self.regs.a = new_value;
                    }
                    IncDecTarget::B => {
                        let reg = self.regs.b;
                        new_value = self.dec(&reg);
                        self.regs.b = new_value;
                    }
                    IncDecTarget::C => {
                        let reg = self.regs.c;
                        new_value = self.dec(&reg);
                        self.regs.c = new_value;
                    }
                    IncDecTarget::D => {
                        let reg = self.regs.d;
                        new_value = self.dec(&reg);
                        self.regs.d = new_value;
                    }
                    IncDecTarget::E => {
                        let reg = self.regs.e;
                        new_value = self.dec(&reg);
                        self.regs.e = new_value;
                    }
                    IncDecTarget::H => {
                        let reg = self.regs.h;
                        new_value = self.dec(&reg);
                        self.regs.h = new_value;
                    }
                    IncDecTarget::L => {
                        let reg = self.regs.l;
                        new_value = self.dec(&reg);
                        self.regs.l = new_value;
                    }
                    IncDecTarget::HLAddr => {
                        let byte = self.bus.read_byte(self.regs.get_hl());
                        new_value = self.dec(&byte);
                        self.bus.write_byte(self.regs.get_hl(), new_value);
                    }
                    IncDecTarget::HL => {
                        self.regs.set_hl(self.regs.get_hl() - 1);
                    }
                    IncDecTarget::BC => {
                        self.regs.set_hl(self.regs.get_hl() - 1);
                    }
                    IncDecTarget::DE => {
                        self.regs.set_hl(self.regs.get_hl() - 1);
                    }
                    IncDecTarget::SP => {
                        self.regs.set_hl(self.regs.get_hl() - 1);
                    }
                }

                self.pc.wrapping_sub(1)
            }

            Instructions::INC(target) => {
                let new_value: u8;

                match target {
                    IncDecTarget::A => {
                        let reg = self.regs.a;
                        new_value = self.inc(&reg);
                        self.regs.a = new_value;
                    }
                    IncDecTarget::B => {
                        let reg = self.regs.b;
                        new_value = self.inc(&reg);
                        self.regs.b = new_value;
                    }
                    IncDecTarget::C => {
                        let reg = self.regs.c;
                        new_value = self.inc(&reg);
                        self.regs.c = new_value;
                    }
                    IncDecTarget::D => {
                        let reg = self.regs.d;
                        new_value = self.inc(&reg);
                        self.regs.d = new_value;
                    }
                    IncDecTarget::E => {
                        let reg = self.regs.e;
                        new_value = self.inc(&reg);
                        self.regs.e = new_value;
                    }
                    IncDecTarget::H => {
                        let reg = self.regs.h;
                        new_value = self.inc(&reg);
                        self.regs.h = new_value;
                    }
                    IncDecTarget::L => {
                        let reg = self.regs.l;
                        new_value = self.inc(&reg);
                        self.regs.l = new_value;
                    }
                    IncDecTarget::HLAddr => {
                        let byte = self.bus.read_byte(self.regs.get_hl());
                        new_value = self.inc(&byte);
                        self.bus.write_byte(self.regs.get_hl(), new_value);
                    }
                    IncDecTarget::HL => {
                        self.regs.set_hl(self.regs.get_hl() + 1);
                    }
                    IncDecTarget::BC => {
                        self.regs.set_hl(self.regs.get_hl() + 1);
                    }
                    IncDecTarget::DE => {
                        self.regs.set_hl(self.regs.get_hl() + 1);
                    }
                    IncDecTarget::SP => {
                        self.regs.set_hl(self.regs.get_hl() + 1);
                    }
                }

                self.pc.wrapping_add(1)
            }

            Instructions::LDH(load_type) => match load_type {
                LoadType::Other(target, source) => {
                    let source_value = match source {
                        LoadOtherSource::A => self.regs.a,
                        LoadOtherSource::A8 => self.read_next_byte(),
                        LoadOtherSource::CAddress => self.regs.c,
                    };

                    match source {
                        LoadOtherSource::A8 => {
                            self.pc = self.pc.wrapping_add(1);
                        }
                        _ => {}
                    }

                    match target {
                        LoadOtherTarget::A => {
                            self.regs.a = self.bus.read_byte(0xFF00 + source_value as u16)
                        }
                        LoadOtherTarget::A8 => {
                            self.bus
                                .write_byte(0xFF00 + self.read_next_byte() as u16, source_value);
                            self.pc = self.pc.wrapping_add(1);
                        }
                        LoadOtherTarget::CAddress => {
                            self.bus.write_byte(source_value as u16, self.regs.a);
                        }
                    }

                    self.pc.wrapping_add(1)
                }

                _ => panic!("Unimplemented!"),
            },

            Instructions::LD(load_type) => match load_type {
                LoadType::Word(target, source) => {
                    let source_value = match source {
                        LoadWordSource::D16 => self.read_next_word(),
                        LoadWordSource::SP => self.sp,
                        LoadWordSource::HL => self.regs.get_hl(),
                        LoadWordSource::SPr8 => self.sp + self.read_next_byte() as u16,
                    };

                    match target {
                        LoadWordTarget::BC => {
                            self.regs.set_bc(source_value);
                        }
                        LoadWordTarget::DE => {
                            self.regs.set_de(source_value);
                        }
                        LoadWordTarget::SP => {
                            self.sp = source_value;
                        }
                        LoadWordTarget::HL => {
                            self.regs.set_hl(source_value);
                        }
                        LoadWordTarget::A16 => {
                            self.bus.write_word(self.read_next_word(), source_value);
                        }
                    }

                    match source {
                        LoadWordSource::HL => self.pc.wrapping_add(1),
                        LoadWordSource::SPr8 => self.pc.wrapping_add(2),
                        LoadWordSource::D16 | LoadWordSource::SP => self.pc.wrapping_add(3),
                    }
                }

                LoadType::Byte(target, source) => {
                    let source_value = match source {
                        LoadByteSource::A => self.regs.a,
                        LoadByteSource::B => self.regs.b,
                        LoadByteSource::C => self.regs.c,
                        LoadByteSource::D => self.regs.d,
                        LoadByteSource::D8 => self.read_next_byte(),
                        LoadByteSource::E => self.regs.e,
                        LoadByteSource::H => self.regs.h,
                        LoadByteSource::L => self.regs.l,
                        LoadByteSource::HL => self.bus.read_byte(self.regs.get_hl()),
                        LoadByteSource::BC => self.bus.read_byte(self.regs.get_bc()),
                        LoadByteSource::DE => self.bus.read_byte(self.regs.get_de()),
                        LoadByteSource::HLI => {
                            self.regs.set_hl(self.regs.get_hl() + 1);
                            self.bus.read_byte(self.regs.get_hl() - 1)
                        }
                        LoadByteSource::HLD => {
                            self.regs.set_hl(self.regs.get_hl() - 1);
                            self.bus.read_byte(self.regs.get_hl() + 1)
                        }
                        LoadByteSource::A16 => self.bus.read_byte(self.read_next_word()),
                    };

                    match target {
                        LoadByteTarget::A => self.regs.a = source_value,
                        LoadByteTarget::B => self.regs.b = source_value,
                        LoadByteTarget::C => self.regs.c = source_value,
                        LoadByteTarget::D => self.regs.d = source_value,
                        LoadByteTarget::E => self.regs.e = source_value,
                        LoadByteTarget::H => self.regs.h = source_value,
                        LoadByteTarget::L => self.regs.l = source_value,
                        LoadByteTarget::HL => {
                            self.bus.write_byte(self.regs.get_hl(), source_value);
                        }
                        LoadByteTarget::HLI => {
                            self.bus.write_byte(self.regs.get_hl(), source_value);
                            self.regs.set_hl(self.regs.get_hl() + 1);
                        }
                        LoadByteTarget::HLD => {
                            self.bus.write_byte(self.regs.get_hl(), source_value);
                            self.regs.set_hl(self.regs.get_hl() - 1);
                        }
                        LoadByteTarget::BC => {
                            self.bus.write_byte(self.regs.get_bc(), source_value);
                        }
                        LoadByteTarget::DE => {
                            self.bus.write_byte(self.regs.get_de(), source_value);
                        }
                        LoadByteTarget::A16 => {
                            self.bus.write_byte(self.read_next_word(), source_value);
                            return self.pc.wrapping_add(3);
                        }
                    }

                    match source {
                        LoadByteSource::D8 => self.pc.wrapping_add(2),
                        LoadByteSource::A16 => self.pc.wrapping_add(3),
                        _ => self.pc.wrapping_add(1),
                    }
                }

                _ => panic!("Not implemented!"),
            },

            Instructions::PUSH(target) => {
                let value = match target {
                    StackTarget::BC => self.regs.get_bc(),
                    StackTarget::DE => self.regs.get_de(),
                    StackTarget::HL => self.regs.get_hl(),
                    StackTarget::AF => self.regs.get_af(),
                };

                self.push(value);
                return self.pc.wrapping_add(1);
            }

            Instructions::POP(target) => {
                let result = self.pop();
                match target {
                    StackTarget::BC => self.regs.set_bc(result),
                    StackTarget::AF => self.regs.set_af(result), 
                    StackTarget::DE => self.regs.set_de(result), 
                    StackTarget::HL => self.regs.set_hl(result)
                };

                return self.pc.wrapping_add(1);
            }

            Instructions::CP(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for CP operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        self.regs.f.zero = self.regs.a == source_value;
                        self.regs.f.subtract = true;
                        self.regs.f.half_carry = ((self.regs.a - source_value) & 0xF) > (self.regs.a & 0xF);
                        self.regs.f.carry = (self.regs.a as i16 - source_value as i16) < 0;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::OR(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        self.regs.a |= source_value;
                        self.regs.f.zero = self.regs.a == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = false;
                        self.regs.f.carry = false;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::XOR(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        self.regs.a ^= source_value;
                        self.regs.f.zero = self.regs.a == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = false;
                        self.regs.f.carry = false;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::AND(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        self.regs.a &= source_value;
                        self.regs.f.zero = self.regs.a == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = true;
                        self.regs.f.carry = false;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::SUB(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        let new_value = self.sub(source_value);
                        self.regs.a = new_value;
                    }
                    _ => panic!("This should not occur!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::SBC(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation!"
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        let difference: i16 =
                            self.regs.a as i16 - source_value as i16 - u8::from(self.regs.f) as i16;
                        self.regs.f.carry = difference < 0x0;
                        self.regs.f.subtract = true;
                        self.regs.f.half_carry = ((self.regs.a as i16 & 0xF)
                            - (self.regs.a as i16 & 0xF)
                            - u8::from(self.regs.f) as i16)
                            < 0x0;
                        self.regs.a = self.regs.a - self.regs.h - u8::from(self.regs.f);
                        self.regs.f.zero = self.regs.a == 0;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::ADC(target, source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => {
                        panic!(
                            "This should not occur. i8 is not a valid source for SUB operation! "
                        );
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        let sum: u16 =
                            self.regs.a as u16 + source_value as u16 + u8::from(self.regs.f) as u16;
                        self.regs.f.carry = sum >= 0x100;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = ((self.regs.a as u16 & 0xF)
                            + (source_value as u16 & 0xF)
                            + u8::from(self.regs.f) as u16)
                            >= 0x10;
                        self.regs.a = self.regs.a + self.regs.h + u8::from(self.regs.f);
                        self.regs.f.zero = self.regs.a == 0;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => self.pc.wrapping_add(2),
                    _ => self.pc.wrapping_add(1),
                }
            }

            Instructions::ADD16(source) => {
                let source_value = match source {
                    Arithmetic16Target::BC => self.regs.get_bc(),
                    Arithmetic16Target::DE => self.regs.get_de(),
                    Arithmetic16Target::HL => self.regs.get_hl(),
                    Arithmetic16Target::SP => self.sp,
                };

                let (sum, did_overflow) = self.regs.get_hl().overflowing_add(source_value);
                self.regs.f.subtract = false;
                self.regs.f.half_carry = (sum & 0xFFF) < (self.regs.get_hl() & 0xFFF);
                self.regs.f.carry = did_overflow;
                self.regs.set_hl(sum);
                self.pc.wrapping_add(1)
            }

            Instructions::ADD(target, source) => {
                match source {
                    ArithmeticSource::I8 => {
                        let source_value = self.read_next_byte();
                        let sp_value = self.sp;
                        let (new_value, _) = (sp_value as i32).overflowing_add(source_value as i32);
                        self.regs.f.zero = new_value == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.carry = new_value > 0xFFFF;
                        self.regs.f.half_carry =
                            (self.sp & 0xF) + (source_value as u16 & 0xF) > 0xF;
                        self.sp = new_value as u16;
                        return self.pc.wrapping_add(2);
                    }
                    _ => { /* Keep going */ }
                }

                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    ArithmeticSource::U8 => self.read_next_byte(),
                    _ => 0,
                };

                match target {
                    ArithmeticTarget::A => {
                        /* ADD A, r8 */
                        let new_value = self.add(source_value);
                        self.regs.a = new_value;
                        return self.pc.wrapping_add(1);
                    }

                    _ => 0,
                }
            }
        }
    }

    fn inc(&mut self, register: &u8) -> u8 {
        let new_value = *register + 1;
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = *register & 0xF == 0xF;
        new_value
    }

    fn dec(&mut self, register: &u8) -> u8 {
        let new_value = *register - 1;
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = true;
        self.regs.f.half_carry = new_value & 0xF == 0xF;
        new_value
    }

    fn sub(&mut self, value: u8) -> u8 {
        let new_value = self.regs.a - value;
        self.regs.f.carry = new_value < 0;
        self.regs.f.zero = new_value == 0;
        self.regs.f.half_carry = (new_value & 0xF) > (self.regs.a & 0xF);
        self.regs.f.subtract = true;
        new_value
    }

    fn add(&mut self, value: u8) -> u8 {
        let (new_value, did_overflow) = self.regs.a.overflowing_add(value);
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = false;
        self.regs.f.carry = did_overflow;
        self.regs.f.half_carry = (self.regs.a & 0xF) + (value & 0xF) > 0xF;
        new_value
    }

    fn jump(&self, should_jump: bool) -> u16 {
        if should_jump {
            let lower_byte = self.bus.read_byte(self.pc + 1) as u16;
            let higher_byte = self.bus.read_byte(self.pc + 2) as u16;
            (higher_byte << 8) | lower_byte
        } else {
            self.pc.wrapping_add(3)
        }
    }

    fn jump_relative(&self, should_jump: bool) -> u16 {
        if should_jump {
            self.bus.read_byte(self.pc + 1) as u16
        } else {
            self.pc.wrapping_add(2)
        }
    }

    fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, ((value & 0xFF00) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, (value & 0xFF) as u8);
    }

    fn pop(&mut self) -> u16 {
        let lsb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let msb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);

        return (msb << 8) | lsb;
    }

    fn call(&mut self, should_jump: bool) -> u16 {
        let next_pc = self.pc.wrapping_add(3);

        if should_jump {
            self.push(next_pc);
            self.read_next_word()
        } else {
            return next_pc;
        }
    }
    fn return_(&mut self, should_jump: bool) -> u16 {
        if should_jump {
            self.pop()
        } else {
            self.pc.wrapping_add(1)
        }
    }

    fn read_next_word(&self) -> u16 {
        let first_half = self.bus.memory[(self.pc + 1) as usize] as u16;
        let second_half = self.bus.memory[(self.pc + 2) as usize] as u16;
        (second_half << 8) | first_half
    }

    fn read_next_byte(&self) -> u8 {
        self.bus.memory[(self.pc + 1) as usize]
    }
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
    fn get_af(&self) -> u16 {
        return (self.a as u16) << 8 | (u8::from(self.f) as u16);
    }

    fn get_bc(&self) -> u16 {
        return (self.b as u16) << 8 | (self.c as u16);
    }

    fn get_de(&self) -> u16 {
        return (self.d as u16) << 8 | (self.e as u16);
    }

    fn get_hl(&self) -> u16 {
        return (self.h as u16) << 8 | (self.l as u16);
    }

    fn set_af(&mut self, value: u16) {
        self.a = ((value & 0xFF00) >> 8) as u8;
        self.f = FlagsRegister::from((value & 0xFF) as u8);
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
