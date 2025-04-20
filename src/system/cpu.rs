use super::audio::*;
use super::bus::*;
use super::gpu::*;
use super::instructions::*;
use super::interrupts::*;
use super::joypad::*;
use super::memory::*;
use super::registers::*;
use super::serial::*;
use super::timer::*;
use std::cell::RefCell;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::rc::Rc;

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

#[rustfmt::skip]
pub enum RSTTargets {
    H00, H10, H20, H30, H08, H18, H28, H38,
}

#[rustfmt::skip]
pub enum IncDecTarget {
    A, B, C, D, E, H, L, HL, HLAddr, BC, DE, SP,
}

#[rustfmt::skip]
pub enum Arithmetic16Target {
    HL, BC, DE, SP,
}

#[rustfmt::skip]
pub enum ArithmeticTarget {
    A, SP,
}

#[rustfmt::skip]
#[derive(Eq, PartialEq)]
pub enum ArithmeticSource {
    A, B, C, D, E, H, L, U8, HLAddr, I8,
}

#[derive(PartialEq)]
pub enum JumpTest {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    Always,
}

impl CPU {
    pub fn new(path: impl AsRef<Path>) -> CPU {
        let intref = Rc::new(RefCell::new(Interrupt::new()));

        let mut this = CPU {
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
                serial: Serial::new(intref.clone()),
                keys: Joypad::new(intref.clone()),
                apu: APU::new(),
                hdma: HDMA::new(),
                speed: Speed::Regular,
                speed_shift: false,
                run_bootrom: false,
                bootrom: vec![0; 0x00],
                gpu: GPU::new(intref.clone()),
            },
            pc: 0x0000,
            sp: 0x0000,
        };
        let hardware = match this.bus.memory.cartridge.read_byte(0x143) & 0x80 {
            0x80 => Hardware::CGB,
            _ => Hardware::DMG,
        };
        let code = match hardware {
            Hardware::CGB => "Yes",
            Hardware::DMG => "No",
        };
        println!("CGB Flag: {}", code);
        this.bus.gpu.hardware = hardware;

        this
    }

    pub fn check_vblank(&mut self) -> bool {
        let value = self.bus.gpu.vblank;
        self.bus.gpu.vblank = false;
        value
    }

    pub fn initialize_system(&mut self) {
        /* Power Up Sequence */
        match self.bus.gpu.hardware {
            Hardware::CGB => self.regs.a = 0x11,
            Hardware::DMG => self.regs.a = 0x01,
        }
        self.regs.f = FlagsRegister::from(0xB0);
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
            if self.bus.gpu.hardware == Hardware::CGB {
                let mut file = fs::File::open("cgb_bios.bin").unwrap();
                file.read_to_end(&mut self.bus.bootrom).unwrap();
            } else {
                let mut file = fs::File::open("dmg_boot.bin").unwrap();
                file.read_to_end(&mut self.bus.bootrom).unwrap();
            }

            println!("File length: {:X}", self.bus.bootrom.len());
        } else {
            self.pc = 0x100;
            self.initialize_system();
        }
    }

    pub fn run_bootrom(&mut self) {
        let mut current_cycles: u32 = 0;

        while current_cycles < MAX_CYCLES {
            if self.bus.intref.borrow().interrupt_delay {
                self.icount += 1;
                if self.icount == 2 {
                    self.bus.intref.borrow_mut().interrupt_delay = false;
                    self.bus.intref.borrow_mut().interrupt_master_enable = true;
                }
            }

            /* Check for interrupts */
            let cycles: u32;
            cycles = self.process_interrupts();

            if cycles != 0 {
                current_cycles += cycles as u32;
                continue;
            } else if self.halted {
                current_cycles += 4;
                continue;
            } else {
                let mut instruction = self.bus.read_byte(self.pc);
                let prefixed = instruction == 0xCB;
                if prefixed {
                    instruction = self.bus.read_byte(self.pc.wrapping_add(1));
                }
                let (next, cycles) =
                    if let Some(instruction) = Instructions::from_byte(instruction, prefixed) {
                        self.decode_instruction(instruction)
                    } else {
                        panic!("Unknown instruction found! Opcode!");
                    };

                self.pc = next;
                current_cycles += cycles as u32;
                self.bus.timer.update_timers(cycles as u32);
                self.bus.gpu.update_graphics(cycles as u32 + 8);

                match self.bus.gpu.hardware {
                    Hardware::CGB => {
                        if next == 0x100 {
                            self.bus.run_bootrom = false;
                            self.initialize_system();
                            println!("Bootrom Finished");
                            break;
                        }
                    }
                    Hardware::DMG => {
                        if next > 0xFF {
                            self.bus.run_bootrom = false;
                            self.initialize_system();
                            println!("Bootrom Finished");
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn update_emulator(&mut self) {
        self.step_cycles = 0;

        while self.step_cycles < MAX_CYCLES {
            let mut cycles: u32;

            if self.pc == 0x10 {
                self.bus.change_speed();
            }

            if self.bus.intref.borrow().interrupt_delay {
                self.icount += 1;
                if self.icount == 2 {
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

            // Run HDMA
            let hdma_cycles = self.run_hdma();

            // MMU Next 
            self.bus.timer.update_timers(cycles + (self.bus.speed as u32 * hdma_cycles));
            self.bus.gpu.update_graphics(cycles + 4);
        }
    }

    fn run_hdma(&mut self) -> u32 {
        if !self.bus.hdma.active {
            return 0;
        }
        match self.bus.hdma.mode {
            HDMAMode::GDMA => {
                let length = u32::from(self.bus.hdma.remain) + 1;
                for _ in 0..length {
                    let mem_source = self.bus.hdma.source;
                    for i in 0..0x10 {
                        let byte: u8 = self.bus.read_byte(mem_source + i);
                        self.bus.gpu.write_vram((self.bus.hdma.destination + i) as usize, byte);
                    }
                    self.bus.hdma.source += 0x10;
                    self.bus.hdma.destination += 0x10;
                    if self.bus.hdma.remain == 0 {
                        self.bus.hdma.remain = 0x7f;
                    } else {
                        self.bus.hdma.remain -= 1;
                    }
                }
                self.bus.hdma.active = false;
                length * 8 * 4
            }
            HDMAMode::HDMA => {
                if !self.bus.gpu.hblank {
                    return 0;
                }
                let mem_source = self.bus.hdma.source;
                for i in 0..0x10 {
                    let byte: u8 = self.bus.read_byte(mem_source + i);
                    self.bus.gpu.write_vram((self.bus.hdma.destination + i) as usize, byte);
                }
                self.bus.hdma.source += 0x10;
                self.bus.hdma.destination += 0x10;
                if self.bus.hdma.remain == 0 {
                    self.bus.hdma.remain = 0x7F;
                } else {
                    self.bus.hdma.remain -= 1;
                }
                if self.bus.hdma.remain == 0x7F {
                    self.bus.hdma.active = false;
                }
                32
            }
        }
    }

    #[rustfmt::skip]
    fn process_interrupts(&mut self) -> u32 {

        if !self.halted && !self.bus.intref.borrow().interrupt_master_enable { return 0; }

        let fired = self.bus.intref.borrow().interrupt_enable & self.bus.intref.borrow().interrupt_flag;
        if fired == 0x00 { 
            return 0;
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
        16
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
                    self.regs.a = self.regs.a.wrapping_add(if self.regs.f.half_carry { 0x9A } else { 0xA0 });
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
                let old: u8 = if (self.regs.a & 0x80) != 0 { 1 } else { 0 };
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
                        old = if (self.regs.a & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.a << 1) | old;
                        self.regs.a = new_value;
                        self.regs.f.zero = self.regs.a == 0;
                    }

                    ArithmeticSource::B => {
                        old = if (self.regs.b & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.b << 1) | old;
                        self.regs.b = new_value;
                        self.regs.f.zero = self.regs.b == 0;
                    }

                    ArithmeticSource::C => {
                        old = if (self.regs.c & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.c << 1) | old;
                        self.regs.c = new_value;
                        self.regs.f.zero = self.regs.c == 0;
                    }

                    ArithmeticSource::D => {
                        old = if (self.regs.d & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.d << 1) | old;
                        self.regs.d = new_value;
                        self.regs.f.zero = self.regs.d == 0;
                    }

                    ArithmeticSource::E => {
                        old = if (self.regs.e & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.e << 1) | old;
                        self.regs.e = new_value;
                        self.regs.f.zero = self.regs.e == 0;
                    }

                    ArithmeticSource::H => {
                        old = if (self.regs.h & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.h << 1) | old;
                        self.regs.h = new_value;
                        self.regs.f.zero = self.regs.h == 0;
                    }

                    ArithmeticSource::L => {
                        old = if (self.regs.l & 0x80) != 0 { 1 } else { 0 };
                        self.regs.f.carry = old != 0;
                        new_value = (self.regs.l << 1) | old;
                        self.regs.l = new_value;
                        self.regs.f.zero = self.regs.l == 0;
                    }

                    ArithmeticSource::HLAddr => {
                        let mut byte = self.bus.read_byte(self.regs.get_hl());
                        old = if (byte & 0x80) != 0 { 1 } else { 0 };
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
                    let flag_c = if self.regs.f.carry { 1 } else { 0 };
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
                        value = if carry { 0x80 | (value >> 1) } else { value >> 1 };
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
                }
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

                let flag_carry = if self.regs.f.carry { 1 } else { 0 };
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
                        self.bus.read_byte(self.regs.get_hl())
                    }
                    ArithmeticSource::U8 => self.read_next_byte(),
                    ArithmeticSource::I8 => unreachable!(),
                };

                let flag_carry = if self.regs.f.carry { 1 } else { 0 };
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
                        self.bus.read_byte(self.regs.get_hl())
                    }
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