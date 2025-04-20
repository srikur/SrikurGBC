extern crate sdl2;

mod system;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::str;
use system::cpu::*;
use system::joypad::Keys;

fn main() {
    let mut rom = String::from("");
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("Gameboy Color Emulator");
        ap.refer(&mut rom)
            .add_argument("rom", argparse::Store, "Rom name");
        ap.parse_args_or_exit();
    }

    let mut cpu = CPU::new(rom);
    cpu.bus.memory.cartridge.determine_mbc();
    cpu.bus.run_bootrom = false; // Toggle this to select whether the bootrom should run
    cpu.log = false; // Toggle this to select whether to print trace to log
    cpu.initialize_bootrom();

    /* Initialize SDL */
    let sdl_context = sdl2::init().unwrap();
    let video_system = sdl_context.video().unwrap();

    let main_window = video_system
        .window("Gameboy Color Emulator", 160, 144)
        .resizable()
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = main_window.into_canvas().build().unwrap();

    /* Get ROM info */
    let title = str::from_utf8(&cpu.bus.memory.cartridge.game_rom[0x134..0x140]);

    println!("------ROM Info------");
    println!("Title: {}", title.unwrap());

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
                    canvas.set_draw_color(Color::RGB(
                        cpu.bus.gpu.screen_data[scanline][pixel][0],
                        cpu.bus.gpu.screen_data[scanline][pixel][1],
                        cpu.bus.gpu.screen_data[scanline][pixel][2],
                    ));
                    let result = canvas.fill_rect(Rect::new(pixel as i32, scanline as i32, 1, 1));

                    if result.is_err() {
                        panic!("Unable to draw :(");
                    }
                }
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    cpu.bus.memory.cartridge.save();
                    break 'main;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    cpu.bus.memory.cartridge.save();
                    break 'main;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Z),
                    ..
                } => {
                    cpu.bus.keys.key_down(Keys::A);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::X),
                    ..
                } => {
                    cpu.bus.keys.key_down(Keys::B);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => cpu.bus.keys.key_down(Keys::Start),
                Event::KeyDown {
                    keycode: Some(Keycode::Backspace),
                    ..
                } => cpu.bus.keys.key_down(Keys::Select),
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => cpu.bus.keys.key_down(Keys::Right),
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => cpu.bus.keys.key_down(Keys::Left),
                Event::KeyDown {
                    keycode: Some(Keycode::Up),
                    ..
                } => {
                    cpu.bus.keys.key_down(Keys::Up);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Down),
                    ..
                } => {
                    cpu.bus.keys.key_down(Keys::Down);
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Z),
                    ..
                } => cpu.bus.keys.key_up(Keys::A),
                Event::KeyUp {
                    keycode: Some(Keycode::X),
                    ..
                } => cpu.bus.keys.key_up(Keys::B),
                Event::KeyUp {
                    keycode: Some(Keycode::Return),
                    ..
                } => cpu.bus.keys.key_up(Keys::Start),
                Event::KeyUp {
                    keycode: Some(Keycode::Backspace),
                    ..
                } => cpu.bus.keys.key_up(Keys::Select),
                Event::KeyUp {
                    keycode: Some(Keycode::Right),
                    ..
                } => cpu.bus.keys.key_up(Keys::Right),
                Event::KeyUp {
                    keycode: Some(Keycode::Left),
                    ..
                } => cpu.bus.keys.key_up(Keys::Left),
                Event::KeyUp {
                    keycode: Some(Keycode::Up),
                    ..
                } => cpu.bus.keys.key_up(Keys::Up),
                Event::KeyUp {
                    keycode: Some(Keycode::Down),
                    ..
                } => cpu.bus.keys.key_up(Keys::Down),
                _ => {}
            }
        }
        canvas.present();
    }
}
