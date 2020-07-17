extern crate sdl2;

mod system;

use std::fs;
use std::fs::File;
use std::io::Read;
use std::str;
use std::io::prelude::*;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

#[rustfmt::skip]
fn main() {
    let filename = "tetris.gb";
    let mut f = File::open(&filename).expect("File not Found!");
    let metadata = fs::metadata(&filename).expect("");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("Buffer Overflow");

    let mut cpu = system::cpu::CPU::new(buffer);
    cpu.bus.run_bootrom = false; // Toggle this to select whether the bootrom should run
    cpu.log = false; // Toggle this to select whether to print trace to log
    cpu.initialize_bootrom();

    /* Initialize SDL */
    let sdl_context = sdl2::init().unwrap();
    let video_system = sdl_context.video().unwrap();

    let main_window = video_system
        .window("Srikur's GB Emulator", 160, 144)
        .resizable()
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = main_window.into_canvas().build().unwrap();

    /* Get ROM info */
    let title = str::from_utf8(&cpu.bus.memory.game_rom[0x134..0x140]);
    let cgb_flag = &cpu.bus.memory.game_rom[0x143];
    let sgb_flag = &cpu.bus.memory.game_rom[0x146];
    cpu.bus.memory.cartridge_type = cpu.bus.memory.game_rom[0x147];

    match cpu.bus.memory.cartridge_type {
        1 => cpu.bus.memory.mbc1 = true,
        2 => cpu.bus.memory.mbc1 = true,
        3 => cpu.bus.memory.mbc1 = true,
        5 => cpu.bus.memory.mbc2 = true,
        6 => cpu.bus.memory.mbc2 = true,
        _ => { /* Need to support more memory banking */ }
    }

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

    // Main Game Loop
    let mut event_pump = sdl_context.event_pump().unwrap();
    'main: loop {
        if cpu.bus.run_bootrom {
            cpu.run_bootrom();
        } else {
            cpu.update_emulator();
        }

        // Render hopefully
        if cpu.check_vblank() {
            for scanline in 0..cpu.bus.gpu.screen_data.len() {
                for pixel in 0..cpu.bus.gpu.screen_data[scanline].len() {
                    canvas.set_draw_color(Color::RGB(cpu.bus.gpu.screen_data[scanline][pixel][0], 
                        cpu.bus.gpu.screen_data[scanline][pixel][1], 
                        cpu.bus.gpu.screen_data[scanline][pixel][2]));
                    let result = canvas.fill_rect(Rect::new(pixel as i32, scanline as i32, 1, 1));
    
                    if result.is_err() {
                        panic!("Unable to draw :(");
                    }
                }
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => break 'main,
                Event::KeyDown {keycode: Some(Keycode::Escape),..} => break 'main,
                Event::KeyDown {keycode: Some(Keycode::Z),..} => { cpu.key_down(system::cpu::joypad::Keys::A); }
                Event::KeyDown {keycode: Some(Keycode::X),..} => { cpu.key_down(system::cpu::joypad::Keys::B); }
                Event::KeyDown {keycode: Some(Keycode::Return),..} => {cpu.key_down(system::cpu::joypad::Keys::Start)}
                Event::KeyDown {keycode: Some(Keycode::Backspace),..} => {cpu.key_down(system::cpu::joypad::Keys::Select)}
                Event::KeyDown {keycode: Some(Keycode::Right),..} => {cpu.key_down(system::cpu::joypad::Keys::Right)}
                Event::KeyDown { keycode: Some(Keycode::N), .. } => {
                    let mut file = File::create("vram_dump").unwrap();
                    let _result = file.write_all(&cpu.bus.gpu.vram);
                    if _result.is_ok() {
                        println!("Finished dumping VRAM");
                    }
                },
                Event::KeyDown {keycode: Some(Keycode::Left),..} => {cpu.key_down(system::cpu::joypad::Keys::Left)}
                Event::KeyDown {keycode: Some(Keycode::Up),..} => {cpu.key_down(system::cpu::joypad::Keys::Up);}
                Event::KeyDown {keycode: Some(Keycode::Down),..} => {cpu.key_down(system::cpu::joypad::Keys::Down);}
                Event::KeyUp {keycode: Some(Keycode::Z),..} => {cpu.key_up(system::cpu::joypad::Keys::A)}
                Event::KeyUp {keycode: Some(Keycode::X),..} => {cpu.key_up(system::cpu::joypad::Keys::B)}
                Event::KeyUp {keycode: Some(Keycode::Return),..} => {cpu.key_up(system::cpu::joypad::Keys::Start)}
                Event::KeyUp {keycode: Some(Keycode::Backspace),..} => {cpu.key_up(system::cpu::joypad::Keys::Select)}
                Event::KeyUp {keycode: Some(Keycode::Right),..} => {cpu.key_up(system::cpu::joypad::Keys::Right)}
                Event::KeyUp {keycode: Some(Keycode::Left),..} => {cpu.key_up(system::cpu::joypad::Keys::Left)}
                Event::KeyUp {keycode: Some(Keycode::Up),..} => {cpu.key_up(system::cpu::joypad::Keys::Up)}
                Event::KeyUp {keycode: Some(Keycode::Down),..} => {cpu.key_up(system::cpu::joypad::Keys::Down)}
                _ => {}
            }
        }

        canvas.present();
        //::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}