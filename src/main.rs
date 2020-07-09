extern crate sdl2;

mod system;

use std::fs;
use std::fs::File;
use std::io::Read;
use std::str;
use std::io::prelude::*;
use std::time::Duration;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;

fn main() {
    let filename = "instr_timing.gb";
    let mut f = File::open(&filename).expect("File not Found!");
    let metadata = fs::metadata(&filename).expect("");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("Buffer Overflow");

    let mut cpu = system::cpu::CPU::new(buffer);
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
        if !cpu.bus.bootrom_run {
            cpu.run_bootrom();
        } else {
            cpu.update_emulator();
        }

        // Render hopefully
        for scanline in 0..cpu.bus.gpu.screen_data.len() {
            for pixel in 0..cpu.bus.gpu.screen_data[scanline].len() {
                for rgb in cpu.bus.gpu.screen_data[scanline][pixel].iter() {
                    canvas.set_draw_color(Color::RGB(*rgb, *rgb, *rgb));
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
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'main,
                Event::KeyDown {
                    keycode: Some(Keycode::Z),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(4);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::X),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(5);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::V),
                    ..
                } => {
                    let pixels = vram_dump(&cpu);
                    // render the tiles SDL
                    let vram_window = video_system
                        .window("VRAM Blocks 0 & 1 Dump", 128, 128)
                        .resizable()
                        .position_centered()
                        .build()
                        .unwrap();

                    let mut canvas = vram_window.into_canvas().build().unwrap();
                    let texture_creator = canvas.texture_creator();
                    let mut texture = texture_creator
                        .create_texture_target(PixelFormatEnum::RGBA8888, 128, 128)
                        .unwrap();
                    let result = texture.update(None, &pixels, 128);
                    if result.is_err() {
                        panic!("Unable to upload pixel data to texture :(");
                    }
                    canvas.present();

                    ::std::thread::sleep(Duration::new(5, 0));
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(7)
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Backspace),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(6)
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(0)
                }
                Event::KeyDown { keycode: Some(Keycode::N), .. } => {
                    let mut file = File::create("vram_dump").unwrap();
                    let _result = file.write_all(&cpu.bus.gpu.vram);
                    if _result.is_ok() {
                        println!("Finished dumping VRAM");
                    }
                },
                Event::KeyDown {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(1)
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Up),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(2)
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Down),
                    ..
                } => {
                    println!("Key Pressed!");
                    cpu.set_key_pressed(3)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Z),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(4)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::X),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(5)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(7)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Backspace),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(6)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Right),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(0)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Left),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(1)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Up),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(2)
                }
                Event::KeyUp {
                    keycode: Some(Keycode::Down),
                    ..
                } => {
                    println!("Key Released!");
                    cpu.set_key_released(3)
                }
                _ => {}
            }
        }

        canvas.present();
        //::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

type PixelColor = system::cpu::gpu::TilePixelValue;
fn vram_dump(cpu: &system::cpu::CPU) -> [u8; 0x4000] {
    // array of pixels for the vram dump window
    let mut vram_pixels = [0u8; 0x4000];
    // array for all 256 tiles of bank 0 and bank 1
    let mut tiles = [[[0u8; 8]; 8]; 256];

    // iterate through bank 0 tiles 0-255
    for tile_index in (0..0x1000).step_by(16) {
        // iterate over each row of the tile
        for row_index in (0..16).step_by(2) {
            let first_byte = cpu.bus.gpu.vram[tile_index + row_index];
            let second_byte = cpu.bus.gpu.vram[tile_index + row_index + 1];
            // iterate through each bit of the two bytes to determine the color
            for bit in 0..8 {
                let high_bit = second_byte & (0x80 >> bit) != 0; // 1 = true, 0 = false
                let low_bit = first_byte & (0x80 >> bit) != 0; // 1 = true, 0 = false

                let color = match (high_bit, low_bit) {
                    (true, true) => PixelColor::Three,
                    (false, true) => PixelColor::Two,
                    (true, false) => PixelColor::One,
                    (false, false) => PixelColor::Zero,
                };

                // retrieve the color shade of gray for these two bits (one pixel)
                /*let first_bit = cpu.bus.gpu.bg_palette & (0x80 >> (2 * color as u8)) != 0;
                let second_bit = cpu.bus.gpu.bg_palette & (0x80 >> (1 + (2 * color as u8))) != 0;
                let shade = match (second_bit, first_bit) {
                    (true, true) => 0,     // black
                    (false, true) => 119,  // dark gray
                    (true, false) => 192,  // light gray
                    (false, false) => 255, // white
                };*/

                let shade = match color {
                    PixelColor::Three => 0,
                    PixelColor::Two => 119,
                    PixelColor::One => 192,
                    PixelColor::Zero => 255,
                };

                tiles[tile_index / 16][row_index / 2][bit] = shade;
            }
        }
    }

    // map the tiles to screen data (128 * 128 pixels)
    for tile_num in 0..256 {
        for row in 0..8 {
            for column in 0..8 {
                // i is the tile number
                // j is the row in the tile
                // k is the column in the tile
                vram_pixels[(tile_num * 8) + (128 * row) + column] = tiles[tile_num][row][column];
            }
        }
    }

    vram_pixels
}
