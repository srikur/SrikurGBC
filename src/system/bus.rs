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
}

impl MemoryBus {
    pub fn read_byte(&self, address: u16) -> u8 {
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

            0xFF01..=0xFF02 => self.serial.read_serial(address),

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

            0xFF01..=0xFF02 => self.serial.write_serial(address, value),

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