extern crate sdl2;

use std::fs;
use std::fs::File;
use std::io::Read;
use std::str;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;

mod system;

fn init_sdl() {
    let sdl_context = sdl2::init().unwrap();
    let video_system = sdl_context.video().unwrap();

    let main_window = video_system
        .window("Srikur's GB Emulator", 166, 144)
        .position_centered()
        .opengl()
        .build()
        .map_err(|e| e.to_string());
    
    
}

fn main() {
    let filename = "blue.gb";
    let mut f = File::open(&filename).expect("File not Found!");
    let metadata = fs::metadata(&filename).expect("");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("Buffer Overflow");

    let mut cpu = system::cpu::CPU {
        ime: false,
        is_halted: false,
        bus: system::cpu::MemoryBus {
            memory: system::cpu::memory::MMU {
                bios: [0; 0x100],
                rom: buffer,
                cartridge_type: 0,
                wram: [0; 0x2000],
                eram: [0; 0x2000],
                zram: [0; 0x80],
                interrupt_enable: 0,
                interrupt_flag: 0,
            },
            keys: system::cpu::keys::Keys {
                rows: [0; 2],
                column: 0,
            },
            gpu: system::cpu::gpu::GPU {
                vram: [0; system::cpu::gpu::VRAM_SIZE],
                oam: [0; 0xA0],
                stat: 0,
                bg_palette: [0; 4],
                obp0_palette: [0; 4],
                obp1_palette: [0; 4],
                tileset: [system::cpu::gpu::empty_tile(); 384],
                screen_graphics: false,
                scroll_x: 0,
                scroll_y: 0,
                current_line: 0,
                scanline_counter: 0,
                background_palette: system::cpu::gpu::BackgroundPalette {
                    display: false,
                    sprite_size: false,
                    sprites: false,
                    switch_background: false,
                    tile_map: false,
                    tile_set: false,
                    window: false,
                    window_tile_map: false,
                },
            },
        },
        regs: system::cpu::Registers {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            f: system::cpu::FlagsRegister {
                zero: true,
                subtract: false,
                half_carry: true,
                carry: true,
            },
            h: 0x01,
            l: 0x4D,
        },
        pc: 0x100,
        sp: 0xFFFE,
    };

    /* Power Up Sequence */
    cpu.bus.write_byte(0xFF05, 0x00);
    cpu.bus.write_byte(0xFF06, 0x00);
    cpu.bus.write_byte(0xFF07, 0x00);
    cpu.bus.write_byte(0xFF10, 0x80);
    cpu.bus.write_byte(0xFF11, 0xBF);
    cpu.bus.write_byte(0xFF12, 0xF3);
    cpu.bus.write_byte(0xFF14, 0xBF);
    cpu.bus.write_byte(0xFF16, 0x3F);
    cpu.bus.write_byte(0xFF17, 0x00);
    cpu.bus.write_byte(0xFF19, 0xBF);
    cpu.bus.write_byte(0xFF1A, 0x7F);
    cpu.bus.write_byte(0xFF1B, 0xFF);
    cpu.bus.write_byte(0xFF1C, 0x9F);
    cpu.bus.write_byte(0xFF1E, 0xBF);
    cpu.bus.write_byte(0xFF20, 0xFF);
    cpu.bus.write_byte(0xFF21, 0x00);
    cpu.bus.write_byte(0xFF22, 0x00);
    cpu.bus.write_byte(0xFF23, 0xBF);
    cpu.bus.write_byte(0xFF24, 0x77);
    cpu.bus.write_byte(0xFF25, 0xF3);
    cpu.bus.write_byte(0xFF26, 0xF1);
    cpu.bus.write_byte(0xFF40, 0x91);
    cpu.bus.write_byte(0xFF42, 0x00);
    cpu.bus.write_byte(0xFF43, 0x00);
    cpu.bus.write_byte(0xFF45, 0x00);
    cpu.bus.write_byte(0xFF47, 0xFC);
    cpu.bus.write_byte(0xFF48, 0xFF);
    cpu.bus.write_byte(0xFF49, 0xFF);
    cpu.bus.write_byte(0xFF4A, 0x00);
    cpu.bus.write_byte(0xFF4B, 0x00);
    cpu.bus.write_byte(0xFFFF, 0x00);

    /* Initialize SDL */
    init_sdl();

    /* Get ROM info */
    let title = str::from_utf8(&cpu.bus.memory.rom[0x134..0x140]);
    let cgb_flag = &cpu.bus.memory.rom[0x143];
    let sgb_flag = &cpu.bus.memory.rom[0x146];
    cpu.bus.memory.cartridge_type = cpu.bus.memory.rom[0x147];

    println!("------ROM Info------");
    println!("Title: {}", title.unwrap());
    match cgb_flag {
        0x80 => println!("Game supports CGB functions, but works on old gameboys also"),
        0xC0 => println!("Game works on CGB only"),
        _ => println!("Unknown CGB Flag"),
    }
    match sgb_flag {
        0x00 => println!("No SGB functions"),
        0x03 => println!("Game supports SGB functions"),
        _ => println!("Unknown SGB Flag"),
    }

    let quit: bool = false;

    while !quit {
        cpu.emulate_cycle();
    }
}
