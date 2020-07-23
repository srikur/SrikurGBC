use std::fs;
use std::io::prelude::*;
use std::path::{Path};
use std::rc::Rc;
use std::cell::RefCell;
use super::registers::*;
use super::interrupts::*;
use super::memory::*;
use super::gpu::*;
use super::joypad::*;
use super::timer::*;
use super::audio::*;

pub struct CPU {
    pub regs: Registers,
    pub pc: u16,
    pub sp: u16,
    pub bus: MemoryBus,

    // Extras
    pub icount: u8,
    pub halted: bool,
    pub halt_bug: bool,

    // Logging
    pub log: bool,
    pub log_buffer: std::fs::File,

    // Timing
    pub step_cycles: u32, 
}

pub struct MemoryBus {
    pub intref: Rc<RefCell<Interrupt>>,
    pub memory: MMU,
    pub gpu: GPU,
    pub keys: Joypad,
    pub timer: Timer,
    pub apu: APU,
    pub run_bootrom: bool,
    pub bootrom: Vec<u8>,
}

#[derive(PartialEq)]
#[rustfmt::skip]
enum LoadByteTarget {
    A, B, C, D, E, H, L, HLI, HLD, HL, BC, DE, A16,
}

#[derive(PartialEq)]
#[rustfmt::skip]
enum LoadByteSource {
    A, B, C, D, E, H, L, D8, HL, HLI, HLD, A16, BC, DE,
}

#[rustfmt::skip]
#[derive(PartialEq)]
enum LoadWordSource {
    D16, HL, SP, SPr8,
}

#[rustfmt::skip]
enum LoadWordTarget {
    BC, DE, HL, SP, A16,
}

#[derive(PartialEq)]
enum LoadOtherTarget {
    A, A8, CAddress,
}

#[derive(PartialEq)]
enum LoadOtherSource {
    A, A8, CAddress,
}

enum StackTarget {
    BC, DE, HL, AF,
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
    SUB(ArithmeticSource),
    JR(JumpTest),
    JP(JumpTest),
    ADC(ArithmeticSource),
    SBC(ArithmeticSource),
    AND(ArithmeticSource),
    XOR(ArithmeticSource),
    OR(ArithmeticSource),
    CP(ArithmeticSource),
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

#[rustfmt::skip]
enum RSTTargets {
    H00, H10, H20, H30, H08, H18, H28, H38,
}

#[rustfmt::skip]
enum IncDecTarget {
    A, B, C, D, E, H, L, HL, HLAddr, BC, DE, SP,
}

#[rustfmt::skip]
enum Arithmetic16Target {
    HL, BC, DE, SP,
}

#[rustfmt::skip]
enum ArithmeticTarget {
    A, SP,
}

#[rustfmt::skip]
#[derive(Eq, PartialEq)]
enum ArithmeticSource {
    A, B, C, D, E, H, L, U8, HLAddr, I8,
}

#[derive(PartialEq)]
enum JumpTest {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    Always,
}

impl MemoryBus {
    fn read_byte(&self, address: u16) -> u8 {
        let mut address = address as usize;

        match address {
            /* ROM Banks */
            0x0000..=0x7FFF => {
                if self.run_bootrom && (address <= 0xFF) {
                    self.bootrom[address]
                } else {
                    self.memory.cartridge.read_byte(address)
                }
            }

            ERAM_BEGIN..=ERAM_END => self.memory.cartridge.read_byte(address),

            /* Read from VRAM */
            VRAM_BEGIN..=VRAM_END => self.gpu.read_vram(address - VRAM_BEGIN),

            /* Read from Work RAM */
            WRAM_BEGIN..=WRAM_END => self.memory.wram[address - WRAM_BEGIN],
            0xE000..=0xFDFF => {
                address -= 0x2000;
                self.memory.wram[address - WRAM_BEGIN]
            }

            /* Read from Sprite Attribute Table */
            OAM_BEGIN..=OAM_END => self.gpu.oam[address - OAM_BEGIN],

            /* GPU Registers */
            GPU_REGS_BEGIN..=GPU_REGS_END => self.gpu.read_registers(address),

            /* Read from High RAM */
            ZRAM_BEGIN..=ZRAM_END => self.memory.zram[address - ZRAM_BEGIN],

            /* Not usable memory */
            0xFEA0..=0xFEFF => 0x00,

            /* Joypad Input */
            JOYPAD_INPUT => self.keys.get_joypad_state(),

            /* Interrupt Flag 0xFF0F */
            INTERRUPT_FLAG => self.intref.borrow().interrupt_flag,

            /* Interrupt Enable 0xFFFF */
            INTERRUPT_ENABLE => self.intref.borrow().interrupt_enable,

            /* DIV - Divider Register */
            DIVIDER_REGISTER => self.timer.divider_register as u8,

            /* TIMA - Timer Counter */
            TIMA => self.timer.tima as u8,

            /* TAC - Timer Control */
            TAC => {
                let clock = if self.timer.clock_enabled {1} else {0};
                let speed = match self.timer.input_clock_speed {
                    1024 => 0, 
                    6 => 1, 
                    64 => 2,
                    256 => 3,
                    _ => 0,
                };
                (clock << 2) | speed
            },

            /* Audio Controls */
            SOUND_BEGIN..=SOUND_END => self.apu.read_byte(address),

            /* Extra space */
            EXTRA_SPACE_BEGIN..=EXTRA_SPACE_END => {
                self.gpu.extra[address - EXTRA_SPACE_BEGIN]
            }

            0xFF4D => {0x00}

            _ => panic!("Unimplemented register: {:X}", address),
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {

        let address = address as usize;
        match address {
            /* Handle Banking */
            0x0000..=0x7FFF => {
                self.memory.cartridge.write_byte(address, value);
            }

            ERAM_BEGIN..=ERAM_END => {
                self.memory.cartridge.write_byte(address, value);
            }

            JOYPAD_INPUT => self.keys.set_joypad_state(value),

            /* Write to VRAM */
            VRAM_BEGIN..=VRAM_END => {
                self.gpu.write_vram(address - VRAM_BEGIN, value);
            }

            /* Write to WRAM */
            WRAM_BEGIN..=WRAM_END => {
                self.memory.wram[address - WRAM_BEGIN] = value;
            }

            /* Write to Echo RAM */
            0xE000..=0xFDFF => {
                self.memory.wram[address - WRAM_BEGIN - 0x2000] = value;
            }

            /* Write to I/0 Registers */
            INTERRUPT_FLAG => {
                self.intref.borrow_mut().interrupt_flag = value;
            }

            DIVIDER_REGISTER => {
                self.timer.divider_register = 0;
            }

            TIMA => {
                self.timer.tima = value as u8;
            }

            TMA => {
                self.timer.tma = value as u8;
            }

            TAC => {
                /* Timer Control */
                self.timer.clock_enabled = (value & 0x04) != 0;
                let new_speed: u32 = match value & 0x03 {
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
            SOUND_BEGIN..=SOUND_END => self.apu.write_byte(address, value),

            EXTRA_SPACE_BEGIN..=EXTRA_SPACE_END => {
                self.gpu.extra[address - EXTRA_SPACE_BEGIN] = value;
            }

            /* Write to High RAM */
            ZRAM_BEGIN..=ZRAM_END => {
                self.memory.zram[address - ZRAM_BEGIN] = value;
            }

            /* Write to Sprite Attribute Table (OAM) */
            OAM_BEGIN..=OAM_END => {
                self.gpu.oam[address - OAM_BEGIN] = value;
            }

            /* Not usable memory */
            0xFEA0..=0xFEFF => return, // Invalid memory location

            /* Not usable as well */
            0xFF4C..=0xFF7F => return,

            /* Write to Interrupts Enable Register */
            INTERRUPT_ENABLE => {
                self.intref.borrow_mut().interrupt_enable = value;
            }

            /* Write to GPU registers */
            GPU_REGS_BEGIN..=GPU_REGS_END => {
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

            _ => panic!("Unimplemented Register: {:X} Value: {:X}", address, value),
        }

        
    }

    pub fn write_word(&mut self, address: u16, word: u16) {
        let lower = word >> 8;
        let higher = word & 0xFF;
        self.write_byte(address, higher as u8);
        self.write_byte(address + 1, lower as u8);
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

    #[rustfmt::skip]
    fn from_byte_not_prefixed(byte: u8) -> Option<Instructions> {
        match byte {
            0x00 => Some(Instructions::NOP()),
            0x01 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::BC,LoadWordSource::D16,))),
            0x02 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::BC,LoadByteSource::A,))),
            0x03 => Some(Instructions::INC(IncDecTarget::BC)),
            0x04 => Some(Instructions::INC(IncDecTarget::B)),
            0x05 => Some(Instructions::DEC(IncDecTarget::B)),
            0x06 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::D8,))),
            0x07 => Some(Instructions::RLCA()),
            0x08 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::A16,LoadWordSource::SP,))),
            0x09 => Some(Instructions::ADD16(Arithmetic16Target::BC)),
            0x10 => Some(Instructions::NOP()),
            0x0A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::BC,))),
            0x0B => Some(Instructions::DEC(IncDecTarget::BC)),
            0x0C => Some(Instructions::INC(IncDecTarget::C)),
            0x0D => Some(Instructions::DEC(IncDecTarget::C)),
            0x0E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::D8,))),
            0x0F => Some(Instructions::RRCA()),
            0x11 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::DE,LoadWordSource::D16,))),
            0x12 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::DE,LoadByteSource::A,))),
            0x13 => Some(Instructions::INC(IncDecTarget::DE)),
            0x14 => Some(Instructions::INC(IncDecTarget::D)),
            0x15 => Some(Instructions::DEC(IncDecTarget::D)),
            0x16 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::D8,))),
            0x17 => Some(Instructions::RLA()),
            0x18 => Some(Instructions::JR(JumpTest::Always)),
            0x19 => Some(Instructions::ADD16(Arithmetic16Target::DE)),
            0x1A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::DE,))),
            0x1B => Some(Instructions::DEC(IncDecTarget::DE)),
            0x1C => Some(Instructions::INC(IncDecTarget::E)),
            0x1D => Some(Instructions::DEC(IncDecTarget::E)),
            0x1E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::D8,))),
            0x1F => Some(Instructions::RRA()),
            0x20 => Some(Instructions::JR(JumpTest::NotZero)),
            0x21 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::HL,LoadWordSource::D16,))),
            0x22 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HLI,LoadByteSource::A,))),
            0x23 => Some(Instructions::INC(IncDecTarget::HL)),
            0x24 => Some(Instructions::INC(IncDecTarget::H)),
            0x25 => Some(Instructions::DEC(IncDecTarget::H)),
            0x26 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::D8,))),
            0x27 => Some(Instructions::DAA()),
            0x28 => Some(Instructions::JR(JumpTest::Zero)),
            0x29 => Some(Instructions::ADD16(Arithmetic16Target::HL)),
            0x2A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::HLI,))),
            0x2B => Some(Instructions::DEC(IncDecTarget::HL)),
            0x2C => Some(Instructions::INC(IncDecTarget::L)),
            0x2D => Some(Instructions::DEC(IncDecTarget::L)),
            0x2E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::D8,))),
            0x2F => Some(Instructions::CPL()),
            0x30 => Some(Instructions::JR(JumpTest::NotCarry)),
            0x31 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::SP,LoadWordSource::D16,))),
            0x32 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HLD,LoadByteSource::A,))),
            0x33 => Some(Instructions::INC(IncDecTarget::SP)),
            0x34 => Some(Instructions::INC(IncDecTarget::HLAddr)),
            0x35 => Some(Instructions::DEC(IncDecTarget::HLAddr)),
            0x36 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::D8,))),
            0x37 => Some(Instructions::SCF()),
            0x38 => Some(Instructions::JR(JumpTest::Carry)),
            0x39 => Some(Instructions::ADD16(Arithmetic16Target::SP)),
            0x3A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::HLD,))),
            0x3B => Some(Instructions::DEC(IncDecTarget::SP)),
            0x3C => Some(Instructions::INC(IncDecTarget::A)),
            0x3D => Some(Instructions::DEC(IncDecTarget::A)),
            0x3E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::D8,))),
            0x3F => Some(Instructions::CCF()),
            0x40 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::B,))),
            0x41 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::C,))),
            0x42 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::D,))),
            0x43 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::E,))),
            0x44 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::H,))),
            0x45 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::L,))),
            0x46 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::HL,))),
            0x47 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::B,LoadByteSource::A,))),
            0x48 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::B,))),
            0x49 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::C,))),
            0x4A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::D,))),
            0x4B => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::E,))),
            0x4C => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::H,))),
            0x4D => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::L,))),
            0x4E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::HL,))),
            0x4F => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::C,LoadByteSource::A,))),
            0x50 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::B,))),
            0x51 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::C,))),
            0x52 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::D,))),
            0x53 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::E,))),
            0x54 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::H,))),
            0x55 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::L,))),
            0x56 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::HL,))),
            0x57 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::D,LoadByteSource::A,))),
            0x58 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::B,))),
            0x59 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::C,))),
            0x5A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::D,))),
            0x5B => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::E,))),
            0x5C => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::H,))),
            0x5D => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::L,))),
            0x5E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::HL,))),
            0x5F => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::E,LoadByteSource::A,))),
            0x60 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::B,))),
            0x61 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::C,))),
            0x62 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::D,))),
            0x63 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::E,))),
            0x64 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::H,))),
            0x65 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::L,))),
            0x66 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::HL,))),
            0x67 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::H,LoadByteSource::A,))),
            0x68 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::B,))),
            0x69 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::C,))),
            0x6A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::D,))),
            0x6B => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::E,))),
            0x6C => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::H,))),
            0x6D => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::L,))),
            0x6E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::HL,))),
            0x6F => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::L,LoadByteSource::A,))),
            0x70 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::B,))),
            0x71 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::C,))),
            0x72 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::D,))),
            0x73 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::E,))),
            0x74 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::H,))),
            0x75 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::L,))),
            0x76 => Some(Instructions::HALT()),
            0x77 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::HL,LoadByteSource::A,))),
            0x78 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::B,))),
            0x79 => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::C,))),
            0x7A => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::D,))),
            0x7B => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::E,))),
            0x7C => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::H,))),
            0x7D => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::L,))),
            0x7E => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::HL,))),
            0x7F => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::A,))),
            0x80 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::B)),
            0x81 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::C)),
            0x82 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::D)),
            0x83 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::E)),
            0x84 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::H)),
            0x85 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::L)),
            0x86 => Some(Instructions::ADD(ArithmeticTarget::A,ArithmeticSource::HLAddr,)),
            0x87 => Some(Instructions::ADD(ArithmeticTarget::A, ArithmeticSource::A)),
            0x88 => Some(Instructions::ADC(ArithmeticSource::B)),
            0x89 => Some(Instructions::ADC(ArithmeticSource::C)),
            0x8A => Some(Instructions::ADC(ArithmeticSource::D)),
            0x8B => Some(Instructions::ADC(ArithmeticSource::E)),
            0x8C => Some(Instructions::ADC(ArithmeticSource::H)),
            0x8D => Some(Instructions::ADC(ArithmeticSource::L)),
            0x8E => Some(Instructions::ADC(ArithmeticSource::HLAddr)),
            0x8F => Some(Instructions::ADC(ArithmeticSource::A)),
            0x90 => Some(Instructions::SUB(ArithmeticSource::B)),
            0x91 => Some(Instructions::SUB(ArithmeticSource::C)),
            0x92 => Some(Instructions::SUB(ArithmeticSource::D)),
            0x93 => Some(Instructions::SUB(ArithmeticSource::E)),
            0x94 => Some(Instructions::SUB(ArithmeticSource::H)),
            0x95 => Some(Instructions::SUB(ArithmeticSource::L)),
            0x96 => Some(Instructions::SUB(ArithmeticSource::HLAddr)),
            0x97 => Some(Instructions::SUB(ArithmeticSource::A)),
            0x98 => Some(Instructions::SBC(ArithmeticSource::B)),
            0x99 => Some(Instructions::SBC(ArithmeticSource::C)),
            0x9A => Some(Instructions::SBC(ArithmeticSource::D)),
            0x9B => Some(Instructions::SBC(ArithmeticSource::E)),
            0x9C => Some(Instructions::SBC(ArithmeticSource::H)),
            0x9D => Some(Instructions::SBC(ArithmeticSource::L)),
            0x9E => Some(Instructions::SBC(ArithmeticSource::HLAddr)),
            0x9F => Some(Instructions::SBC(ArithmeticSource::A)),
            0xA0 => Some(Instructions::AND(ArithmeticSource::B)),
            0xA1 => Some(Instructions::AND(ArithmeticSource::C)),
            0xA2 => Some(Instructions::AND(ArithmeticSource::D)),
            0xA3 => Some(Instructions::AND(ArithmeticSource::E)),
            0xA4 => Some(Instructions::AND(ArithmeticSource::H)),
            0xA5 => Some(Instructions::AND(ArithmeticSource::L)),
            0xA6 => Some(Instructions::AND(ArithmeticSource::HLAddr)),
            0xA7 => Some(Instructions::AND(ArithmeticSource::A)),
            0xA8 => Some(Instructions::XOR(ArithmeticSource::B)),
            0xA9 => Some(Instructions::XOR(ArithmeticSource::C)),
            0xAA => Some(Instructions::XOR(ArithmeticSource::D)),
            0xAB => Some(Instructions::XOR(ArithmeticSource::E)),
            0xAC => Some(Instructions::XOR(ArithmeticSource::H)),
            0xAD => Some(Instructions::XOR(ArithmeticSource::L)),
            0xAE => Some(Instructions::XOR(ArithmeticSource::HLAddr)),
            0xAF => Some(Instructions::XOR(ArithmeticSource::A)),
            0xB0 => Some(Instructions::OR(ArithmeticSource::B)),
            0xB1 => Some(Instructions::OR(ArithmeticSource::C)),
            0xB2 => Some(Instructions::OR(ArithmeticSource::D)),
            0xB3 => Some(Instructions::OR(ArithmeticSource::E)),
            0xB4 => Some(Instructions::OR(ArithmeticSource::H)),
            0xB5 => Some(Instructions::OR(ArithmeticSource::L)),
            0xB6 => Some(Instructions::OR(ArithmeticSource::HLAddr)),
            0xB7 => Some(Instructions::OR(ArithmeticSource::A)),
            0xB8 => Some(Instructions::CP(ArithmeticSource::B)),
            0xB9 => Some(Instructions::CP(ArithmeticSource::C)),
            0xBA => Some(Instructions::CP(ArithmeticSource::D)),
            0xBB => Some(Instructions::CP(ArithmeticSource::E)),
            0xBC => Some(Instructions::CP(ArithmeticSource::H)),
            0xBD => Some(Instructions::CP(ArithmeticSource::L)),
            0xBE => Some(Instructions::CP(ArithmeticSource::HLAddr)),
            0xBF => Some(Instructions::CP(ArithmeticSource::A)),
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
            0xCC => Some(Instructions::CALL(JumpTest::Zero)),
            0xCD => Some(Instructions::CALL(JumpTest::Always)),
            0xCE => Some(Instructions::ADC(ArithmeticSource::U8)),
            0xCF => Some(Instructions::RST(RSTTargets::H08)),
            0xD0 => Some(Instructions::RET(JumpTest::NotCarry)),
            0xD1 => Some(Instructions::POP(StackTarget::DE)),
            0xD2 => Some(Instructions::JP(JumpTest::NotCarry)),
            0xD4 => Some(Instructions::CALL(JumpTest::NotCarry)),
            0xD5 => Some(Instructions::PUSH(StackTarget::DE)),
            0xD6 => Some(Instructions::SUB(ArithmeticSource::U8)),
            0xD7 => Some(Instructions::RST(RSTTargets::H10)),
            0xD8 => Some(Instructions::RET(JumpTest::Carry)),
            0xD9 => Some(Instructions::RETI()),
            0xDA => Some(Instructions::JP(JumpTest::Carry)),
            0xDC => Some(Instructions::CALL(JumpTest::Carry)),
            0xDE => Some(Instructions::SBC(ArithmeticSource::U8)),
            0xDF => Some(Instructions::RST(RSTTargets::H18)),
            0xE0 => Some(Instructions::LDH(LoadType::Other(LoadOtherTarget::A8,LoadOtherSource::A,))),
            0xE1 => Some(Instructions::POP(StackTarget::HL)),
            0xE2 => Some(Instructions::LDH(LoadType::Other(LoadOtherTarget::CAddress,LoadOtherSource::A,))),
            0xE5 => Some(Instructions::PUSH(StackTarget::HL)),
            0xE6 => Some(Instructions::AND(ArithmeticSource::U8)),
            0xE7 => Some(Instructions::RST(RSTTargets::H20)),
            0xE8 => Some(Instructions::ADD(ArithmeticTarget::SP,ArithmeticSource::I8,)),
            0xE9 => Some(Instructions::JPHL()),
            0xEA => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A16,LoadByteSource::A,))),
            0xEE => Some(Instructions::XOR(ArithmeticSource::U8)),
            0xEF => Some(Instructions::RST(RSTTargets::H28)),
            0xF0 => Some(Instructions::LDH(LoadType::Other(LoadOtherTarget::A,LoadOtherSource::A8,))),
            0xF1 => Some(Instructions::POP(StackTarget::AF)),
            0xF2 => Some(Instructions::LDH(LoadType::Other(LoadOtherTarget::A,LoadOtherSource::CAddress,))),
            0xF3 => Some(Instructions::DI()),
            0xF5 => Some(Instructions::PUSH(StackTarget::AF)),
            0xF6 => Some(Instructions::OR(ArithmeticSource::U8)),
            0xF7 => Some(Instructions::RST(RSTTargets::H30)),
            0xF8 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::HL,LoadWordSource::SPr8,))),
            0xF9 => Some(Instructions::LD(LoadType::Word(LoadWordTarget::SP,LoadWordSource::HL,))),
            0xFA => Some(Instructions::LD(LoadType::Byte(LoadByteTarget::A,LoadByteSource::A16,))),
            0xFB => Some(Instructions::EI()),
            0xFE => Some(Instructions::CP(ArithmeticSource::U8)),
            0xFF => Some(Instructions::RST(RSTTargets::H38)),
            _ => None,
        }
    }
}

impl CPU {
    pub fn new(path: impl AsRef<Path>) -> CPU {

        let intref = Rc::new(RefCell::new(Interrupt::new()));

        CPU {
            regs: Registers::new(),
            step_cycles: 0,
            icount: 0,
            log: false,
            log_buffer: fs::File::create("log.txt").expect("Unable to open log file!"),
            halted: false,
            halt_bug: false,
            bus: MemoryBus {
                intref: intref.clone(),
                timer: Timer::new(intref.clone()),
                memory: MMU::new(path),
                keys: Joypad::new(intref.clone()),
                apu: APU::new(),
                run_bootrom: false,
                bootrom: vec![0; 256],
                gpu: GPU::new(intref.clone()),
            },
            pc: 0x0000,
            sp: 0x0000,
        }
    }

    pub fn check_vblank(&mut self) -> bool {
        let value = self.bus.gpu.vblank;
        self.bus.gpu.vblank = false;
        value
    }

    pub fn initialize_system(&mut self) {
        /* Power Up Sequence */
        self.regs.set_af(0x01B0);
        self.regs.set_bc(0x0013);
        self.regs.set_de(0x00D8);
        self.regs.set_hl(0x014D);
        self.sp = 0xFFFE;
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
        if self.bus.run_bootrom {
            self.bus.bootrom = fs::read("dmg_boot.bin").unwrap();

            // Put the bootrom in the rom memory
            for item in 0..=0xFF {
                self.bus.write_byte(item as u16, self.bus.bootrom[item]);
            }
        } else {
            self.pc = 0x100;
            self.initialize_system();
        }
    }

    pub fn run_bootrom(&mut self) {
        let mut current_cycles: u32 = 0;

        while current_cycles < MAX_CYCLES {

            /* Check for interrupts */
            let cycles: u32;
            cycles = self.process_interrupts();

            if cycles != 0 {
                current_cycles += cycles as u32;
                continue
            } else if self.halted {
                current_cycles += 4;
                continue
            } else {

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

                let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
                //print!("{}", description);
                if self.log {
                    self.log_buffer.write(format!("PC:{:X} Instr:{} AF:{:X} BC:{:X} DE:{:X} HL:{:X}\n", 
                    self.pc, description, self.regs.get_af(), self.regs.get_bc(), self.regs.get_de(), self.regs.get_hl()).as_bytes()).expect("Unable to write!");
                }
    
                self.pc = next;
                current_cycles += cycles as u32;
                self.bus.timer.update_timers(cycles as u32);
                self.bus.gpu.update_graphics(cycles as u32);
                self.process_interrupts();
    
                if next > 0xFF {
                    self.bus.run_bootrom = false;
                    println!("Bootrom finished!");
                    self.initialize_system();
                    break;
                }
            }
        }
    }

    pub fn update_emulator(&mut self) {
        self.step_cycles = 0;

        while self.step_cycles < MAX_CYCLES {
            let mut cycles: u32;

            if self.bus.intref.borrow().interrupt_delay {
                self.icount += 1;
                if self.icount == 2{
                    self.bus.intref.borrow_mut().interrupt_delay = false;
                    self.bus.intref.borrow_mut().interrupt_master_enable = true;
                }
            }

            /* Check for interrupts */
            cycles = self.process_interrupts();

            if cycles != 0 {
                self.step_cycles += cycles as u32;
            } else if self.halted {
                self.step_cycles += 4;
            } else {
                /* Execute an instruction */
                cycles = self.execute_instruction();
                self.step_cycles += cycles as u32;
            }
    
            // MMU Next 
            self.bus.timer.update_timers(cycles);
            self.bus.gpu.update_graphics(cycles + 8);
        }
    }

    #[rustfmt::skip]
    fn process_interrupts(&mut self) -> u32 {

        let mut cycles = 0;

        if !self.halted && !self.bus.intref.borrow().interrupt_master_enable { return 0; }

        let fired = self.bus.intref.borrow().interrupt_enable & self.bus.intref.borrow().interrupt_flag;
        if fired == 0x00 { return 0; }

        if self.halted {
            cycles += 4;
        }

        self.halted = false;
        if !self.bus.intref.borrow().interrupt_master_enable {
            return 0; 
        }
        self.bus.intref.borrow_mut().interrupt_master_enable = false;

        let flag = self.bus.intref.borrow().interrupt_flag & !(1 << fired.trailing_zeros());
        self.bus.intref.borrow_mut().interrupt_flag = flag;

        self.bus.write_byte(self.sp.wrapping_sub(1), (self.pc >> 8) as u8);
        self.bus.write_byte(self.sp.wrapping_sub(2), (self.pc & 0xFF) as u8);
        self.sp = self.sp.wrapping_sub(2);

        self.pc = 0x40 | ((fired.trailing_zeros() as u16) << 3);
        cycles + 20
    }

    pub fn execute_instruction(&mut self) -> u32 {
        let mut instruction = self.bus.read_byte(self.pc);

        if self.halt_bug {
            self.halt_bug = false;
            self.pc = self.pc.wrapping_sub(1);
        }

        let prefixed = instruction == 0xCB;
        if prefixed {
            instruction = self.bus.read_byte(self.pc + 1);
        }

        let (next, cycles) = if let Some(instruction) =
            Instructions::from_byte(instruction, prefixed)
        {
            self.decode_instruction(instruction)
        } else {
            let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
            panic!("Unknown instruction found! Opcode: {}", description);
        };

        let description = format!("0x{}{:X}", if prefixed { "CB" } else { "" }, instruction);
        //print!("{} ", description);

        if self.log {
            self.log_buffer.write(format!("PC:{:X} Instr:{} AF:{:X} BC:{:X} DE:{:X} HL:{:X}\n", 
            self.pc, description, self.regs.get_af(), self.regs.get_bc(), self.regs.get_de(), self.regs.get_hl()).as_bytes()).expect("Unable to write!");
        }

        self.pc = next;
        cycles as u32
    }

    fn decode_instruction(&mut self, instruction: Instructions) -> (u16, u8) {
        match instruction {

            Instructions::DAA() => {

                let mut carry = false;
                if !self.regs.f.subtract {  // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                    if self.regs.f.carry || self.regs.a > 0x99 { 
                        self.regs.a = self.regs.a.wrapping_add(0x60); 
                        carry = true; 
                    }
                    if self.regs.f.half_carry || (self.regs.a & 0x0F) > 0x09 { 
                        self.regs.a = self.regs.a.wrapping_add(0x06); 
                    }
                } else if self.regs.f.carry { 
                    carry = true;
                    self.regs.a = self.regs.a.wrapping_add(if self.regs.f.half_carry {0x9A} else {0xA0}); 
                } else if self.regs.f.half_carry {
                    self.regs.a = self.regs.a.wrapping_add(0xFA);
                }

                  self.regs.f.carry = carry;
                  self.regs.f.zero = self.regs.a == 0;
                  self.regs.f.half_carry = false;

                (self.pc.wrapping_add(1), 4)
            }

            Instructions::RETI() => {
                self.bus.intref.borrow_mut().interrupt_master_enable = true;
                let value = self.pop();
                (value, 16)
            }

            Instructions::DI() => {
                self.bus.intref.borrow_mut().interrupt_delay = false;
                self.bus.intref.borrow_mut().interrupt_master_enable = false;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::EI() => {
                self.icount = 0;
                self.bus.intref.borrow_mut().interrupt_delay = true;
                (self.pc.wrapping_add(1), 4)
            }

            Instructions::HALT() => {

                let bug = (self.bus.intref.borrow().interrupt_enable & self.bus.intref.borrow().interrupt_enable & 0x1F) != 0;
                
                if !self.bus.intref.borrow().interrupt_master_enable && bug {
                    //halt bug - halt mode is NOT entered. CPU fails to increase PC after executing next instruction
                    self.halt_bug = true;
                } else if !self.bus.intref.borrow().interrupt_master_enable && !bug {
                    self.halt_bug = false;
                    self.halted = true;
                    //halt mode is entered but when IF flag is set and the corresponding IE flag is 
                    //also set, the CPU doesn't jump to the interrupt vector, it just keeps going
                } else {
                    //normal ime == 1
                    self.halt_bug = false;
                    self.halted = true;
                }

                (self.pc.wrapping_add(1), 4)
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

                self.push(self.pc.wrapping_add(1));
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
                let r = (self.regs.a << 1).wrapping_add(self.regs.f.carry as u8);
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
                    byte = (byte << 1) | flag_c;
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
                self.regs.f.zero = false;
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
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.a = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.a == 0;
                    }

                    ArithmeticSource::B => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.b = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.b == 0;
                    }

                    ArithmeticSource::C => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.c = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.c == 0;
                    }

                    ArithmeticSource::D => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.d = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.d == 0;
                    }

                    ArithmeticSource::E => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.e = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.e == 0;
                    }

                    ArithmeticSource::H => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.h = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.h == 0;
                    }

                    ArithmeticSource::L => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        self.regs.l = if carry { 0x80 | (value >> 1) } else { value >> 1 };
                        self.regs.f.zero = self.regs.l == 0;
                    }

                    ArithmeticSource::HLAddr => {
                        let carry = value & 0x01 == 0x01;
                        self.regs.f.carry = carry;
                        value = if carry { 0x80 | (value >> 1) } else {value >> 1 };
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
                    let carry = value & 0x01 == 0x01;
                    value = if self.regs.f.carry { 0x80 | (value >> 1) } else { value >> 1 };
                    self.bus.write_byte(self.regs.get_hl(), value);
                    self.regs.f.carry = carry;
                    self.regs.f.subtract = false;
                    self.regs.f.half_carry = false;
                    self.regs.f.zero = value == 0;
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

                let carry = reg & 0x01 == 0x01;
                let new_value = if self.regs.f.carry { 0x80 | (reg >> 1) } else { reg >> 1 };

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

                self.regs.f.carry = carry;
                self.regs.f.zero = new_value == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;

                (self.pc.wrapping_add(2), 8)
            }

            Instructions::RRA() => {
                let carry = self.regs.a & 0x01 == 0x01;
                let new_value = if self.regs.f.carry { 0x80 | (self.regs.a >> 1) } else { self.regs.a >> 1 };
                self.regs.f.zero = false;
                self.regs.a = new_value;
                self.regs.f.carry = carry;
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
                        self.regs.set_hl(self.regs.get_hl().wrapping_sub(1));
                    }
                    IncDecTarget::BC => {
                        self.regs.set_bc(self.regs.get_bc().wrapping_sub(1));
                    }
                    IncDecTarget::DE => {
                        self.regs.set_de(self.regs.get_de().wrapping_sub(1));
                    }
                    IncDecTarget::SP => {
                        self.sp = self.sp.wrapping_sub(1);
                    }
                }

                match target {

                    IncDecTarget::HLAddr => (self.pc.wrapping_add(1), 12),

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
                    if (target == LoadOtherTarget::A8) && (source == LoadOtherSource::A) {
                        // E0
                        let a = 0xFF00 | u16::from(self.read_next_byte());
                        self.bus.write_byte(a, self.regs.a);
                        (self.pc.wrapping_add(2), 12)
                    } else if (target == LoadOtherTarget::CAddress) && (source == LoadOtherSource::A) {
                        // E2
                        let c = 0xFF00 | u16::from(self.regs.c);
                        self.bus.write_byte(c, self.regs.a);
                        (self.pc.wrapping_add(1), 8)
                    } else if (target == LoadOtherTarget::A) && (source == LoadOtherSource::A8) {
                        // F0
                        let a = 0xFF00 | u16::from(self.read_next_byte());
                        self.regs.a = self.bus.read_byte(a);
                        (self.pc.wrapping_add(2), 12)
                    } else if (target == LoadOtherTarget::A) && (source == LoadOtherSource::CAddress) {
                        // F2
                        let a = 0xFF00 | u16::from(self.regs.c);
                        self.regs.a = self.bus.read_byte(a);
                        (self.pc.wrapping_add(1), 8)
                    } else { unreachable!() }
                },
                _ => unreachable!(),
            },

            Instructions::LD(load_type) => match load_type {
                LoadType::Word(target, source) => {
                    let source_value = match source {
                        LoadWordSource::D16 => self.read_next_word(),
                        LoadWordSource::SP => self.sp,
                        LoadWordSource::HL => self.regs.get_hl(),
                        LoadWordSource::SPr8 => i16::from(self.read_next_byte() as i8) as u16,
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
                            if source == LoadWordSource::SPr8 {
                                self.regs.f.carry = ((self.sp & 0xFF) + (source_value & 0xFF)) > 0xFF;
                                self.regs.f.half_carry = ((self.sp & 0xF) + (source_value & 0xF)) > 0xF;
                                self.regs.f.subtract = false;
                                self.regs.f.zero = false;
                                self.regs.set_hl(self.sp.wrapping_add(source_value));
                            } else {
                                self.regs.set_hl(source_value);
                            }
                            
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
                    StackTarget::AF => self.regs.set_af(result & 0xFFF0),
                    StackTarget::DE => self.regs.set_de(result),
                    StackTarget::HL => self.regs.set_hl(result),
                };

                (self.pc.wrapping_add(1), 12)
            }

            Instructions::CP(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };

                self.sub(source_value);

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::OR(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };

                self.regs.a |= source_value;
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = false;

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::XOR(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };

                self.regs.a ^= source_value;
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = false;
                self.regs.f.carry = false;

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::AND(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };

                self.regs.a &= source_value;
                self.regs.f.zero = self.regs.a == 0;
                self.regs.f.subtract = false;
                self.regs.f.half_carry = true;
                self.regs.f.carry = false;

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::SUB(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };
                self.regs.a = self.sub(source_value);
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

                self.regs.f.carry = (reg & 0x80) >> 7 == 0x01;
                let new_value = reg << 1;

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

                self.regs.f.carry = reg & 0x01 == 0x01;
                let new_value = (reg >> 1) | (reg & 0x80);

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
                    value = (value >> 4) | (value << 4);
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

                let new_value = (reg >> 4) | (reg << 4);

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
                    (self.pc.wrapping_add(2), 12)
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

            Instructions::SBC(source) => {
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
                    ArithmeticSource::I8 => unreachable!(),
                };

                let flag_carry = if self.regs.f.carry {1} else {0};
                let r = self.regs.a.wrapping_sub(source_value).wrapping_sub(flag_carry);
                self.regs.f.carry = u16::from(self.regs.a) < (u16::from(source_value) + u16::from(flag_carry));
                self.regs.f.half_carry = (self.regs.a & 0xF) < ((source_value & 0xF) + flag_carry);
                self.regs.f.subtract = true;
                self.regs.f.zero = r == 0x00;
                self.regs.a = r;

                match source {
                    ArithmeticSource::U8 => (self.pc.wrapping_add(2), 8),
                    ArithmeticSource::HLAddr => (self.pc.wrapping_add(1), 8),
                    _ => (self.pc.wrapping_add(1), 4),
                }
            }

            Instructions::ADC(source) => {
                let source_value = match source {
                    ArithmeticSource::A => self.regs.a,
                    ArithmeticSource::B => self.regs.b,
                    ArithmeticSource::C => self.regs.c,
                    ArithmeticSource::D => self.regs.d,
                    ArithmeticSource::E => self.regs.e,
                    ArithmeticSource::H => self.regs.h,
                    ArithmeticSource::L => self.regs.l,
                    ArithmeticSource::HLAddr => {
                        self.bus.timer.update_timers(8);
                        self.bus.read_byte(self.regs.get_hl())
                    },
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => unreachable!(),
                };

                let flag_carry = if self.regs.f.carry {1} else {0};
                let r = self.regs.a.wrapping_add(source_value).wrapping_add(flag_carry);
                self.regs.f.carry = (u16::from(self.regs.a) + u16::from(source_value) + u16::from(flag_carry)) > 0xFF;
                self.regs.f.half_carry = ((self.regs.a & 0xF) + (source_value & 0xF) + (flag_carry & 0xF)) > 0xF;
                self.regs.f.subtract = false;
                self.regs.f.zero = r == 0x0;
                self.regs.a = r;

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

                let reg = self.regs.get_hl();
                let sum = reg.wrapping_add(source_value);
                self.regs.f.carry = reg > (0xFFFF - source_value);
                self.regs.f.subtract = false;
                self.regs.f.half_carry = (reg & 0x07FF) + (source_value & 0x07FF) > 0x07FF;
                self.regs.set_hl(sum);
                (self.pc.wrapping_add(1), 8)
            }

            Instructions::ADD(target, source) => {
                match source {
                    ArithmeticSource::I8 => {
                        /* ADD SP, r8 */
                        let sval = self.read_next_byte();
                        let source_value = i16::from(self.read_next_byte() as i8) as u16;
                        let sp_value = self.sp;
                        self.regs.f.carry = ((sp_value & 0xFF) + (sval & 0xFF) as u16) > 0xFF;
                        self.regs.f.half_carry = ((sp_value & 0xF) + (sval & 0xF) as u16) > 0xF;
                        self.regs.f.subtract = false;
                        self.regs.f.zero = false;
                        self.sp = sp_value.wrapping_add(source_value);
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
                    ArithmeticSource::HLAddr => {
                        self.bus.timer.update_timers(8);
                        self.bus.read_byte(self.regs.get_hl())
                    },
                    ArithmeticSource::U8 => self.read_next_byte(),
                    _ => unreachable!(),
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
        self.regs.f.half_carry = (*register & 0xF) == 0xF;
        let new_value = (*register).wrapping_add(1);
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = false;
        new_value as u8
    }

    fn dec(&mut self, register: &u8) -> u8 {
        self.regs.f.half_carry = (*register & 0xF) == 0x00;
        let new_value = (*register).wrapping_sub(1);
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = true;
        new_value as u8
    }

    fn sub(&mut self, value: u8) -> u8 {
        let new_value = self.regs.a.wrapping_sub(value);
        self.regs.f.carry = u16::from(self.regs.a) < u16::from(value);
        self.regs.f.half_carry = (self.regs.a & 0xF) < (value & 0xF);
        self.regs.f.subtract = true;
        self.regs.f.zero = new_value == 0x0;
        new_value
    }

    fn add(&mut self, value: u8) -> u8 {
        let a = self.regs.a;
        let new_value = a.wrapping_add(value);
        self.regs.f.carry = (u16::from(a) + u16::from(value)) > 0xff;
        self.regs.f.zero = new_value == 0;
        self.regs.f.subtract = false;
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
        self.bus.write_byte(self.sp, (value & 0xFF) as u8);
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

    fn read_next_byte(&mut self) -> u8 {
        self.bus.read_byte(self.pc + 1)
    }
}