use super::interrupts::{Interrupt, Interrupts};
use std::rc::Rc;
use std::cell::RefCell;

pub const DIVIDER_REGISTER: usize = 0xFF04;
pub const TIMA: usize = 0xFF05;
pub const TMA: usize = 0xFF06;
pub const TAC: usize = 0xFF07;
pub const MAX_CYCLES: u32 = 69905;

pub struct Timer {
    pub intref: Rc<RefCell<Interrupt>>,
    pub tima: u8, // timer counter     
    pub clock_counter: u32,
    pub divider_register: u8,
    pub divider_counter: u32,
    pub tma: u8,
    pub input_clock_speed: u32, // tmc
    pub clock_enabled: bool,
}

impl Timer {
    pub fn new(int: Rc<RefCell<Interrupt>>) -> Self {
        Timer {
            intref: int,
            tima: 0,
            divider_register: 0,
            divider_counter: 0,
            tma: 0,
            clock_counter: 1024,
            input_clock_speed: 1024,
            clock_enabled: false,
        }
    }

    pub fn update_timers(&mut self, cycles: u32) {

        self.divider_counter += cycles;
        while self.divider_counter > 256 {
            self.divider_register = self.divider_register.wrapping_add(1);
            self.divider_counter -= 256;
        }

        if self.clock_enabled {
            self.clock_counter += cycles;
            let rs = self.clock_counter / self.input_clock_speed;
            self.clock_counter = self.clock_counter % self.input_clock_speed;
            for _ in 0..rs {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0x00 {
                    self.tima = self.tma;
                    self.intref.borrow_mut().set_interrupt(Interrupts::Timer);
                }
            }
        }
    }
}