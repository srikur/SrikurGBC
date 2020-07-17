pub const DIVIDER_REGISTER: usize = 0xFF04;
pub const TIMA: usize = 0xFF05;
pub const TMA: usize = 0xFF06;
pub const TAC: usize = 0xFF07;

pub const MAX_CYCLES: u32 = 69905;

pub struct Timer {
    pub tima: u8, // timer counter     
    pub clock_counter: i32,
    pub divider_register: u8,
    pub divider_counter: u16,
    pub tma: u8,
    pub input_clock_speed: u16, // tmc
    pub clock_enabled: bool,
}