use super::interrupts::*;
use super::memory::*;
use super::gpu::*;
use super::joypad::*;
use super::timer::*;
use super::audio::*;
use super::serial::*;
use std::rc::Rc;
use std::cell::RefCell;

pub struct MemoryBus {
    pub intref: Rc<RefCell<Interrupt>>,
    pub memory: MMU,
    pub gpu: GPU,
    pub keys: Joypad,
    pub timer: Timer,
    pub apu: APU,
    pub serial: Serial,
    pub run_bootrom: bool,
    pub bootrom: Vec<u8>,

    // CGB
    pub speed: Speed,
    pub speed_shift: bool,
    pub hdma: HDMA,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Speed {
    Regular = 1,
    Double = 2,
}

impl MemoryBus {
    pub fn read_byte(&self, address: u16) -> u8 {
        let address = address as usize;

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
            VRAM_BEGIN..=VRAM_END => self.gpu.read_vram(address),

            /* Read from Work RAM */
            0xC000..=0xCFFF => self.memory.wram[address - 0xC000],
            0xD000..=0xDFFF => self.memory.wram[address - 0xD000 + 0x1000 * self.memory.wram_bank],
            0xE000..=0xEFFF => self.memory.wram[address - 0xE000],
            0xF000..=0xFDFF => self.memory.wram[address- 0xF000 + 0x1000 * self.memory.wram_bank],

            /* Read from Sprite Attribute Table */
            OAM_BEGIN..=OAM_END => self.gpu.oam[address - OAM_BEGIN],

            /* GPU Registers */
            GPU_REGS_BEGIN..=GPU_REGS_END => self.gpu.read_registers(address),

            /* Read from High RAM */
            HRAM_BEGIN..=HRAM_END => self.memory.hram[address - HRAM_BEGIN],

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

            0xFF01..=0xFF02 => self.serial.read_serial(address),

            0xFF4D => {
                let first = if self.speed == Speed::Double { 0x80 } else { 0x00 };
                let second = if self.speed_shift { 0x01 } else { 0x00 };
                first | second
            }

            0xFF51..=0xFF55 => self.hdma.read_hdma(address as u16), // get hdma

            0xFF68..=0xFF6B => self.gpu.read_registers(address),

            /* WRAM Bank */
            0xFF70 => self.memory.wram_bank as u8,

            _ => 0x00,
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
                self.gpu.write_vram(address, value);
            }

            /* Write to WRAM */
            0xC000..=0xCFFF => self.memory.wram[address - 0xC000] = value,
            0xD000..=0xDFFF => self.memory.wram[address - 0xD000 + 0x1000 * self.memory.wram_bank] = value,
            0xE000..=0xEFFF => self.memory.wram[address - 0xE000] = value,
            0xF000..=0xFDFF => self.memory.wram[address- 0xF000 + 0x1000 * self.memory.wram_bank] = value,

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

            0xFF01..=0xFF02 => self.serial.write_serial(address, value),

            /* Write to High RAM */
            HRAM_BEGIN..=HRAM_END => {
                self.memory.hram[address - HRAM_BEGIN] = value;
            }

            /* Write to Sprite Attribute Table (OAM) */
            OAM_BEGIN..=OAM_END => {
                self.gpu.oam[address - OAM_BEGIN] = value;
            }

            /* Not usable memory */
            0xFEA0..=0xFEFF => return, // Invalid memory location

            0xFF4D => self.speed_shift = (value & 0x01) == 0x01,
            
            0xFF51..=0xFF55 => self.hdma.write_hdma(address as u16, value),

            0xFF68..=0xFF6B => {
                self.gpu.write_registers(address, value)
            },

            /* Change WRAM Bank */
            0xFF70 => {
                self.memory.wram_bank = match value & 0x07 {
                    0 => 1,
                    value => value as usize,
                };
            }

            /* Write to Interrupts Enable Register */
            INTERRUPT_ENABLE => {
                self.intref.borrow_mut().interrupt_enable = value;
            }

            /* Write to GPU registers */
            GPU_REGS_BEGIN..=GPU_REGS_END | 0xFF4F => {
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

            _ => {},
        }

        
    }

    pub fn write_word(&mut self, address: u16, word: u16) {
        let lower = word >> 8;
        let higher = word & 0xFF;
        self.write_byte(address, higher as u8);
        self.write_byte(address + 1, lower as u8);
    }

    pub fn change_speed(&mut self) {
        if self.speed_shift {
            if self.speed == Speed::Double {
                self.speed = Speed::Regular;
            } else {
                self.speed = Speed::Double;
            }
        }
        self.speed_shift = false;
    }
}