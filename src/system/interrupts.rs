pub const INTERRUPT_FLAG: usize = 0xFF0F;
pub const INTERRUPT_ENABLE: usize = 0xFFFF;

pub struct Interrupt {
    pub interrupt_enable: u8,
    pub interrupt_flag: u8,
    pub interrupt_master_enable: bool,
    pub interrupt_delay: bool,
}

#[derive(Clone)]
pub enum Interrupts {
    VBlank,
    LCDStat,
    Timer,
    //Serial,
    Joypad,
}

impl Interrupt {
    pub fn new() -> Self {
        Interrupt {
            interrupt_enable: 0,
            interrupt_flag: 0,
            interrupt_master_enable: true,
            interrupt_delay: false,
        }
    }

    pub fn set_interrupt(&mut self, interrupt: Interrupts) {
        let mask = match interrupt {
            Interrupts::VBlank => 0x01,
            Interrupts::LCDStat => 0x02,
            Interrupts::Timer => 0x04,
            //Interrupts::Serial => 0x08,
            Interrupts::Joypad => 0x10,
        };
        self.interrupt_flag |= mask;
    }
}
