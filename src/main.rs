extern crate sdl2;
extern crate gl;

use std::fs;
use std::fs::File;
use std::io::Read;
use std::str;
use std::time::Duration;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

mod system;

fn main() {
    let filename = "tetris.gb";
    let mut f = File::open(&filename).expect("File not Found!");
    let metadata = fs::metadata(&filename).expect("");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("Buffer Overflow");

    let mut cpu = system::cpu::CPU::new(buffer);
    cpu.initialize_system();

    /* Initialize SDL */
    let sdl_context = sdl2::init().unwrap();
    let video_system = sdl_context.video().unwrap();

    let main_window = video_system
        .window("Srikur's GB Emulator", 166, 144)
        .position_centered()
        .opengl()
        .build()
        .unwrap();

    let _gl_context = main_window.gl_create_context().unwrap();
    let _gl = gl::load_with(|s| video_system.gl_get_proc_address(s) as *const std::os::raw::c_void);

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
        cpu.update_emulator();

        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => break 'main,
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'main
                },
                Event::KeyDown { keycode: Some(Keycode::Z), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(4);
                },
                Event::KeyDown { keycode: Some(Keycode::X), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(5);
                },
                Event::KeyDown { keycode: Some(Keycode::Return), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(7);
                },
                Event::KeyDown { keycode: Some(Keycode::Backspace), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(6);
                },
                Event::KeyDown { keycode: Some(Keycode::Right), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(0);
                },
                Event::KeyDown { keycode: Some(Keycode::Left), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(1);
                },
                Event::KeyDown { keycode: Some(Keycode::Up), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(2);
                },
                Event::KeyDown { keycode: Some(Keycode::Down), ..} => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(3);
                },
                Event::KeyUp { keycode: Some(Keycode::Z), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(4);
                },
                Event::KeyUp { keycode: Some(Keycode::X), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(5);
                },
                Event::KeyUp { keycode: Some(Keycode::Return), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(7);
                },
                Event::KeyUp { keycode: Some(Keycode::Backspace), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(6);
                },
                Event::KeyUp { keycode: Some(Keycode::Right), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(0);
                },
                Event::KeyUp { keycode: Some(Keycode::Left), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(1);
                },
                Event::KeyUp { keycode: Some(Keycode::Up), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(2);
                },
                Event::KeyUp { keycode: Some(Keycode::Down), ..} => {
                    println!("Key Released!");
                    cpu.set_key_released(3);
                },
                _ => {}
            }
        }
        main_window.gl_swap_window();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
