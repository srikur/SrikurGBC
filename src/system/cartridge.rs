use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use super::rtc::RealTimeClock;

pub struct Cartridge {
    pub savepath: PathBuf,
    pub game_rom: Vec<u8>,
    pub game_ram: Vec<u8>,
    pub ram_enabled: bool,
    pub bank_mode: Mode,
    pub rtc: RealTimeClock,
    pub rom_bank: usize, 
    pub ram_bank: usize,
    pub bank: u8,
    pub mbc: MBC,
}

pub enum Mode {
    Rom,
    Ram,
}

pub enum MBC {
    None,
    MBC1, 
    MBC2, 
    MBC3, 
    MBC5,
}

impl Cartridge {

    pub fn new(path: impl AsRef<Path>) -> Self {

        let mut file = File::open(path.as_ref()).unwrap();
        let mut rom = Vec::new();
        file.read_to_end(&mut rom).unwrap();
        if rom.len() < 0x150 {
            panic!("Missing required info!");
        }

        Cartridge {
            game_rom: rom,
            savepath: path.as_ref().to_path_buf().with_extension("sav"),
            rtc: RealTimeClock::new(path.as_ref().to_path_buf().with_extension("rtc")),
            game_ram: vec![],
            ram_enabled: false,
            bank_mode: Mode::Rom,
            rom_bank: 0x01,
            ram_bank: 0x00,
            bank: 0x01,
            mbc: MBC::None,
        }
    }

    fn load_ram(&self, save_file: impl AsRef<Path>, size: usize) -> Vec<u8> {
        match File::open(save_file) {
            Ok(mut ok) => {
                let mut ram = Vec::new();
                ok.read_to_end(&mut ram).unwrap();
                ram
            }
            Err(_) => vec![0; size],
        }
    }

    pub fn determine_mbc(&mut self) {
        if self.game_rom.len() > self.get_rom_size(self.game_rom[0x148]) { panic!("Incorrect ROM size!") }
        let cart_type = self.game_rom[0x147];
        println!("MBC Value: {:X}", cart_type);
        match cart_type {
            0x00 => self.mbc = MBC::None,
            0x01 => self.mbc = MBC::MBC1,
            0x02 => {
                self.mbc = MBC::MBC1;
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = vec![0; ram_size];
            }
            0x03 => {
                self.mbc = MBC::MBC1;
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = self.load_ram(self.savepath.clone(), ram_size);
            }
            0x05 => {
                self.mbc = MBC::MBC2;
                self.game_ram = vec![0; 0x200];
            }
            0x06 => {
                self.game_ram = self.load_ram(self.savepath.clone(), 0x200);
                self.mbc = MBC::MBC2;
            }
            0x0F => {
                self.mbc = MBC::MBC3;
            }
            0x10 => {
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = self.load_ram(self.savepath.clone(), ram_size);
                self.mbc = MBC::MBC3;
            }
            0x11 => {
                self.mbc = MBC::MBC3;
            }
            0x12 => {
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = vec![0; ram_size];
                self.mbc = MBC::MBC3;
            }
            0x13 => {
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = self.load_ram(self.savepath.clone(), ram_size);
                self.mbc = MBC::MBC3;
            }
            0x19 => {
                self.mbc = MBC::MBC5;
            }
            0x1A => {
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = vec![0; ram_size];
                self.mbc = MBC::MBC5;
            }
            0x1B => {
                let ram_size = self.get_ram_size(self.game_rom[0x149]);
                self.game_ram = self.load_ram(self.savepath.clone(), ram_size);
                self.mbc = MBC::MBC5;
            }
            _ => panic!("Unimplemented MBC Type!"),
        }
    }

    fn get_rom_size(&self, byte: u8) -> usize {
        let bank = 0x4000;
        match byte {
            0x00 => bank * 2,
            0x01 => bank * 4,
            0x02 => bank * 8,
            0x03 => bank * 16,
            0x04 => bank * 32,
            0x05 => bank * 64,
            0x06 => bank * 128,
            0x07 => bank * 256,
            0x08 => bank * 512,
            0x52 => bank * 72,
            0x53 => bank * 80,
            0x54 => bank * 96,
            size => panic!("Unsupported Rom Size: 0x{:02x}", size),
        }
    }

    fn get_ram_size(&self, byte: u8) -> usize {
        match byte {
            0x00 => 0,
            0x01 => 0x400 * 2,
            0x02 => 0x400 * 8,
            0x03 => 0x400 * 32,
            0x04 => 0x400 * 128,
            0x05 => 0x400 * 64,
            size => panic!("Unsupported Ram Size: 0x{:02x}", size),
        }
    }

    fn rom_bank(&self) -> usize {
        match self.bank_mode {
            Mode::Rom => usize::from(self.bank & 0x7F),
            Mode::Ram => usize::from(self.bank & 0x1F), 
        }
    }

    fn ram_bank(&self) -> usize {
        match self.bank_mode {
            Mode::Rom => 0x00,
            Mode::Ram => usize::from((self.bank & 0x60) >> 5), 
        }
    }

    fn read_byte_none(&self, address: usize) -> u8 {
        self.game_rom[address]
    }

    #[rustfmt::skip]
    fn read_byte_mbc1(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => self.game_rom[address],
            0x4000..=0x7FFF => self.game_rom[self.rom_bank() * 0x4000 + address - 0x4000],
            0xA000..=0xBFFF => if self.ram_enabled { self.game_ram[self.ram_bank() * 0x2000 + address - 0xA000] } else {0x00},
            _ => 0xFF,
        }
    }

    #[rustfmt::skip]
    fn read_byte_mbc2(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => self.game_rom[address],
            0x4000..=0x7FFF => self.game_rom[self.rom_bank * 0x4000 + address - 0x4000],
            0xA000..=0xA1FF => if self.ram_enabled { self.game_ram[address - 0xA000] } else {0x00},
            _ => 0xFF,
        }
    }

    #[rustfmt::skip]
    fn read_byte_mbc3(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => self.game_rom[address],
            0x4000..=0x7FFF => self.game_rom[self.rom_bank * 0x4000 + address - 0x4000],
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    if self.ram_bank <= 0x03 {
                        self.game_ram[self.ram_bank * 0x2000 + address - 0xa000]
                    } else {
                        self.rtc.read_rtc(self.ram_bank as u16)
                    }
                } else {
                    0x00
                }
            }
            _ => 0x00,
        }
    }

    #[rustfmt::skip]
    fn read_byte_mbc5(&self, address: usize) -> u8 {
        match address {
            0x0000..=0x3FFF => self.game_rom[address],
            0x4000..=0x7FFF => {
                let i = self.rom_bank * 0x4000 + address - 0x4000;
                self.game_rom[i]
            }
            0xa000..=0xbfff => {
                if self.ram_enabled {
                    let i = self.ram_bank * 0x2000 + address - 0xA000;
                    self.game_ram[i]
                } else {
                    0x00
                }
            }
            _ => 0x00,
        }
    }

    fn write_byte_none(&mut self, _address: usize, _value: u8) {}

    #[rustfmt::skip]
    fn write_byte_mbc1(&mut self, address: usize, value: u8) {
        match address {
            0xA000..=0xBFFF => {
                if self.ram_enabled && (self.game_ram.len() != 0) {
                    let i = self.ram_bank() * 0x2000 + address - 0xA000;
                    self.game_ram[i] = value;
                }
            }
            0x0000..=0x1FFF => { self.ram_enabled = value & 0x0F == 0x0A; }
            0x2000..=0x3FFF => {
                let value = value & 0x1F;
                let value = match value {
                    0x00 => 0x01,
                    _ => value,
                };
                self.bank = (self.bank & 0x60) | value;
            }
            0x4000..=0x5FFF => { self.bank = self.bank & 0x9F | ((value & 0x03) << 5) }
            0x6000..=0x7FFF => match value {
                0x00 => self.bank_mode = Mode::Rom,
                0x01 => self.bank_mode = Mode::Ram,
                def => panic!("Invalid cartridge type {}", def),
            },
            _ => {}
        }
    }

    fn write_byte_mbc2(&mut self, address: usize, value: u8) {
        let value = value & 0x0F;
        match address {
            0xA000..=0xA1FF => {
                if self.ram_enabled {
                    self.game_ram[address - 0xA000] = value;
                }
            }
            0x0000..=0x1FFF => {
                if address & 0x0100 == 0 {
                    self.ram_enabled = value == 0x0a;
                }
            }
            0x2000..=0x3FFF => {
                if address & 0x0100 != 0 {
                    self.rom_bank = value as usize;
                }
            }
            _ => {}
        }
    }

    #[rustfmt::skip]
    fn write_byte_mbc3(&mut self, address: usize, value: u8) {
        match address {
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    if self.ram_bank <= 0x03 {
                        self.game_ram[self.ram_bank * 0x2000 + address - 0xA000] = value;
                    } else {
                        self.rtc.write_rtc(self.ram_bank as u16, value);
                    }
                }
            }
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                let value = (value & 0x7F) as usize;
                let value = match value {
                    0x00 => 0x01,
                    _ => value,
                };
                self.rom_bank = value;
            }
            0x4000..=0x5FFF => { self.ram_bank = (value & 0x0F) as usize; }
            0x6000..=0x7FFF => {
                if value & 0x01 != 0 {
                    self.rtc.tick();
                }
            }
            _ => {}
        }
    }

    fn write_byte_mbc5(&mut self, address: usize, value: u8) {
        match address {
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    let i = self.ram_bank * 0x2000 + address - 0xA000;
                    self.game_ram[i] = value;
                }
            }
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0x100) | (value as usize),
            0x3000..=0x3FFF => self.rom_bank = (self.rom_bank & 0x0FF) | (((value & 0x01) as usize) << 8),
            0x4000..=0x5FFF => self.ram_bank = (value & 0x0F) as usize,
            _ => {}
        }
    }

    pub fn save(&mut self) {
        match self.mbc {
            MBC::None => {},
            MBC::MBC1 => {
                if self.savepath.to_str().unwrap().is_empty() {
                    return;
                }
                File::create(self.savepath.clone())
                    .and_then(|mut f| f.write_all(&self.game_ram))
                    .unwrap()
            },
            MBC::MBC2 => {
                if self.savepath.to_str().unwrap().is_empty() {
                    return;
                }
                File::create(self.savepath.clone())
                    .and_then(|mut f| f.write_all(&self.game_ram))
                    .unwrap()
            },
            MBC::MBC3 => {
                self.rtc.rtc_save();
                if self.savepath.to_str().unwrap().is_empty() {
                    return;
                }
                File::create(self.savepath.clone())
                    .and_then(|mut f| f.write_all(&self.game_ram))
                    .unwrap();
            },
            MBC::MBC5 => {
                if self.savepath.to_str().unwrap().is_empty() {
                    return;
                }
                File::create(self.savepath.clone())
                    .and_then(|mut f| f.write_all(&self.game_ram))
                    .unwrap()
            },
        }
    }

    pub fn read_byte(&self, address: usize) -> u8 {
        match self.mbc {
            MBC::None => self.read_byte_none(address),
            MBC::MBC1 => self.read_byte_mbc1(address),
            MBC::MBC2 => self.read_byte_mbc2(address),
            MBC::MBC3 => self.read_byte_mbc3(address),
            MBC::MBC5 => self.read_byte_mbc5(address),
        }
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        match self.mbc {
            MBC::None => self.write_byte_none(address, value),
            MBC::MBC1 => self.write_byte_mbc1(address, value),
            MBC::MBC2 => self.write_byte_mbc2(address, value),
            MBC::MBC3 => self.write_byte_mbc3(address, value),
            MBC::MBC5 => self.write_byte_mbc5(address, value),
        }
    }
}