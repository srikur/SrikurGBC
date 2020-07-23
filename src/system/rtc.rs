use std::time::SystemTime;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct RealTimeClock {
    pub s: u8,
    pub m: u8,
    pub h: u8,
    pub dl: u8,
    pub dh: u8,
    pub zero: u64,
    pub savepath: PathBuf,
}

impl RealTimeClock {
    pub fn new(savepath: impl AsRef<Path>) -> Self {
        let zero = match std::fs::read(savepath.as_ref()) {
            Ok(ok) => {
                let mut b: [u8; 8] = Default::default();
                b.copy_from_slice(&ok);
                u64::from_be_bytes(b)
            }
            Err(_) => SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        Self {
            zero,
            s: 0,
            m: 0,
            h: 0,
            dl: 0,
            dh: 0,
            savepath: savepath.as_ref().to_path_buf(),
        }
    }

    pub fn tick(&mut self) {
        let d = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - self.zero;

        self.s = (d % 60) as u8;
        self.m = (d / 60 % 60) as u8;
        self.h = (d / 3600 % 24) as u8;
        let days = (d / 3600 / 24) as u16;
        self.dl = (days % 256) as u8;
        match days {
            0x0000..=0x00ff => {}
            0x0100..=0x01ff => {
                self.dh |= 0x01;
            }
            _ => {
                self.dh |= 0x01;
                self.dh |= 0x80;
            }
        }
    }

    pub fn rtc_save(&mut self) {
        if self.savepath.to_str().unwrap().is_empty() {
            return;
        }
        File::create(self.savepath.clone())
            .and_then(|mut f| f.write_all(&self.zero.to_be_bytes()))
            .unwrap()
    }

    pub fn read_rtc(&self, address: u16) -> u8 {
        match address {
            0x08 => self.s,
            0x09 => self.m,
            0x0a => self.h,
            0x0b => self.dl,
            0x0c => self.dh,
            _ => unreachable!(),
        }
    }

    pub fn write_rtc(&mut self, address: u16, value: u8) {
        match address {
            0x08 => self.s = value,
            0x09 => self.m = value,
            0x0a => self.h = value,
            0x0b => self.dl = value,
            0x0c => self.dh = value,
            _ => unreachable!(),
        }
    }
}