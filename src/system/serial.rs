use std::rc::Rc;
use std::cell::RefCell;
use super::interrupts::*;

pub struct Serial {
    pub intref: Rc<RefCell<Interrupt>>,
    pub data: u8,
    pub control: u8,
}

impl Serial {
    pub fn new(int: Rc<RefCell<Interrupt>>) -> Self {
        Self {
            intref: int,
            data: 0x00,
            control: 0x00,
        }
    }

    pub fn read_serial(&self, address: usize) -> u8 {
        match address {
            0xFF01 => {
                //self.intref.borrow_mut().set_interrupt(Interrupts::Serial);
                self.data
            },
            0xFF02 => self.control,
            _ => unreachable!(),
        }
    }

    pub fn write_serial(&mut self, address: usize, value: u8) {
        match address {
            0xFF01 => {
                self.data = value;
                //self.intref.borrow_mut().set_interrupt(Interrupts::Serial);
            },
            0xFF02 => self.control = value,
            _ => unreachable!(),
        };
    }
}