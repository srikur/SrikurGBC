pub mod audio;
pub mod gpu;
pub mod keys;
pub mod memory;
pub mod timer;

use std::fs;

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
    pub keys: keys::Keys,
    pub timer: timer::Timer,
    pub apu: audio::APU,
    pub bootrom_run: bool,
    pub bootrom: Vec<u8>,
}

#[derive(Copy, Clone)]
pub struct FlagsRegister {
    pub zero: bool,
    pub subtract: bool,
    pub half_carry: bool,
    pub carry: bool,
}

#[derive(PartialEq)]
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

#[derive(PartialEq)]
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

#[derive(PartialEq)]
enum LoadOtherTarget {
    A,
    A8,
    CAddress,
}

#[derive(PartialEq)]
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
    RRC(ArithmeticSource),
    RLA(),
    RRA(),

    /* CB Instructions */
    RLC(ArithmeticSource),
    RL(ArithmeticSource),
    RR(ArithmeticSource),
    SLA(ArithmeticSource),
    SRA(ArithmeticSource),
    SWAP(ArithmeticSource),
    SRL(ArithmeticSource),
    BIT(u8, ArithmeticSource),
    RES(u8, ArithmeticSource),
    SET(u8, ArithmeticSource),
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

#[derive(Eq, PartialEq)]
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

#[derive(PartialEq)]
enum JumpTest {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    Always,
}

pub enum Interrupts {
    VBlank,
    LCDStat,
    Timer,
    //Serial, still need to implement transfer cable functionality
    Joypad,
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
            /* ROM Banks */
            0x0000..=0x7FFF => {
                if !self.bootrom_run && (address <= 0xFF) {
                    self.bootrom[address]
                } else {
                    address = usize::from(
                        address + (self.memory.current_rom_bank as usize * 0x4000)
                            - memory::SWITCHABLE_BANK_BEGIN,
                    );
                    self.memory.game_rom[address]
                }
            }

            /* Read from RAM Bank */
            memory::ERAM_BEGIN..=memory::ERAM_END => {
                address = address - memory::ERAM_BEGIN;
                self.memory.ram_banks[address + (self.memory.current_ram_bank as usize * 0x2000)]
            }

            /* Read from VRAM */
            gpu::VRAM_BEGIN..=gpu::VRAM_END => self.gpu.read_vram(address - gpu::VRAM_BEGIN),

            /* Read from Work RAM */
            memory::WRAM_BEGIN..=memory::WRAM_END => self.memory.wram[address - memory::WRAM_BEGIN],
            0xE000..=0xFDFF => {
                address -= 0x2000;
                self.memory.wram[address - memory::WRAM_BEGIN]
            }

            /* Read from Sprite Attribute Table */
            gpu::OAM_BEGIN..=gpu::OAM_END => self.gpu.oam[address - gpu::OAM_BEGIN],

            /* GPU Registers */
            gpu::GPU_REGS_BEGIN..=gpu::GPU_REGS_END => self.gpu.read_registers(address),

            /* Read from High RAM */
            memory::ZRAM_BEGIN..=memory::ZRAM_END => self.memory.zram[address - memory::ZRAM_BEGIN],

            /* Not usable memory */
            0xFEA0..=0xFEFF => panic!("Not usable memory. Something went wrong!"),

            /* Joypad Input */
            keys::JOYPAD_INPUT => self.keys.get_joypad_state(),

            /* Interrupt Enable 0xFFFF */
            memory::INTERRUPT_ENABLE => self.memory.interrupt_enable,

            /* Interrupt Flag 0xFF0F */
            memory::INTERRUPT_FLAG => self.memory.interrupt_flag,

            /* DIV - Divider Register */
            timer::DIVIDER_REGISTER => self.timer.divider_register as u8,

            /* TIMA - Timer Counter */
            timer::TIMA => self.timer.timer_counter_tima as u8,

            /* TAC - Timer Control */
            timer::TMC => panic!("Return TAC"),

            /* Audio Controls */
            audio::SOUND_BEGIN..=audio::SOUND_END => self.apu.read_byte(address),

            /* Extra space */
            gpu::EXTRA_SPACE_BEGIN..=gpu::EXTRA_SPACE_END => {
                self.gpu.extra[address - gpu::EXTRA_SPACE_BEGIN]
            }

            _ => panic!("Unable to process {:X} in read_byte()", address),
        }
    }

    fn handle_banking(&mut self, address: usize, value: u8) {
        let mut value = value;

        match address {
            0x0000..=0x1FFF => {
                if self.memory.mbc1 || self.memory.mbc2 {
                    if self.memory.mbc2 && ((address & 0x10) == 1) {
                        return;
                    }

                    match value & 0xF {
                        0xA => self.memory.ram_enabled = true,
                        0x0 => self.memory.ram_enabled = false,
                        _ => self.memory.ram_enabled = false,
                    }
                }
            }

            0x2000..=0x3FFF => {
                if self.memory.mbc1 || self.memory.mbc2 {
                    if self.memory.mbc2 {
                        if value & 0xF == 0 {
                            self.memory.current_rom_bank += 1;
                        }
                        return;
                    }

                    self.memory.current_rom_bank &= 0xE0;
                    self.memory.current_rom_bank |= value & 0x1F;
                    if self.memory.current_rom_bank == 0 {
                        self.memory.current_rom_bank += 1;
                    }
                }
            }

            0x4000..=0x5FFF => {
                if self.memory.mbc1 {
                    if !self.memory.cartridge_type != 0 {
                        self.memory.current_rom_bank &= 0x1F;
                        value &= 0xE0;
                        self.memory.current_rom_bank |= value;

                        if self.memory.current_rom_bank == 0 {
                            self.memory.current_rom_bank += 1;
                        }
                    } else {
                        self.memory.current_ram_bank = value & 0x03;
                    }
                }
            }

            0x6000..=0x7FFF => {
                if self.memory.mbc1 {
                    if (value & 0x01) == 0 {
                        self.memory.current_ram_bank = 0;
                    }
                }
            }

            _ => panic!("Error"),
        }
    }

    fn read_word(&self, address: u16) -> u16 {
        let lower = self.read_byte(address) as u16;
        let higher = self.read_byte(address + 1) as u16;
        (higher << 8) | lower
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        //println!("Write byte called with value {:X}", value);

        let address = address as usize;
        match address {
            /* Handle Banking */
            0x0000..=0x7FFF => {
                self.handle_banking(address, value);
            }

            memory::ERAM_BEGIN..=memory::ERAM_END => {
                if self.memory.ram_enabled {
                    let new_address = address - 0xA000;
                    self.memory.ram_banks
                        [new_address + (self.memory.current_ram_bank as usize * 0x2000)] = value;
                }
            }

            /* Write to VRAM */
            gpu::VRAM_BEGIN..=gpu::VRAM_END => {
                self.gpu.write_vram(address - gpu::VRAM_BEGIN, value);
            }

            /* Write to WRAM */
            memory::WRAM_BEGIN..=memory::WRAM_END => {
                self.memory.wram[address - memory::WRAM_BEGIN] = value;
            }

            /* Write to Echo RAM */
            0xE000..=0xFDFF => {
                self.memory.wram[address - memory::WRAM_BEGIN - 0x2000] = value;
            }

            /* Write to I/0 Registers */
            memory::INTERRUPT_FLAG => {
                self.memory.interrupt_flag = value;
            }

            timer::DIVIDER_REGISTER => {
                self.timer.divider_register = 0;
            }

            timer::TIMA => {
                self.timer.timer_counter_tima = value as u32;
            }

            timer::TMA => {
                self.timer.timer_modulo_tma = value as u32;
            }

            timer::TMC => {
                /* Timer Control */
                self.timer.clock_enabled = (value & 0x04) != 0;
                let new_speed: u16 = match value & 0x03 {
                    0 => 1024,
                    1 => 16,
                    2 => 64,
                    3 => 256,
                    _ => 1024,
                };

                if new_speed != self.timer.input_clock_speed {
                    self.timer.input_clock_speed = new_speed;
                }
            }

            /* Audio Controls */
            audio::SOUND_BEGIN..=audio::SOUND_END => self.apu.write_byte(address, value),

            gpu::EXTRA_SPACE_BEGIN..=gpu::EXTRA_SPACE_END => {
                self.gpu.extra[address - gpu::EXTRA_SPACE_BEGIN] = value;
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
            0xFEA0..=0xFEFF => return, // Invalid memory location

            /* Not usable as well */
            0xFF4C..=0xFF7F => return,

            /* Write to Joypad Register */
            0xFF00 => self.keys.joypad_state = value,

            /* Write to Interrupts Enable Register */
            memory::INTERRUPT_ENABLE => {
                self.memory.interrupt_enable = value;
            }

            /* Write to GPU registers */
            gpu::GPU_REGS_BEGIN..=gpu::GPU_REGS_END => {
                if address == 0xFF46 {
                    /* DMA Transfer */
                    let value = (value as u16) << 8;
                    for i in 0..=0x9F {
                        self.gpu.oam[i] = self.read_byte(value + i as u16);
                    }
                    return;
                }

                self.gpu.write_registers(address, value);
            }

            _ => println!(
                "Write byte not implemented for address: 0x{:X}. Value: {:X}",
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
            0x00 => Some(Instructions::RLC(ArithmeticSource::B)),
            0x01 => Some(Instructions::RLC(ArithmeticSource::C)),
            0x02 => Some(Instructions::RLC(ArithmeticSource::D)),
            0x03 => Some(Instructions::RLC(ArithmeticSource::E)),
            0x04 => Some(Instructions::RLC(ArithmeticSource::H)),
            0x05 => Some(Instructions::RLC(ArithmeticSource::L)),
            0x06 => Some(Instructions::RLC(ArithmeticSource::HLAddr)),
            0x07 => Some(Instructions::RLC(ArithmeticSource::A)),
            0x08 => Some(Instructions::RRC(ArithmeticSource::B)),
            0x09 => Some(Instructions::RRC(ArithmeticSource::C)),
            0x0A => Some(Instructions::RRC(ArithmeticSource::D)),
            0x0B => Some(Instructions::RRC(ArithmeticSource::E)),
            0x0C => Some(Instructions::RRC(ArithmeticSource::H)),
            0x0D => Some(Instructions::RRC(ArithmeticSource::L)),
            0x0E => Some(Instructions::RRC(ArithmeticSource::HLAddr)),
            0x0F => Some(Instructions::RRC(ArithmeticSource::A)),
            0x10 => Some(Instructions::RL(ArithmeticSource::B)),
            0x11 => Some(Instructions::RL(ArithmeticSource::C)),
            0x12 => Some(Instructions::RL(ArithmeticSource::D)),
            0x13 => Some(Instructions::RL(ArithmeticSource::E)),
            0x14 => Some(Instructions::RL(ArithmeticSource::H)),
            0x15 => Some(Instructions::RL(ArithmeticSource::L)),
            0x16 => Some(Instructions::RL(ArithmeticSource::HLAddr)),
            0x17 => Some(Instructions::RL(ArithmeticSource::A)),
            0x18 => Some(Instructions::RR(ArithmeticSource::B)),
            0x19 => Some(Instructions::RR(ArithmeticSource::C)),
            0x1A => Some(Instructions::RR(ArithmeticSource::D)),
            0x1B => Some(Instructions::RR(ArithmeticSource::E)),
            0x1C => Some(Instructions::RR(ArithmeticSource::H)),
            0x1D => Some(Instructions::RR(ArithmeticSource::L)),
            0x1E => Some(Instructions::RR(ArithmeticSource::HLAddr)),
            0x1F => Some(Instructions::RR(ArithmeticSource::A)),
            0x20 => Some(Instructions::SLA(ArithmeticSource::B)),
            0x21 => Some(Instructions::SLA(ArithmeticSource::C)),
            0x22 => Some(Instructions::SLA(ArithmeticSource::D)),
            0x23 => Some(Instructions::SLA(ArithmeticSource::E)),
            0x24 => Some(Instructions::SLA(ArithmeticSource::H)),
            0x25 => Some(Instructions::SLA(ArithmeticSource::L)),
            0x26 => Some(Instructions::SLA(ArithmeticSource::HLAddr)),
            0x27 => Some(Instructions::SLA(ArithmeticSource::A)),
            0x28 => Some(Instructions::SRA(ArithmeticSource::B)),
            0x29 => Some(Instructions::SRA(ArithmeticSource::C)),
            0x2A => Some(Instructions::SRA(ArithmeticSource::D)),
            0x2B => Some(Instructions::SRA(ArithmeticSource::E)),
            0x2C => Some(Instructions::SRA(ArithmeticSource::H)),
            0x2D => Some(Instructions::SRA(ArithmeticSource::L)),
            0x2E => Some(Instructions::SRA(ArithmeticSource::HLAddr)),
            0x2F => Some(Instructions::SRA(ArithmeticSource::A)),
            0x30 => Some(Instructions::SWAP(ArithmeticSource::B)),
            0x31 => Some(Instructions::SWAP(ArithmeticSource::C)),
            0x32 => Some(Instructions::SWAP(ArithmeticSource::D)),
            0x33 => Some(Instructions::SWAP(ArithmeticSource::E)),
            0x34 => Some(Instructions::SWAP(ArithmeticSource::H)),
            0x35 => Some(Instructions::SWAP(ArithmeticSource::L)),
            0x36 => Some(Instructions::SWAP(ArithmeticSource::HLAddr)),
            0x37 => Some(Instructions::SWAP(ArithmeticSource::A)),
            0x38 => Some(Instructions::SRL(ArithmeticSource::B)),
            0x39 => Some(Instructions::SRL(ArithmeticSource::C)),
            0x3A => Some(Instructions::SRL(ArithmeticSource::D)),
            0x3B => Some(Instructions::SRL(ArithmeticSource::E)),
            0x3C => Some(Instructions::SRL(ArithmeticSource::H)),
            0x3D => Some(Instructions::SRL(ArithmeticSource::L)),
            0x3E => Some(Instructions::SRL(ArithmeticSource::HLAddr)),
            0x3F => Some(Instructions::SRL(ArithmeticSource::A)),
            0x40 => Some(Instructions::BIT(0, ArithmeticSource::B)),
            0x41 => Some(Instructions::BIT(0, ArithmeticSource::C)),
            0x42 => Some(Instructions::BIT(0, ArithmeticSource::D)),
            0x43 => Some(Instructions::BIT(0, ArithmeticSource::E)),
            0x44 => Some(Instructions::BIT(0, ArithmeticSource::H)),
            0x45 => Some(Instructions::BIT(0, ArithmeticSource::L)),
            0x46 => Some(Instructions::BIT(0, ArithmeticSource::HLAddr)),
            0x47 => Some(Instructions::BIT(0, ArithmeticSource::A)),
            0x48 => Some(Instructions::BIT(1, ArithmeticSource::B)),
            0x49 => Some(Instructions::BIT(1, ArithmeticSource::C)),
            0x4A => Some(Instructions::BIT(1, ArithmeticSource::D)),
            0x4B => Some(Instructions::BIT(1, ArithmeticSource::E)),
            0x4C => Some(Instructions::BIT(1, ArithmeticSource::H)),
            0x4D => Some(Instructions::BIT(1, ArithmeticSource::L)),
            0x4E => Some(Instructions::BIT(1, ArithmeticSource::HLAddr)),
            0x4F => Some(Instructions::BIT(1, ArithmeticSource::A)),
            0x50 => Some(Instructions::BIT(2, ArithmeticSource::B)),
            0x51 => Some(Instructions::BIT(2, ArithmeticSource::C)),
            0x52 => Some(Instructions::BIT(2, ArithmeticSource::D)),
            0x53 => Some(Instructions::BIT(2, ArithmeticSource::E)),
            0x54 => Some(Instructions::BIT(2, ArithmeticSource::H)),
            0x55 => Some(Instructions::BIT(2, ArithmeticSource::L)),
            0x56 => Some(Instructions::BIT(2, ArithmeticSource::HLAddr)),
            0x57 => Some(Instructions::BIT(2, ArithmeticSource::A)),
            0x58 => Some(Instructions::BIT(3, ArithmeticSource::B)),
            0x59 => Some(Instructions::BIT(3, ArithmeticSource::C)),
            0x5A => Some(Instructions::BIT(3, ArithmeticSource::D)),
            0x5B => Some(Instructions::BIT(3, ArithmeticSource::E)),
            0x5C => Some(Instructions::BIT(3, ArithmeticSource::H)),
            0x5D => Some(Instructions::BIT(3, ArithmeticSource::L)),
            0x5E => Some(Instructions::BIT(3, ArithmeticSource::HLAddr)),
            0x5F => Some(Instructions::BIT(3, ArithmeticSource::A)),
            0x60 => Some(Instructions::BIT(4, ArithmeticSource::B)),
            0x61 => Some(Instructions::BIT(4, ArithmeticSource::C)),
            0x62 => Some(Instructions::BIT(4, ArithmeticSource::D)),
            0x63 => Some(Instructions::BIT(4, ArithmeticSource::E)),
            0x64 => Some(Instructions::BIT(4, ArithmeticSource::H)),
            0x65 => Some(Instructions::BIT(4, ArithmeticSource::L)),
            0x66 => Some(Instructions::BIT(4, ArithmeticSource::HLAddr)),
            0x67 => Some(Instructions::BIT(4, ArithmeticSource::A)),
            0x68 => Some(Instructions::BIT(5, ArithmeticSource::B)),
            0x69 => Some(Instructions::BIT(5, ArithmeticSource::C)),
            0x6A => Some(Instructions::BIT(5, ArithmeticSource::D)),
            0x6B => Some(Instructions::BIT(5, ArithmeticSource::E)),
            0x6C => Some(Instructions::BIT(5, ArithmeticSource::H)),
            0x6D => Some(Instructions::BIT(5, ArithmeticSource::L)),
            0x6E => Some(Instructions::BIT(5, ArithmeticSource::HLAddr)),
            0x6F => Some(Instructions::BIT(5, ArithmeticSource::A)),
            0x70 => Some(Instructions::BIT(6, ArithmeticSource::B)),
            0x71 => Some(Instructions::BIT(6, ArithmeticSource::C)),
            0x72 => Some(Instructions::BIT(6, ArithmeticSource::D)),
            0x73 => Some(Instructions::BIT(6, ArithmeticSource::E)),
            0x74 => Some(Instructions::BIT(6, ArithmeticSource::H)),
            0x75 => Some(Instructions::BIT(6, ArithmeticSource::L)),
            0x76 => Some(Instructions::BIT(6, ArithmeticSource::HLAddr)),
            0x77 => Some(Instructions::BIT(6, ArithmeticSource::A)),
            0x78 => Some(Instructions::BIT(7, ArithmeticSource::B)),
            0x79 => Some(Instructions::BIT(7, ArithmeticSource::C)),
            0x7A => Some(Instructions::BIT(7, ArithmeticSource::D)),
            0x7B => Some(Instructions::BIT(7, ArithmeticSource::E)),
            0x7C => Some(Instructions::BIT(7, ArithmeticSource::H)),
            0x7D => Some(Instructions::BIT(7, ArithmeticSource::L)),
            0x7E => Some(Instructions::BIT(7, ArithmeticSource::HLAddr)),
            0x7F => Some(Instructions::BIT(7, ArithmeticSource::A)),
            0x80 => Some(Instructions::RES(0, ArithmeticSource::B)),
            0x81 => Some(Instructions::RES(0, ArithmeticSource::C)),
            0x82 => Some(Instructions::RES(0, ArithmeticSource::D)),
            0x83 => Some(Instructions::RES(0, ArithmeticSource::E)),
            0x84 => Some(Instructions::RES(0, ArithmeticSource::H)),
            0x85 => Some(Instructions::RES(0, ArithmeticSource::L)),
            0x86 => Some(Instructions::RES(0, ArithmeticSource::HLAddr)),
            0x87 => Some(Instructions::RES(0, ArithmeticSource::A)),
            0x88 => Some(Instructions::RES(1, ArithmeticSource::B)),
            0x89 => Some(Instructions::RES(1, ArithmeticSource::C)),
            0x8A => Some(Instructions::RES(1, ArithmeticSource::D)),
            0x8B => Some(Instructions::RES(1, ArithmeticSource::E)),
            0x8C => Some(Instructions::RES(1, ArithmeticSource::H)),
            0x8D => Some(Instructions::RES(1, ArithmeticSource::L)),
            0x8E => Some(Instructions::RES(1, ArithmeticSource::HLAddr)),
            0x8F => Some(Instructions::RES(1, ArithmeticSource::A)),
            0x90 => Some(Instructions::RES(2, ArithmeticSource::B)),
            0x91 => Some(Instructions::RES(2, ArithmeticSource::C)),
            0x92 => Some(Instructions::RES(2, ArithmeticSource::D)),
            0x93 => Some(Instructions::RES(2, ArithmeticSource::E)),
            0x94 => Some(Instructions::RES(2, ArithmeticSource::H)),
            0x95 => Some(Instructions::RES(2, ArithmeticSource::L)),
            0x96 => Some(Instructions::RES(2, ArithmeticSource::HLAddr)),
            0x97 => Some(Instructions::RES(2, ArithmeticSource::A)),
            0x98 => Some(Instructions::RES(3, ArithmeticSource::B)),
            0x99 => Some(Instructions::RES(3, ArithmeticSource::C)),
            0x9A => Some(Instructions::RES(3, ArithmeticSource::D)),
            0x9B => Some(Instructions::RES(3, ArithmeticSource::E)),
            0x9C => Some(Instructions::RES(3, ArithmeticSource::H)),
            0x9D => Some(Instructions::RES(3, ArithmeticSource::L)),
            0x9E => Some(Instructions::RES(3, ArithmeticSource::HLAddr)),
            0x9F => Some(Instructions::RES(3, ArithmeticSource::A)),
            0xA0 => Some(Instructions::RES(4, ArithmeticSource::B)),
            0xA1 => Some(Instructions::RES(4, ArithmeticSource::C)),
            0xA2 => Some(Instructions::RES(4, ArithmeticSource::D)),
            0xA3 => Some(Instructions::RES(4, ArithmeticSource::E)),
            0xA4 => Some(Instructions::RES(4, ArithmeticSource::H)),
            0xA5 => Some(Instructions::RES(4, ArithmeticSource::L)),
            0xA6 => Some(Instructions::RES(4, ArithmeticSource::HLAddr)),
            0xA7 => Some(Instructions::RES(4, ArithmeticSource::A)),
            0xA8 => Some(Instructions::RES(5, ArithmeticSource::B)),
            0xA9 => Some(Instructions::RES(5, ArithmeticSource::C)),
            0xAA => Some(Instructions::RES(5, ArithmeticSource::D)),
            0xAB => Some(Instructions::RES(5, ArithmeticSource::E)),
            0xAC => Some(Instructions::RES(5, ArithmeticSource::H)),
            0xAD => Some(Instructions::RES(5, ArithmeticSource::L)),
            0xAE => Some(Instructions::RES(5, ArithmeticSource::HLAddr)),
            0xAF => Some(Instructions::RES(5, ArithmeticSource::A)),
            0xB0 => Some(Instructions::RES(6, ArithmeticSource::B)),
            0xB1 => Some(Instructions::RES(6, ArithmeticSource::C)),
            0xB2 => Some(Instructions::RES(6, ArithmeticSource::D)),
            0xB3 => Some(Instructions::RES(6, ArithmeticSource::E)),
            0xB4 => Some(Instructions::RES(6, ArithmeticSource::H)),
            0xB5 => Some(Instructions::RES(6, ArithmeticSource::L)),
            0xB6 => Some(Instructions::RES(6, ArithmeticSource::HLAddr)),
            0xB7 => Some(Instructions::RES(6, ArithmeticSource::A)),
            0xB8 => Some(Instructions::RES(7, ArithmeticSource::B)),
            0xB9 => Some(Instructions::RES(7, ArithmeticSource::C)),
            0xBA => Some(Instructions::RES(7, ArithmeticSource::D)),
            0xBB => Some(Instructions::RES(7, ArithmeticSource::E)),
            0xBC => Some(Instructions::RES(7, ArithmeticSource::H)),
            0xBD => Some(Instructions::RES(7, ArithmeticSource::L)),
            0xBE => Some(Instructions::RES(7, ArithmeticSource::HLAddr)),
            0xBF => Some(Instructions::RES(7, ArithmeticSource::A)),
            0xC0 => Some(Instructions::SET(0, ArithmeticSource::B)),
            0xC1 => Some(Instructions::SET(0, ArithmeticSource::C)),
            0xC2 => Some(Instructions::SET(0, ArithmeticSource::D)),
            0xC3 => Some(Instructions::SET(0, ArithmeticSource::E)),
            0xC4 => Some(Instructions::SET(0, ArithmeticSource::H)),
            0xC5 => Some(Instructions::SET(0, ArithmeticSource::L)),
            0xC6 => Some(Instructions::SET(0, ArithmeticSource::HLAddr)),
            0xC7 => Some(Instructions::SET(0, ArithmeticSource::A)),
            0xC8 => Some(Instructions::SET(1, ArithmeticSource::B)),
            0xC9 => Some(Instructions::SET(1, ArithmeticSource::C)),
            0xCA => Some(Instructions::SET(1, ArithmeticSource::D)),
            0xCB => Some(Instructions::SET(1, ArithmeticSource::E)),
            0xCC => Some(Instructions::SET(1, ArithmeticSource::H)),
            0xCD => Some(Instructions::SET(1, ArithmeticSource::L)),
            0xCE => Some(Instructions::SET(1, ArithmeticSource::HLAddr)),
            0xCF => Some(Instructions::SET(1, ArithmeticSource::A)),
            0xD0 => Some(Instructions::SET(2, ArithmeticSource::B)),
            0xD1 => Some(Instructions::SET(2, ArithmeticSource::C)),
            0xD2 => Some(Instructions::SET(2, ArithmeticSource::D)),
            0xD3 => Some(Instructions::SET(2, ArithmeticSource::E)),
            0xD4 => Some(Instructions::SET(2, ArithmeticSource::H)),
            0xD5 => Some(Instructions::SET(2, ArithmeticSource::L)),
            0xD6 => Some(Instructions::SET(2, ArithmeticSource::HLAddr)),
            0xD7 => Some(Instructions::SET(2, ArithmeticSource::A)),
            0xD8 => Some(Instructions::SET(3, ArithmeticSource::B)),
            0xD9 => Some(Instructions::SET(3, ArithmeticSource::C)),
            0xDA => Some(Instructions::SET(3, ArithmeticSource::D)),
            0xDB => Some(Instructions::SET(3, ArithmeticSource::E)),
            0xDC => Some(Instructions::SET(3, ArithmeticSource::H)),
            0xDD => Some(Instructions::SET(3, ArithmeticSource::L)),
            0xDE => Some(Instructions::SET(3, ArithmeticSource::HLAddr)),
            0xDF => Some(Instructions::SET(3, ArithmeticSource::A)),
            0xE0 => Some(Instructions::SET(4, ArithmeticSource::B)),
            0xE1 => Some(Instructions::SET(4, ArithmeticSource::C)),
            0xE2 => Some(Instructions::SET(4, ArithmeticSource::D)),
            0xE3 => Some(Instructions::SET(4, ArithmeticSource::E)),
            0xE4 => Some(Instructions::SET(4, ArithmeticSource::H)),
            0xE5 => Some(Instructions::SET(4, ArithmeticSource::L)),
            0xE6 => Some(Instructions::SET(4, ArithmeticSource::HLAddr)),
            0xE7 => Some(Instructions::SET(4, ArithmeticSource::A)),
            0xE8 => Some(Instructions::SET(5, ArithmeticSource::B)),
            0xE9 => Some(Instructions::SET(5, ArithmeticSource::C)),
            0xEA => Some(Instructions::SET(5, ArithmeticSource::D)),
            0xEB => Some(Instructions::SET(5, ArithmeticSource::E)),
            0xEC => Some(Instructions::SET(5, ArithmeticSource::H)),
            0xED => Some(Instructions::SET(5, ArithmeticSource::L)),
            0xEE => Some(Instructions::SET(5, ArithmeticSource::HLAddr)),
            0xEF => Some(Instructions::SET(5, ArithmeticSource::A)),
            0xF0 => Some(Instructions::SET(6, ArithmeticSource::B)),
            0xF1 => Some(Instructions::SET(6, ArithmeticSource::C)),
            0xF2 => Some(Instructions::SET(6, ArithmeticSource::D)),
            0xF3 => Some(Instructions::SET(6, ArithmeticSource::E)),
            0xF4 => Some(Instructions::SET(6, ArithmeticSource::H)),
            0xF5 => Some(Instructions::SET(6, ArithmeticSource::L)),
            0xF6 => Some(Instructions::SET(6, ArithmeticSource::HLAddr)),
            0xF7 => Some(Instructions::SET(6, ArithmeticSource::A)),
            0xF8 => Some(Instructions::SET(7, ArithmeticSource::B)),
            0xF9 => Some(Instructions::SET(7, ArithmeticSource::C)),
            0xFA => Some(Instructions::SET(7, ArithmeticSource::D)),
            0xFB => Some(Instructions::SET(7, ArithmeticSource::E)),
            0xFC => Some(Instructions::SET(7, ArithmeticSource::H)),
            0xFD => Some(Instructions::SET(7, ArithmeticSource::L)),
            0xFE => Some(Instructions::SET(7, ArithmeticSource::HLAddr)),
            0xFF => Some(Instructions::SET(7, ArithmeticSource::A)),
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
            0xE2 => Some(Instructions::LDH(LoadType::Other(
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
    pub fn new(buffer: Vec<u8>) -> CPU {
        CPU {
            ime: false,
            is_halted: false,
            bus: MemoryBus {
                bootrom_run: false,
                bootrom: vec![0; 256],
                memory: memory::MMU {
                    game_rom: buffer,
                    bios: [0; 0x100],
                    cartridge_type: 0,
                    wram: [0; 0x2000],
                    zram: [0; 0x80],
                    interrupt_enable: 0,
                    interrupt_flag: 0,
                    ram_enabled: false,
                    ram_banks: [0; 0x8000],
                    current_rom_bank: 1,
                    current_ram_bank: 0,
                    mbc1: false,
                    mbc2: false,
                },
                timer: timer::Timer {
                    timer_counter_tima: 0,
                    divider_register: 0,
                    divider_counter: 0,
                    timer_modulo_tma: 0,
                    clock_counter: 0,
                    input_clock_speed: 1024,
                    clock_enabled: false,
                },
                apu: audio::APU {},
                keys: keys::Keys {
                    joypad_state: 0,
                    joypad_register: 0xFF,
                },
                gpu: gpu::GPU {
                    tile_set: [gpu::empty_tile(); 384],
                    screen_data: [[[0; 1]; 160]; 144],
                    vram: [0; gpu::VRAM_SIZE],
                    oam: [0; 0xA0],
                    stat: 0,
                    extra: [0; 0x3F],
                    bg_palette: 0,
                    obp0_palette: 0,
                    obp1_palette: 0,
                    lcd_enabled: false,
                    window_tilemap_display_select: false, // Bit 6
                    window_display_enable: false,         // Bit 5
                    bg_window_tile_data_select: false,    // Bit 4
                    bg_tile_map_display_select: false,    // Bit 3
                    obj_size: false,                      // Bit 2
                    obj_display_enable: false,            // Bit 1
                    bg_window_display_priority: false,    // Bit 0
                    scroll_x: 0,
                    scroll_y: 0,
                    lyc: 0,
                    window_x: 0,
                    window_y: 0,
                    current_line: 0,
                    scanline_counter: 456,
                },
            },
            regs: Registers {
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
            },
            pc: 0x0000,
            sp: 0x0000,
        }
    }

    pub fn initialize_system(&mut self) {
        /* Power Up Sequence */
        self.regs.set_af(0x01B0);
        self.regs.set_bc(0x0013);
        self.regs.set_de(0x00D8);
        self.regs.set_hl(0x014D);
        self.sp = 65534;
        self.bus.write_byte(0xFF05, 0x00);
        self.bus.write_byte(0xFF06, 0x00);
        self.bus.write_byte(0xFF07, 0x00);
        self.bus.write_byte(0xFF10, 0x80);
        self.bus.write_byte(0xFF11, 0xBF);
        self.bus.write_byte(0xFF12, 0xF3);
        self.bus.write_byte(0xFF14, 0xBF);
        self.bus.write_byte(0xFF16, 0x3F);
        self.bus.write_byte(0xFF17, 0x00);
        self.bus.write_byte(0xFF19, 0xBF);
        self.bus.write_byte(0xFF1A, 0x7F);
        self.bus.write_byte(0xFF1B, 0xFF);
        self.bus.write_byte(0xFF1C, 0x9F);
        self.bus.write_byte(0xFF1E, 0xBF);
        self.bus.write_byte(0xFF20, 0xFF);
        self.bus.write_byte(0xFF21, 0x00);
        self.bus.write_byte(0xFF22, 0x00);
        self.bus.write_byte(0xFF23, 0xBF);
        self.bus.write_byte(0xFF24, 0x77);
        self.bus.write_byte(0xFF25, 0xF3);
        self.bus.write_byte(0xFF26, 0xF1);
        self.bus.write_byte(0xFF40, 0x91);
        self.bus.write_byte(0xFF42, 0x00);
        self.bus.write_byte(0xFF43, 0x00);
        self.bus.write_byte(0xFF45, 0x00);
        self.bus.write_byte(0xFF47, 0xFC);
        self.bus.write_byte(0xFF48, 0xFF);
        self.bus.write_byte(0xFF49, 0xFF);
        self.bus.write_byte(0xFF4A, 0x00);
        self.bus.write_byte(0xFF4B, 0x00);
        self.bus.write_byte(0xFFFF, 0x00);
    }

    pub fn initialize_bootrom(&mut self) {

        self.bus.bootrom = fs::read("dmg_boot.bin").unwrap();

        // Put the bootrom in the rom memory
        for item in 0..=0xFF {
            self.bus.write_byte(item as u16, self.bus.bootrom[item]);
        }
    }

    pub fn run_bootrom(&mut self) {
        let mut current_cycles: u32 = 0;

        while current_cycles < timer::MAX_CYCLES {
            let mut instruction = self.bus.bootrom[self.pc as usize];
            let prefixed = instruction == 0xCB;
            if prefixed {
                instruction = self.bus.bootrom[self.pc as usize + 1];
            }
            let (next, cycles) =
                if let Some(instruction) = Instructions::from_byte(instruction, prefixed) {
                    self.decode_instruction(instruction)
                } else {
                    panic!("Unknown instruction found! Opcode!");
            };

            self.pc = next;
            //print!("{} ", description);
            current_cycles += cycles as u32;
            self.update_timers(cycles);
            self.update_graphics(cycles);
            self.process_interrupts();

            if next > 0xFF {
                self.bus.bootrom_run = true;
                self.initialize_system();
                break;
            }
        }
    }

    pub fn update_emulator(&mut self) {
        let mut current_cycles = 0;

        while current_cycles < timer::MAX_CYCLES {
            let cycles: u8 = self.execute_instruction();
            current_cycles += cycles as u32;

            /* Update timers */
            self.update_timers(cycles);

            /* Update graphics */
            self.update_graphics(cycles);

            /* Check for interrupts */
            self.process_interrupts();
        }
    }

    fn update_graphics(&mut self, cycles: u8) {
        self.set_status();

        if self.bus.gpu.lcd_enabled {
            self.bus.gpu.scanline_counter -= cycles as i16;
        } else {
            return;
        }

        if self.bus.gpu.scanline_counter <= 0 {
            self.bus.gpu.current_line += 1;
            self.bus.gpu.scanline_counter = 456;

            match self.bus.gpu.current_line {
                144 => self.set_interrupt(Interrupts::VBlank),
                0..=143 => self.bus.gpu.draw_scanline(),
                _ => self.bus.gpu.current_line = 0,
            }
        }
    }

    fn set_status(&mut self) {
        if !self.bus.gpu.lcd_enabled {
            self.bus.gpu.scanline_counter = 456;
            self.bus.gpu.current_line = 0;
            self.bus.gpu.stat &= 0xFC;
            self.bus.gpu.stat |= 0x01;
            return;
        }

        let current_mode = self.bus.gpu.stat & 0b11;
        let mode: u8;
        let mut interrupt_requested: bool = false;

        if self.bus.gpu.current_line >= 144 {
            mode = 1;
            self.bus.gpu.stat |= 0x01;
            self.bus.gpu.stat &= 0xFD;
            interrupt_requested = self.bus.gpu.stat & 0x10 != 0;
        } else {
            if self.bus.gpu.scanline_counter >= 376 {
                mode = 2;
                self.bus.gpu.stat |= 0x02;
                self.bus.gpu.stat &= 0xFE;
                interrupt_requested = self.bus.gpu.stat & 0x20 != 0;
            } else if self.bus.gpu.scanline_counter >= 204 {
                mode = 3;
                self.bus.gpu.stat |= 0x03;
            } else {
                mode = 0;
                self.bus.gpu.stat &= 0xFC;
                interrupt_requested = self.bus.gpu.stat & 0x03 != 0;
            }
        }

        if interrupt_requested && (mode != current_mode) {
            self.set_interrupt(Interrupts::LCDStat);
        }

        if self.bus.gpu.lyc == self.bus.gpu.current_line {
            self.bus.gpu.stat |= 0x04;
            if self.bus.gpu.stat & 0x20 != 0 {
                self.set_interrupt(Interrupts::LCDStat);
            }
        } else {
            self.bus.gpu.stat &= 0xFB;
        }
    }

    fn update_timers(&mut self, cycles: u8) {
        /* Divider Register */
        self.bus.timer.divider_counter += cycles as u32;
        if self.bus.timer.divider_counter >= 255 {
            self.bus.timer.divider_counter = 0;
            self.bus.timer.divider_register += 1;
        }

        if self.bus.timer.clock_enabled {
            self.bus.timer.clock_counter += cycles as u16;

            if self.bus.timer.clock_counter >= self.bus.timer.input_clock_speed as u16 {
                self.bus.timer.clock_counter = 0;

                if self.bus.timer.timer_counter_tima == 255 {
                    self.bus.timer.timer_counter_tima = self.bus.timer.timer_modulo_tma;
                    self.set_interrupt(Interrupts::Timer);
                } else {
                    self.bus.timer.timer_counter_tima += 1;
                }
            }
        }
    }

    fn set_interrupt(&mut self, interrupt: Interrupts) {
        let mask = match interrupt {
            Interrupts::VBlank => 0x01,
            Interrupts::LCDStat => 0x02,
            Interrupts::Timer => 0x04,
            //Interrupts::Serial => 0x08,
            Interrupts::Joypad => 0x10,
        };

        self.bus.memory.interrupt_flag |= mask;
    }

    fn process_interrupts(&mut self) {
        /* Check for interrupts */
        if self.ime
            && (self.bus.memory.interrupt_enable != 0)
            && (self.bus.memory.interrupt_flag != 0)
        {
            let fired = self.bus.memory.interrupt_enable & self.bus.memory.interrupt_flag;

            if (fired & 0x01) != 0 {
                self.bus.memory.interrupt_flag &= 0xFE;
                println!("Interrupt 0x01 Requested!");
                self.rst40();
            } else if (fired & 0x02) != 0 {
                self.bus.memory.interrupt_flag &= 0xFD;
                println!("Interrupt 0x01 Requested!");
                self.rst48();
            } else if (fired & 0x04) != 0 {
                self.bus.memory.interrupt_flag &= 0xFB;
                println!("Interrupt 0x01 Requested!");
                self.rst50();
            } else if (fired & 0x08) != 0 {
                self.bus.memory.interrupt_flag &= 0xF7;
                println!("Interrupt 0x01 Requested!");
                self.rst58();
            } else if (fired & 0x0F) != 0 {
                self.bus.memory.interrupt_flag &= 0xEF;
                println!("Interrupt 0x01 Requested!");
                self.rst60();
            } else {
                self.ime = true;
            }
        }
    }

    pub fn set_key_pressed(&mut self, key: u8) {
        let mut state = self.bus.keys.joypad_state;
        let mut previously_unset: bool = false;

        if state & (1 << key) == 0 {
            previously_unset = true;
        }

        let mask: u8 = !(1 << key);
        state &= mask;

        let button: bool;

        if key > 3 {
            button = true;
        } else {
            button = false;
        }

        let mut interrupt_requested = false;
        let reg = self.bus.keys.joypad_register;

        if button && (reg & 0x20 == 0) {
            //Button
            interrupt_requested = true;
        } else if button && (reg & 0x10 == 0) {
            //Direction
            interrupt_requested = true;
        }

        if interrupt_requested && !previously_unset {
            self.set_interrupt(Interrupts::Joypad);
        }

        self.bus.keys.joypad_state = state;
    }

    pub fn set_key_released(&mut self, key: u8) {
        self.bus.keys.joypad_state |= 1 << key;
    }

    pub fn execute_instruction(&mut self) -> u8 {
        let mut instruction = self.bus.read_byte(self.pc);
        let prefixed = instruction == 0xCB;
        if prefixed {
            instruction = self.bus.read_byte(self.pc + 1);
        }

        let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
        print!("{} ", description);

        let (next, cycles) = if let Some(instruction) =
            Instructions::from_byte(instruction, prefixed)
        {
            self.decode_instruction(instruction)
        } else {
            let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
            panic!("Unknown instruction found! Opcode: {}", description);
        };

        self.pc = next;
        cycles
    }

    /* V-Blank Interrupt */
    fn rst40(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x40;
    }

    /* LCD Status Triggers */
    fn rst48(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x48;
    }

    /* Timer Overflow */
    fn rst50(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x50;
    }

    /* Serial Link */
    fn rst58(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x58;
    }

    /* Joypad Press */
    fn rst60(&mut self) {
        self.ime = false;
        self.sp -= 2;
        self.bus.write_word(self.sp, self.pc + 1);
        self.pc = 0x60;
    }

    fn decode_instruction(&mut self, instruction: Instructions) -> (u16, u8) {
        if self.is_halted {
            return (self.pc, 4);
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

                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RETI() => {
                self.ime = true;
                self.sp += 2;
                (self.bus.read_word(self.sp - 2), 16)
            }

            Instructions::DI() => {
                self.ime = false;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::EI() => {
                self.ime = true;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::HALT() => {
                self.is_halted = true;
                (self.pc, 4)
            }

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

                self.sp = self.sp.wrapping_sub(2);
                self.bus.write_word(self.sp, self.pc + 1);
                (location, 16)
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
                let old: u8 = if (self.regs.a & 0x80) != 0 {1} else {0};
                self.regs.f.carry = old != 0;
                self.regs.a = (self.regs.a << 1) | old;
                self.regs.f.zero = false;
                self.regs.f.half_carry = false;
                self.regs.f.subtract = false;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RLC(source) => {
                let new_value: u8;
                let old: u8;

                match source {
                    ArithmeticSource::A => {
                        old = if (self.regs.a & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.a << 1) | old;
                        self.regs.a = new_value;
                        self.regs.f.zero = self.regs.a == 0;
                    }

                    ArithmeticSource::B => {
                        old = if (self.regs.b & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.b << 1) | old;
                        self.regs.b = new_value;
                        self.regs.f.zero = self.regs.b == 0;
                    }

                    ArithmeticSource::C => {
                        old = if (self.regs.c & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.c << 1) | old;
                        self.regs.c = new_value;
                        self.regs.f.zero = self.regs.c == 0;
                    }

                    ArithmeticSource::D => {
                        old = if (self.regs.d & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.d << 1) | old;
                        self.regs.d = new_value;
                        self.regs.f.zero = self.regs.d == 0;
                    }

                    ArithmeticSource::E => {
                        old = if (self.regs.e & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.e << 1) | old;
                        self.regs.e = new_value;
                        self.regs.f.zero = self.regs.e == 0;
                    }

                    ArithmeticSource::H => {
                        old = if (self.regs.h & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.h << 1) | old;
                        self.regs.h = new_value;
                        self.regs.f.zero = self.regs.h == 0;
                    }

                    ArithmeticSource::L => {
                        old = if (self.regs.l & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.l << 1) | old;
                        self.regs.l = new_value;
                        self.regs.f.zero = self.regs.l == 0;
                    }

                    ArithmeticSource::HLAddr => {
                        let mut byte = self.bus.read_byte(self.regs.get_hl());
                        old = if (byte & 0x80) != 0 {1} else {0};
                        self.regs.f.carry = old != 0;
                        byte = (byte << 1) | old;
                        self.bus.write_byte(self.regs.get_hl(), byte);
                        self.regs.f.zero = byte == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = false;
                        return (self.pc.wrapping_add(2), 16);
                    }

                    _ => panic!(),
                }

                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::RLA() => {
                let flag_c = (self.regs.a & 0x80) >> 7 == 0x01;
                let r = (self.regs.a << 1) + (self.regs.f.carry as u8);
                self.regs.f.carry = flag_c;
                self.regs.f.zero = false;
                self.regs.f.half_carry = false;
                self.regs.f.subtract = false;
                self.regs.a = r;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RL(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut byte = self.bus.read_byte(self.regs.get_hl());
                    let flag_c = if self.regs.f.carry {1} else {0};
                    self.regs.f.carry = (byte & 0x80) != 0;
                    byte = (byte << 1) + flag_c;
                    self.bus.write_byte(self.regs.get_hl(), byte);
                    self.regs.f.zero = byte == 0;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: u8 = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    _ => panic!(),
                };

                let flag_c = (reg & 0x80) >> 7 == 0x01;
                let new_value = (reg << 1) | (self.regs.f.carry as u8);
                self.regs.f.carry = flag_c;
                self.regs.f.zero = new_value == 0;
                self.regs.f.half_carry = false;
                self.regs.f.subtract = false;

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                }

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::CCF() => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = !self.regs.f.carry;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::CPL() => {
                self.regs.f.half_carry = true;
                self.regs.f.subtract = true;
                self.regs.a = !self.regs.a;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::SCF() => {
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = true;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RRCA() => {
                self.regs.f.carry = self.regs.a & 0x01 != 0;
                self.regs.a = (self.regs.a >> 1) | ((self.regs.a & 0x01) << 7);
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RRC(source) => {
                let mut value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => self.bus.read_byte(self.regs.get_hl()),
                    _ => panic!(),
                };

                match source {
                    ArithmeticSource::A => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.a = self.regs.a >> 1 | value << 7;
                        self.regs.f.zero = self.regs.a == 0;
                    }

                    ArithmeticSource::B => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.b = self.regs.b >> 1 | value << 7;
                        self.regs.f.zero = self.regs.b == 0;
                    }

                    ArithmeticSource::C => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.c = self.regs.c >> 1 | value << 7;
                        self.regs.f.zero = self.regs.c == 0;
                    }

                    ArithmeticSource::D => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.d = self.regs.d >> 1 | value << 7;
                        self.regs.f.zero = self.regs.d == 0;
                    }

                    ArithmeticSource::E => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.e = self.regs.e >> 1 | value << 7;
                        self.regs.f.zero = self.regs.e == 0;
                    }

                    ArithmeticSource::H => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.h = self.regs.h >> 1 | value << 7;
                        self.regs.f.zero = self.regs.h == 0;
                    }

                    ArithmeticSource::L => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        self.regs.l = self.regs.l >> 1 | value << 7;
                        self.regs.f.zero = self.regs.l == 0;
                    }

                    ArithmeticSource::HLAddr => {
                        value &= 0x01;
                        self.regs.f.carry = value != 0;
                        value = value >> 1 | value << 7;
                        self.bus.write_byte(self.regs.get_hl(), value);
                        self.regs.f.zero = value == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = false;
                        return (self.pc.wrapping_add(2), 16);
                    }

                    _ => panic!(),
                }

                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::RR(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut value = self.bus.read_byte(self.regs.get_hl());
                    self.regs.f.carry = value & 0x01 != 0;
                    value = (value >> 1) | ((self.regs.f.carry as u8) << 7);
                    self.regs.f.zero = value == 0;
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: u8 = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    _ => panic!(),
                };

                self.regs.f.carry = reg & 0x01 != 0;
                let new_value = (reg >> 1) | ((self.regs.f.carry as u8) << 7);

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                };

                self.regs.f.zero = new_value == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::RRA() => {
                let flag_c = if self.regs.f.carry {1} else {0};
                self.regs.f.carry = self.regs.a & 0x01 != 0;
                let new_value = (self.regs.a >> 7) | (flag_c << 7);
                self.regs.f.zero = false;
                self.regs.a = new_value;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RET(test) => {
                let jump_condition = match test {
                    JumpTest::NotZero => !self.regs.f.zero,
                    JumpTest::Zero => self.regs.f.zero,
                    JumpTest::Carry => self.regs.f.carry,
                    JumpTest::NotCarry => !self.regs.f.carry,
                    JumpTest::Always => true,
                };

                if test == JumpTest::Always {
                    let (next_pc, cycle) = self.return_(jump_condition);
                    (next_pc, cycle - 4)
                } else {
                    self.return_(jump_condition)
                }
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

            Instructions::JPHL() => (self.regs.get_hl(), 4),

            Instructions::NOP() => (self.pc.wrapping_add(1), 4),

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

                match target {
                    IncDecTarget::HL | IncDecTarget::BC | IncDecTarget::DE | IncDecTarget::SP => {
                        (self.pc.wrapping_add(1), 8)
                    }
                    _ => (self.pc.wrapping_add(1), 4),
                }
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
                        self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
                    }
                    IncDecTarget::BC => {
                        self.regs.set_bc(self.regs.get_bc().wrapping_add(1));
                    }
                    IncDecTarget::DE => {
                        self.regs.set_de(self.regs.get_de().wrapping_add(1));
                    }
                    IncDecTarget::SP => {
                        self.sp = self.sp.wrapping_add(1);
                    }
                }

                match target {
                    IncDecTarget::HL | IncDecTarget::BC | IncDecTarget::DE | IncDecTarget::SP => {
                        (self.pc.wrapping_add(1), 8)
                    }
                    IncDecTarget::HLAddr => (self.pc.wrapping_add(1), 12),
                    _ => (self.pc.wrapping_add(1), 4),
                }
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

                    if (target == LoadOtherTarget::A) && (source == LoadOtherSource::A8) {
                        (self.pc.wrapping_add(1), 12)
                    } else if target == LoadOtherTarget::A8 {
                        (self.pc.wrapping_add(1), 12)
                    } else {
                        (self.pc.wrapping_add(1), 8)
                    }
                }

                _ => panic!("Unimplemented!"),
            },

            Instructions::LD(load_type) => match load_type {
                LoadType::Word(target, source) => {
                    let source_value = match source {
                        LoadWordSource::D16 => self.read_next_word(),
                        LoadWordSource::SP => self.sp,
                        LoadWordSource::HL => self.regs.get_hl(),
                        LoadWordSource::SPr8 => self.sp.wrapping_add(self.read_next_byte() as u16),
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
                        LoadWordSource::HL => (self.pc.wrapping_add(1), 8),
                        LoadWordSource::SPr8 => (self.pc.wrapping_add(2), 12),
                        LoadWordSource::D16 => (self.pc.wrapping_add(3), 12),
                        LoadWordSource::SP => (self.pc.wrapping_add(3), 20),
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
                            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
                            self.bus.read_byte(self.regs.get_hl().wrapping_sub(1))
                        }
                        LoadByteSource::HLD => {
                            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
                            self.bus.read_byte(self.regs.get_hl().wrapping_add(1))
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
                            self.regs.set_hl(self.regs.get_hl().wrapping_add(1));
                        }
                        LoadByteTarget::HLD => {
                            self.bus.write_byte(self.regs.get_hl(), source_value);
                            self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
                        }
                        LoadByteTarget::BC => {
                            self.bus.write_byte(self.regs.get_bc(), source_value);
                        }
                        LoadByteTarget::DE => {
                            self.bus.write_byte(self.regs.get_de(), source_value);
                        }
                        LoadByteTarget::A16 => {
                            self.bus.write_byte(self.read_next_word(), source_value);
                            return (self.pc.wrapping_add(3), 16);
                        }
                    }

                    if source == LoadByteSource::D8 {
                        if target == LoadByteTarget::HL {
                            (self.pc.wrapping_add(2), 12)
                        } else {
                            (self.pc.wrapping_add(2), 8)
                        }
                    } else if source == LoadByteSource::HL {
                        (self.pc.wrapping_add(1), 8)
                    } else if (target == LoadByteTarget::HL)
                        || (target == LoadByteTarget::HLD)
                        || (target == LoadByteTarget::HLI)
                    {
                        (self.pc.wrapping_add(1), 8)
                    } else if (target == LoadByteTarget::BC) || (target == LoadByteTarget::DE) {
                        (self.pc.wrapping_add(1), 8)
                    } else if (source == LoadByteSource::BC)
                        || (source == LoadByteSource::DE)
                        || (source == LoadByteSource::HLD)
                        || (source == LoadByteSource::HLI)
                    {
                        (self.pc.wrapping_add(1), 8)
                    } else if source == LoadByteSource::A16 {
                        (self.pc.wrapping_add(3), 16)
                    } else {
                        (self.pc.wrapping_add(1), 4)
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
                (self.pc.wrapping_add(1), 16)
            }

            Instructions::POP(target) => {
                let result = self.pop();
                match target {
                    StackTarget::BC => self.regs.set_bc(result),
                    StackTarget::AF => self.regs.set_af(result),
                    StackTarget::DE => self.regs.set_de(result),
                    StackTarget::HL => self.regs.set_hl(result),
                };

                (self.pc.wrapping_add(1), 12)
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
                        panic!("Error");
                    }
                };

                match target {
                    ArithmeticTarget::A => {
                        let a_reg = self.regs.a;
                        self.regs.f.zero = a_reg == source_value;
                        self.regs.f.subtract = true;
                        self.regs.f.half_carry =
                            ((a_reg as i16 - source_value as i16) & 0xF) as u8 > (a_reg & 0xF);
                        self.regs.f.carry = source_value > a_reg;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::SLA(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut value = self.bus.read_byte(self.regs.get_hl());
                    self.regs.f.carry = value & 0x80 != 0;
                    value <<= 1;
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.zero = value == 0;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: &u8 = match source {
                    ArithmeticSource::A => &self.regs.a,
                    ArithmeticSource::B => &self.regs.b,
                    ArithmeticSource::C => &self.regs.c,
                    ArithmeticSource::D => &self.regs.d,
                    ArithmeticSource::E => &self.regs.e,
                    ArithmeticSource::H => &self.regs.h,
                    ArithmeticSource::L => &self.regs.l,
                    _ => panic!(),
                };

                self.regs.f.carry = *reg & 0x80 != 0;
                let new_value = *reg << 1;

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                };

                self.regs.f.zero = new_value == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::SRA(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut value = self.bus.read_byte(self.regs.get_hl());
                    self.regs.f.carry = value & 0x01 != 0;
                    value = value >> 1 | (value & 0x80);
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.zero = value == 0;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: &u8 = match source {
                    ArithmeticSource::A => &self.regs.a,
                    ArithmeticSource::B => &self.regs.b,
                    ArithmeticSource::C => &self.regs.c,
                    ArithmeticSource::D => &self.regs.d,
                    ArithmeticSource::E => &self.regs.e,
                    ArithmeticSource::H => &self.regs.h,
                    ArithmeticSource::L => &self.regs.l,
                    _ => panic!(),
                };

                self.regs.f.carry = *reg & 0x01 != 0;
                let new_value = *reg >> 1 | (*reg & 0x80);

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                };

                self.regs.f.zero = new_value == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::SWAP(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut value = self.bus.read_byte(self.regs.get_hl());
                    value = ((value & 0xF) << 4) | ((value & 0xF0) >> 4);
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.zero = value == 0;
                    self.regs.f.carry = false;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: u8 = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    _ => panic!(),
                };

                let new_value = ((reg & 0xF) << 4) | ((reg & 0xF0) >> 4);

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                };

                self.regs.f.zero = new_value == 0;
                self.regs.f.carry = false;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::SRL(source) => {
                if source == ArithmeticSource::HLAddr {
                    let mut value = self.bus.read_byte(self.regs.get_hl());
                    self.regs.f.carry = value & 0x01 != 0;
                    value >>= 1;
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.zero = value == 0;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    return (self.pc.wrapping_add(2), 16);
                }

                let reg: &u8 = match source {
                    ArithmeticSource::A => &self.regs.a,
                    ArithmeticSource::B => &self.regs.b,
                    ArithmeticSource::C => &self.regs.c,
                    ArithmeticSource::D => &self.regs.d,
                    ArithmeticSource::E => &self.regs.e,
                    ArithmeticSource::H => &self.regs.h,
                    ArithmeticSource::L => &self.regs.l,
                    _ => panic!(),
                };

                self.regs.f.carry = *reg & 0x01 != 0;
                let new_value = *reg >> 1;

                match source {
                    ArithmeticSource::A => self.regs.a = new_value,
                    ArithmeticSource::B => self.regs.b = new_value,
                    ArithmeticSource::C => self.regs.c = new_value,
                    ArithmeticSource::D => self.regs.d = new_value,
                    ArithmeticSource::E => self.regs.e = new_value,
                    ArithmeticSource::H => self.regs.h = new_value,
                    ArithmeticSource::L => self.regs.l = new_value,
                    _ => panic!(),
                };

                self.regs.f.zero = new_value == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::BIT(target, source) => {
                let value = 1 << target;

                let zero: bool = match source {
                    ArithmeticSource::A => (self.regs.a & value) == 0,
                    ArithmeticSource::B => (self.regs.b & value) == 0,
                    ArithmeticSource::C => (self.regs.c & value) == 0,
                    ArithmeticSource::D => (self.regs.d & value) == 0,
                    ArithmeticSource::E => (self.regs.e & value) == 0,
                    ArithmeticSource::H => (self.regs.h & value) == 0,
                    ArithmeticSource::L => (self.regs.l & value) == 0,
                    ArithmeticSource::HLAddr => {
                        (self.bus.read_byte(self.regs.get_hl()) & value) == 0
                    }
                    _ => panic!(),
                };

                self.regs.f.zero = zero;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = true;

                if source == ArithmeticSource::HLAddr {
                    (self.pc.wrapping_add(2), 16)
                } else {
                    (self.pc.wrapping_add(2), 8)
                }
            }

            Instructions::RES(target, source) => {
                let value = 1 << target;

                match source {
                    ArithmeticSource::A => self.regs.a &= !value,
                    ArithmeticSource::B => self.regs.b &= !value,
                    ArithmeticSource::C => self.regs.c &= !value,
                    ArithmeticSource::D => self.regs.d &= !value,
                    ArithmeticSource::E => self.regs.e &= !value,
                    ArithmeticSource::H => self.regs.h &= !value,
                    ArithmeticSource::L => self.regs.l &= !value,
                    ArithmeticSource::HLAddr => self.bus.write_byte(
                        self.regs.get_hl(),
                        self.bus.read_byte(self.regs.get_hl()) & !value,
                    ),
                    _ => panic!(),
                }

                if source == ArithmeticSource::HLAddr {
                    (self.pc.wrapping_add(2), 16)
                } else {
                    (self.pc.wrapping_add(2), 8)
                }
            }

            Instructions::SET(target, source) => {
                let value = 1 << target;

                match source {
                    ArithmeticSource::A => self.regs.a |= value,
                    ArithmeticSource::B => self.regs.b |= value,
                    ArithmeticSource::C => self.regs.c |= value,
                    ArithmeticSource::D => self.regs.d |= value,
                    ArithmeticSource::E => self.regs.e |= value,
                    ArithmeticSource::H => self.regs.h |= value,
                    ArithmeticSource::L => self.regs.l |= value,
                    ArithmeticSource::HLAddr => self.bus.write_byte(
                        self.regs.get_hl(),
                        self.bus.read_byte(self.regs.get_hl()) | value,
                    ),
                    _ => panic!(),
                }

                if source == ArithmeticSource::HLAddr {
                    (self.pc.wrapping_add(2), 16)
                } else {
                    (self.pc.wrapping_add(2), 8)
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
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                        let flag_c = if self.regs.f.carry {1} else {0};
                        let sum: u16 = self.regs.a as u16
                            + source_value as u16
                            + flag_c as u16;
                        self.regs.f.carry = sum >= 0x100;
                        self.regs.f.subtract = false;
                        self.regs.f.half_carry = ((self.regs.a as u16 & 0xF)
                            + (source_value as u16 & 0xF)
                            + flag_c as u16)
                            >= 0x10;
                        self.regs.a = sum as u8;
                        self.regs.f.zero = self.regs.a == 0;
                    }
                    _ => panic!("Not an option!"),
                }

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
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
                (self.pc.wrapping_add(1), 8)
            }

            Instructions::ADD(target, source) => {
                match source {
                    ArithmeticSource::I8 => {
                        /* ADD SP, r8 */
                        let source_value = self.read_next_byte();
                        let sp_value = self.sp;
                        let (new_value, _) = (sp_value as i32).overflowing_add(source_value as i32);
                        self.regs.f.zero = new_value == 0;
                        self.regs.f.subtract = false;
                        self.regs.f.carry = new_value > 0xFFFF;
                        self.regs.f.half_carry =
                            (self.sp & 0xF) + (source_value as u16 & 0xF) > 0xF;
                        self.sp = new_value as u16;
                        return (self.pc.wrapping_add(2), 16);
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
                    _ => panic!("Error"),
                };

                match target {
                    ArithmeticTarget::A => {
                        /* ADD A, r8 */
                        let new_value = self.add(source_value);
                        self.regs.a = new_value;
                        match source {
                            ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                            ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                            _ => (self.pc.wrapping_add(1), 4),
                        }
                    }

                    _ => panic!("Error"),
                }
            }
        }
    }

    fn inc(&mut self, register: &u8) -> u8 {
        let new_value = (*register).wrapping_add(1);
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = false;
        self.regs.f.half_carry = *register & 0xF == 0xF;
        new_value as u8
    }

    fn dec(&mut self, register: &u8) -> u8 {
        let new_value = (*register).wrapping_sub(1);
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = true;
        self.regs.f.half_carry = new_value & 0xF == 0xF;
        new_value as u8
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

    fn jump(&self, should_jump: bool) -> (u16, u8) {
        if should_jump {
            let lower_byte = self.bus.read_byte(self.pc + 1) as u16;
            let higher_byte = self.bus.read_byte(self.pc + 2) as u16;
            ((higher_byte << 8) | lower_byte, 16)
        } else {
            (self.pc.wrapping_add(3), 12)
        }
    }

    fn jump_relative(&self, should_jump: bool) -> (u16, u8) {
        let next = self.pc.wrapping_add(2);
        if should_jump {
            let byte = self.bus.read_byte(self.pc + 1) as i8;
            let pc = if byte >= 0 {
                next.wrapping_add(byte as u16)
            } else {
                next.wrapping_sub(byte.abs() as u16)
            };

            (pc, 12)
        } else {
            (next, 8)
        }
    }

    fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, ((value & 0xFF00) >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.bus.write_byte(self.sp, (value & 0x00FF) as u8);
    }

    fn pop(&mut self) -> u16 {
        let lsb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let msb = self.bus.read_byte(self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);

        (msb << 8) | lsb
    }

    fn call(&mut self, should_jump: bool) -> (u16, u8) {
        let next_pc = self.pc.wrapping_add(3);

        if should_jump {
            self.push(next_pc);
            (self.read_next_word(), 24)
        } else {
            (next_pc, 12)
        }
    }

    fn return_(&mut self, should_jump: bool) -> (u16, u8) {
        if should_jump {
            (self.pop(), 20)
        } else {
            (self.pc.wrapping_add(1), 8)
        }
    }

    fn read_next_word(&self) -> u16 {
        let lower = self.bus.read_byte(self.pc + 1) as u16;
        let higher = self.bus.read_byte(self.pc + 2) as u16;
        let word = (higher << 8) | lower;
        word
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
