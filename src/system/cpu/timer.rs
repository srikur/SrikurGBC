pub const DIVIDER_REGISTER: usize = 0xFF04;
pub const TIMA: usize = 0xFF05;
pub const TMA: usize = 0xFF06;
pub const TMC: usize = 0xFF07;

pub const MAX_CYCLES: u32 = 69905;
//pub const CLOCK_SPEED: u32 = 4194304;

pub struct Timer {
    pub timer_counter_tima: u32,
    pub clock_counter: u16,
    pub divider_register: u32,
    pub divider_counter: u32,
    pub timer_modulo_tma: u32,
    pub input_clock_speed: u16,
    pub clock_enabled: bool,
}
