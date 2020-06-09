pub mod gpu;
pub mod interrupt;
pub mod memory;
pub mod keys;

pub struct CPU {
    pub regs: Registers,
    pub pc: u16,
    pub sp: u16,
    pub bus: MemoryBus,
    pub is_halted: bool,
    pub ime: bool,
}

pub struct MemoryBus {
    pub memory: memory::MMU,
    pub gpu: gpu::GPU,
    pub keys: keys::Keys
}

#[derive(Copy, Clone)]
pub struct FlagsRegister {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
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
    DI(),
    EI(),
    DAA(),
    SCF(),
    PUSH(StackTarget),
    POP(StackTarget),
    CALL(JumpTest),
    RET(JumpTest),
    RETI(),
    RLCA(),
    RRCA(),
    RLA(),
    RRA(),
}

enum RSTTargets {
    H00,
    H10,
    H20,
    H30,
    H08,
    H18,
    H28,
    H38,
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

impl MemoryBus {

    fn read_byte(&self, address: u16) -> u8 {
        let mut address = address as usize;
        
        match address {

            /* ROM Bank 0 */
            0x0000..=0x3FFF => {
                self.memory.rom[address as usize]
            }

            /* Additional ROM Banks */
            0x4000..=0x7FFF => {
                self.memory.rom[address as usize]
            }

            /* Read from VRAM */
            gpu::VRAM_BEGIN..=gpu::VRAM_END => {
                self.gpu.read_vram(address - gpu::VRAM_BEGIN)
            }

            /* Read from External Cartridge RAM */
            memory::ERAM_BEGIN..=memory::ERAM_END => {
                self.memory.eram[address - memory::ERAM_BEGIN]
            }

            /* Read from Work RAM */
            memory::WRAM_BEGIN..=memory::WRAM_END => {
                if address >= 0xE000 {
                    address -= 0x2000;
                    self.memory.wram[address - memory::WRAM_BEGIN]
                } else {
                    self.memory.wram[address - memory::WRAM_BEGIN]
                }
            }

            /* Read from Sprite Attribute Table */
            gpu::OAM_BEGIN..=gpu::OAM_END => {
                self.gpu.oam[address - gpu::OAM_BEGIN]
            }

            /* Read from High RAM */
            memory::ZRAM_BEGIN..=memory::ZRAM_END => {
                self.memory.zram[address - memory::ZRAM_BEGIN]
            }

            /* Not usable memory */
            0xFEA0..=0xFEFF => {
                panic!("Not usable memory. Something went wrong!")
            }

            /* Joypad Input */
            0xFF00 => self.keys.read_key(),

            _ => panic!("Unable to process 'address' in read_byte()")
        }
    }

    fn read_byte_2(&self, address: u16) -> u8 {
        let address = address as usize;
        match address & 0xF000 {
            0x0000 => self.memory.rom[address as usize],
            0x1000 => self.memory.rom[address as usize],
            0x2000 => self.memory.rom[address as usize],
            0x3000 => self.memory.rom[address as usize],
            0x4000 => self.memory.rom[address as usize],
            0x5000 => self.memory.rom[address as usize],
            0x6000 => self.memory.rom[address as usize],
            0x7000 => self.memory.rom[address as usize],
            0x8000 | 0x9000 => self.gpu.read_vram((address & 0x1FFF) - gpu::VRAM_BEGIN),
            0xA000 | 0xB000 => self.memory.eram[address & 0x1FFF],
            0xC000 | 0xD000 | 0xE000 => self.memory.wram[address & 0x1FFF],
            0xF000 => match address & 0xF00 {
                0x000 | 0x100 | 0x200 | 0x300 | 0x400 | 0x500 | 0x600 | 0x700 | 0x800 | 0x900
                | 0xA00 | 0xB00 | 0xC00 | 0xD00 => self.memory.wram[address & 0x1FFF],
                0xE00 => {
                    if address < 0xFEA0 {
                        self.gpu.oam[address & 0xFF]
                    } else {
                        0
                    }
                }
                0xF00 => {
                    if address == 0xFFFF {
                        self.memory.interrupt_enable
                    } else if address >= 0xFF80 {
                        self.memory.zram[(address & 0x7F) - memory::ZRAM_BEGIN]
                    } else {
                        match address & 0xF0 {
                            0x00 => {
                                if address == 0xFF0F {
                                    self.memory.interrupt_flag
                                } else {
                                    0
                                }
                            }
                            0x40 => self.gpu.read_vram(address - gpu::VRAM_BEGIN),
                            0x50 => self.gpu.read_vram(address - gpu::VRAM_BEGIN),
                            0x60 => self.gpu.read_vram(address - gpu::VRAM_BEGIN),
                            0x70 => self.gpu.read_vram(address - gpu::VRAM_BEGIN),
                            _ => panic!("Unable to process 'address in read_byte()'"),
                        }
                    }
                }
                _ => panic!("Unable to process 'address' in read_byte()"),
            },
            _ => panic!("Unable to process 'address' in read_byte()"),
        }
    }

    fn read_word(&self, address: u16) -> u16 {
        let lower = self.read_byte(address) as u16;
        let higher = self.read_byte(address + 1) as u16;
        (higher << 8) | lower
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        let address = address as usize;
        match address {
            /* Write to VRAM */
            gpu::VRAM_BEGIN..=gpu::VRAM_END => {
                self.gpu.write_vram(address - gpu::VRAM_BEGIN, value);
            }
            /* Write to WRAM or Echo RAM*/
            memory::WRAM_BEGIN..=memory::WRAM_END => {
                self.memory.wram[address - memory::WRAM_BEGIN] = value;
            }
            /* Write to External RAM */
            memory::ERAM_BEGIN..=memory::ERAM_END => {
                self.memory.eram[address - memory::ERAM_BEGIN] = value;
            }

            /* Write to High RAM */
            memory::ZRAM_BEGIN..=memory::ZRAM_END => {
                self.memory.zram[address - memory::ZRAM_BEGIN] = value;
            }

            /* Write to Sprite Attribute Table (OAM) */
            gpu::OAM_BEGIN..=gpu::OAM_END => {
                self.gpu.oam[address - gpu::OAM_BEGIN] = value;
            }

            /* Not usable memory */
            0xFEA0..=0xFEFF => {
                panic!("Trying to write to invalid memory. Something went wrong!")
            }

            /* Write to Joypad Register */
            0xFF00 => {
                self.keys.write_key(value);
            }

            /* Write to I/0 Registers */
            0xFF0F => {
                self.memory.interrupt_flag = value;
            }

            /* Write to Interrupts Enable Register */
            0xFFFF => {
                self.memory.interrupt_enable = value;
            }

            /* Write to GPU registers */
            gpu::GPU_REGS_BEGIN..=gpu::GPU_REGS_END => {
                self.gpu.write_registers(address, value);
            }

            _ => println!(
                "Write byte called for address: 0x{:X}. Value: {:X}",
                address, value
            ),
        }
    }

    pub fn write_word(&mut self, address: u16, word: u16) {
        let lower = word >> 8;
        let higher = word & 0xFF;
        self.write_byte(address, lower as u8);
        self.write_byte(address, higher as u8);
    }
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
            0x00 => Some(Instructions::NOP()),
            0x01 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::BC,
                LoadWordSource::D16,
            ))),
            0x02 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::BC,
                LoadByteSource::A,
            ))),
            0x03 => Some(Instructions::INC(IncDecTarget::BC)),
            0x04 => Some(Instructions::INC(IncDecTarget::B)),
            0x05 => Some(Instructions::DEC(IncDecTarget::B)),
            0x06 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::D8,
            ))),
            0x07 => Some(Instructions::RLCA()),
            0x08 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::A16,
                LoadWordSource::SP,
            ))),
            0x09 => Some(Instructions::ADD16(Arithmetic16Target::BC)),
            0x0A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::BC,
            ))),
            0x0B => Some(Instructions::DEC(IncDecTarget::BC)),
            0x0C => Some(Instructions::INC(IncDecTarget::C)),
            0x0D => Some(Instructions::DEC(IncDecTarget::C)),
            0x0E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::D8,
            ))),
            0x0F => Some(Instructions::RRCA()),
            0x11 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::DE,
                LoadWordSource::D16,
            ))),
            0x12 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::DE,
                LoadByteSource::A,
            ))),
            0x13 => Some(Instructions::INC(IncDecTarget::DE)),
            0x14 => Some(Instructions::INC(IncDecTarget::D)),
            0x15 => Some(Instructions::DEC(IncDecTarget::D)),
            0x16 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::D8,
            ))),
            0x17 => Some(Instructions::RLA()),
            0x18 => Some(Instructions::JR(JumpTest::Always)),
            0x19 => Some(Instructions::ADD16(Arithmetic16Target::DE)),
            0x1A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::DE,
            ))),
            0x1B => Some(Instructions::DEC(IncDecTarget::DE)),
            0x1C => Some(Instructions::INC(IncDecTarget::E)),
            0x1D => Some(Instructions::DEC(IncDecTarget::E)),
            0x1E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::D8,
            ))),
            0x1F => Some(Instructions::RRA()),
            0x20 => Some(Instructions::JR(JumpTest::NotZero)),
            0x21 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::HL,
                LoadWordSource::D16,
            ))),
            0x22 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HLI,
                LoadByteSource::A,
            ))),
            0x23 => Some(Instructions::INC(IncDecTarget::HL)),
            0x24 => Some(Instructions::INC(IncDecTarget::H)),
            0x25 => Some(Instructions::DEC(IncDecTarget::H)),
            0x26 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::D8,
            ))),
            0x27 => Some(Instructions::DAA()),
            0x28 => Some(Instructions::JR(JumpTest::Zero)),
            0x29 => Some(Instructions::ADD16(Arithmetic16Target::HL)),
            0x2A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::HLI,
            ))),
            0x2B => Some(Instructions::DEC(IncDecTarget::HL)),
            0x2C => Some(Instructions::INC(IncDecTarget::L)),
            0x2D => Some(Instructions::DEC(IncDecTarget::L)),
            0x2E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::D8,
            ))),
            0x2F => Some(Instructions::CPL()),
            0x30 => Some(Instructions::JR(JumpTest::NotCarry)),
            0x31 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::SP,
                LoadWordSource::D16,
            ))),
            0x32 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HLD,
                LoadByteSource::A,
            ))),
            0x33 => Some(Instructions::INC(IncDecTarget::SP)),
            0x34 => Some(Instructions::INC(IncDecTarget::HLAddr)),
            0x35 => Some(Instructions::DEC(IncDecTarget::HLAddr)),
            0x36 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::D8,
            ))),
            0x37 => Some(Instructions::SCF()),
            0x38 => Some(Instructions::JR(JumpTest::Carry)),
            0x39 => Some(Instructions::ADD16(Arithmetic16Target::SP)),
            0x3A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::HLD,
            ))),
            0x3B => Some(Instructions::DEC(IncDecTarget::SP)),
            0x3C => Some(Instructions::INC(IncDecTarget::A)),
            0x3D => Some(Instructions::DEC(IncDecTarget::A)),
            0x3E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::D8,
            ))),
            0x3F => Some(Instructions::CCF()),
            0x40 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::B,
            ))),
            0x41 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::C,
            ))),
            0x42 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::D,
            ))),
            0x43 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::E,
            ))),
            0x44 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::H,
            ))),
            0x45 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::L,
            ))),
            0x46 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::HL,
            ))),
            0x47 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::B,
                LoadByteSource::A,
            ))),
            0x48 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::B,
            ))),
            0x49 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::C,
            ))),
            0x4A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::D,
            ))),
            0x4B => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::E,
            ))),
            0x4C => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::H,
            ))),
            0x4D => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::L,
            ))),
            0x4E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::HL,
            ))),
            0x4F => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::C,
                LoadByteSource::A,
            ))),
            0x50 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::B,
            ))),
            0x51 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::C,
            ))),
            0x52 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::D,
            ))),
            0x53 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::E,
            ))),
            0x54 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::H,
            ))),
            0x55 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::L,
            ))),
            0x56 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::HL,
            ))),
            0x57 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::D,
                LoadByteSource::A,
            ))),
            0x58 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::B,
            ))),
            0x59 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::C,
            ))),
            0x5A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::D,
            ))),
            0x5B => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::E,
            ))),
            0x5C => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::H,
            ))),
            0x5D => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::L,
            ))),
            0x5E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::HL,
            ))),
            0x5F => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::E,
                LoadByteSource::A,
            ))),
            0x60 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::B,
            ))),
            0x61 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::C,
            ))),
            0x62 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::D,
            ))),
            0x63 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::E,
            ))),
            0x64 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::H,
            ))),
            0x65 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::L,
            ))),
            0x66 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::HL,
            ))),
            0x67 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::H,
                LoadByteSource::A,
            ))),
            0x68 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::B,
            ))),
            0x69 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::C,
            ))),
            0x6A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::D,
            ))),
            0x6B => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::E,
            ))),
            0x6C => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::H,
            ))),
            0x6D => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::L,
            ))),
            0x6E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::HL,
            ))),
            0x6F => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::L,
                LoadByteSource::A,
            ))),
            0x70 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::B,
            ))),
            0x71 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::C,
            ))),
            0x72 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::D,
            ))),
            0x73 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::E,
            ))),
            0x74 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::H,
            ))),
            0x75 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::L,
            ))),
            0x76 => Some(Instructions::HALT()),
            0x77 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::HL,
                LoadByteSource::A,
            ))),
            0x78 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::B,
            ))),
            0x79 => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::C,
            ))),
            0x7A => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::D,
            ))),
            0x7B => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::E,
            ))),
            0x7C => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::H,
            ))),
            0x7D => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::L,
            ))),
            0x7E => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::HL,
            ))),
            0x7F => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::A,
            ))),
            0x80 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x81 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::C)),
            0x82 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::D)),
            0x83 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::E)),
            0x84 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::H)),
            0x85 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::L)),
            0x86 => Some(Instructions::ADD(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0x87 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::A)),
            0x88 => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::B)),
            0x89 => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::C)),
            0x8A => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::D)),
            0x8B => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::E)),
            0x8C => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::H)),
            0x8D => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::L)),
            0x8E => Some(Instructions::ADC(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0x8F => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::A)),
            0x90 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::B)),
            0x91 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::C)),
            0x92 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::D)),
            0x93 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::E)),
            0x94 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::H)),
            0x95 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::L)),
            0x96 => Some(Instructions::SUB(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0x97 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::A)),
            0x98 => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::B)),
            0x99 => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::C)),
            0x9A => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::D)),
            0x9B => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::E)),
            0x9C => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::H)),
            0x9D => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::L)),
            0x9E => Some(Instructions::SBC(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0x9F => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::A)),
            0xA0 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::B)),
            0xA1 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::C)),
            0xA2 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::D)),
            0xA3 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::E)),
            0xA4 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::H)),
            0xA5 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::L)),
            0xA6 => Some(Instructions::AND(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0xA7 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::A)),
            0xA8 => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::B)),
            0xA9 => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::C)),
            0xAA => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::D)),
            0xAB => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::E)),
            0xAC => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::H)),
            0xAD => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::L)),
            0xAE => Some(Instructions::XOR(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0xAF => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::A)),
            0xB0 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::B)),
            0xB1 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::C)),
            0xB2 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::D)),
            0xB3 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::E)),
            0xB4 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::H)),
            0xB5 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::L)),
            0xB6 => Some(Instructions::OR(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0xB7 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::A)),
            0xB8 => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::B)),
            0xB9 => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::C)),
            0xBA => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::D)),
            0xBB => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::E)),
            0xBC => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::H)),
            0xBD => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::L)),
            0xBE => Some(Instructions::CP(
                ArithmeticTarget::A,
                ArithmeticSource::HLAddr,
            )),
            0xBF => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::A)),
            0xC0 => Some(Instructions::RET(JumpTest::NotZero)),
            0xC1 => Some(Instructions::POP(StackTarget::BC)),
            0xC2 => Some(Instructions::JP(JumpTest::NotZero)),
            0xC3 => Some(Instructions::JP(JumpTest::Always)),
            0xC4 => Some(Instructions::CALL(JumpTest::NotZero)),
            0xC5 => Some(Instructions::PUSH(StackTarget::BC)),
            0xC6 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xC7 => Some(Instructions::RST(RSTTargets::H00)),
            0xC8 => Some(Instructions::RET(JumpTest::Zero)),
            0xC9 => Some(Instructions::RET(JumpTest::Always)),
            0xCA => Some(Instructions::JP(JumpTest::Zero)),
            0xCB =>
            /* CB Instructions */
            {
                None
            }
            0xCC => Some(Instructions::CALL(JumpTest::Zero)),
            0xCD => Some(Instructions::CALL(JumpTest::Always)),
            0xCE => Some(Instructions::ADC(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xCF => Some(Instructions::RST(RSTTargets::H08)),
            0xD0 => Some(Instructions::RET(JumpTest::NotCarry)),
            0xD1 => Some(Instructions::POP(StackTarget::DE)),
            0xD2 => Some(Instructions::JP(JumpTest::NotCarry)),
            0xD4 => Some(Instructions::CALL(JumpTest::NotCarry)),
            0xD5 => Some(Instructions::PUSH(StackTarget::DE)),
            0xD6 => Some(Instructions::SUB(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xD7 => Some(Instructions::RST(RSTTargets::H10)),
            0xD8 => Some(Instructions::RET(JumpTest::Carry)),
            0xD9 => Some(Instructions::RETI()),
            0xDA => Some(Instructions::JP(JumpTest::Carry)),
            0xDC => Some(Instructions::CALL(JumpTest::Carry)),
            0xDE => Some(Instructions::SBC(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xDF => Some(Instructions::RST(RSTTargets::H18)),
            0xE0 => Some(Instructions::LDH(LoadType::Other(
                LoadOtherTarget::A8,
                LoadOtherSource::A,
            ))),
            0xE1 => Some(Instructions::POP(StackTarget::HL)),
            0xE2 => Some(Instructions::LD(LoadType::Other(
                LoadOtherTarget::CAddress,
                LoadOtherSource::A,
            ))),
            0xE5 => Some(Instructions::PUSH(StackTarget::HL)),
            0xE6 => Some(Instructions::AND(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xE7 => Some(Instructions::RST(RSTTargets::H20)),
            0xE8 => Some(Instructions::ADD(
                ArithmeticTarget::SP,
                ArithmeticSource::I8,
            )),
            0xE9 => Some(Instructions::JPHL()),
            0xEA => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A16,
                LoadByteSource::A,
            ))),
            0xEE => Some(Instructions::XOR(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xEF => Some(Instructions::RST(RSTTargets::H28)),
            0xF0 => Some(Instructions::LDH(LoadType::Other(
                LoadOtherTarget::A,
                LoadOtherSource::A8,
            ))),
            0xF1 => Some(Instructions::POP(StackTarget::AF)),
            0xF2 => Some(Instructions::LD(LoadType::Other(
                LoadOtherTarget::A,
                LoadOtherSource::CAddress,
            ))),
            0xF3 => Some(Instructions::DI()),
            0xF5 => Some(Instructions::PUSH(StackTarget::AF)),
            0xF6 => Some(Instructions::OR(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xF7 => Some(Instructions::RST(RSTTargets::H30)),
            0xF8 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::HL,
                LoadWordSource::SPr8,
            ))),
            0xF9 => Some(Instructions::LD(LoadType::Word(
                LoadWordTarget::SP,
                LoadWordSource::HL,
            ))),
            0xFA => Some(Instructions::LD(LoadType::Byte(
                LoadByteTarget::A,
                LoadByteSource::A16,
            ))),
            0xFB => Some(Instructions::EI()),
            0xFE => Some(Instructions::CP(ArithmeticTarget::A, ArithmeticSource::U8)),
            0xFF => Some(Instructions::RST(RSTTargets::H38)),
            _ => None,
        }
    }
}

impl CPU {
    pub fn emulate_cycle(&mut self) {
        let mut instruction = self.bus.read_byte(self.pc);
        let prefixed = instruction == 0xCB;
        if prefixed {
            instruction = self.bus.read_byte(self.pc + 1);
        }

        let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
        println!("Current Opcode: {}", description);

        let next = if let Some(instruction) = Instructions::from_byte(instruction, prefixed) {
            self.decode_instruction(instruction)
        } else {
            let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
            panic!("Unknown instruction found! Opcode: {}", description);
        };

        self.pc = next;

        if self.ime
            && (self.bus.memory.interrupt_enable != 0)
            && (self.bus.memory.interrupt_flag != 0)
        {
            let fired = self.bus.memory.interrupt_enable & self.bus.memory.interrupt_flag;
            if fired & 0x01 != 0 {
                self.bus.memory.interrupt_flag &= 0xFE;
                self.rst40();
            }
        }
    }

    fn rst40(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x40;
    }

    fn decode_instruction(&mut self, instruction: Instructions) -> u16 {
        if self.is_halted {
            return self.pc;
        }

        match instruction {
            Instructions::DAA() => {
                let mut s = self.regs.a as u16;

                if self.regs.f.subtract {
                    if self.regs.f.half_carry {
                        s = (s - 0x6) & 0xFF;
                    }

                    if self.regs.f.carry {
                        s -= 0x60;
                    }
                } else {
                    if self.regs.f.half_carry || (s & 0xF) > 9 {
                        s += 0x06;
                    }

                    if self.regs.f.carry || (s > 0x9F) {
                        s += 0x60;
                    }
                }

                self.regs.a = s as u8;
                self.regs.f.subtract = false;
                self.regs.f.zero = self.regs.a == 0;
                if s >= 0x100 {
                    self.regs.f.carry = true;
                }

                self.pc.wrapping_add(1)
            }

            Instructions::RETI() => {
                self.ime = true;
                self.sp += 2;
                self.bus.read_word(self.sp - 2)
            }

            Instructions::DI() => {
                self.ime = false;
                self.pc.wrapping_add(1)
            }

            Instructions::EI() => {
                self.ime = true;
                self.pc.wrapping_add(1)
            }

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
                    JumpTest::Always => true,
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
                    JumpTest::Always => true,
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
                    StackTarget::HL => self.regs.set_hl(result),
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
                        /* blah */
                        let a_reg = self.regs.a as i16;
                        let source_value = source_value as i16;
                        self.regs.f.zero = a_reg == source_value;
                        self.regs.f.subtract = true;
                        self.regs.f.half_carry = ((a_reg - source_value) & 0xF) > (a_reg & 0xF);
                        self.regs.f.carry = (a_reg - source_value) < 0;
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
                            "This should not occur. i8 is not a valid source for SUB operation!"
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
        let new_value = self.regs.a as i16 - value as i16;
        self.regs.f.carry = new_value < 0;
        self.regs.f.zero = new_value == 0;
        self.regs.f.half_carry = (new_value as u8 & 0xF) > (self.regs.a & 0xF);
        self.regs.f.subtract = true;
        new_value as u8
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
            let byte = self.bus.read_byte(self.pc + 1) as i8;
            ((self.pc as u32 as i32) + (byte as i32)) as u16
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
        let lower = self.bus.read_byte(self.pc + 1) as u16;
        let higher = self.bus.read_byte(self.pc + 2) as u16;
        (higher << 8) | lower
    }

    fn read_next_byte(&self) -> u8 {
        self.bus.read_byte(self.pc + 1)
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
