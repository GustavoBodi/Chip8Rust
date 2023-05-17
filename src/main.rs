mod fonts;

use crate::fonts::{FONTS, FONT_SIZE};
use rand::Rng;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color; 
use sdl2::rect::Rect;

const PROGRAM_START: usize = 0x200;
const FONT_ADDRESS: usize = 0x50;
const CHIP8_WIDTH: usize = 64;
const CHIP8_HEIGHT: usize = 32;

struct Renderer {
    canvas: sdl2::render::WindowCanvas,
    multiplier: u32,
    context: sdl2::Sdl,
}


impl Renderer {
    fn new(multiplier: u32) -> Self {
        let sdl_context = sdl2::init().unwrap();
        let video = sdl_context.video().unwrap();
        let window = video
            .window(
                "rust",
                multiplier * CHIP8_WIDTH as u32,
                multiplier * CHIP8_HEIGHT as u32,
            )
            .position_centered()
            .opengl()
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().build().unwrap();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();

        Renderer {
            context: sdl_context,
            canvas,
            multiplier,
        }
    }

    fn update(&mut self, pixels: &[[u8; CHIP8_WIDTH]; CHIP8_HEIGHT]) {
        for (y, &row) in pixels.iter().enumerate() {
            for (x, &col) in row.iter().enumerate() {
                let x = x as u32 * self.multiplier;
                let y = y as u32 * self.multiplier;
                self.canvas.set_draw_color(Renderer::color(col));
                let _ = self.canvas.fill_rect(Rect::new(
                    x as i32,
                    y as i32,
                    self.multiplier,
                    self.multiplier,
                ));
            }
        }
        self.canvas.present();
    }

    fn color(value: u8) -> Color {
        if value == 0 {
            Color::RGB(0, 0, 0)
        } else {
            Color::RGB(0, 250, 0)
        }
    }
}

struct Input {
    events: sdl2::EventPump,
}

impl Input {
    fn new(events: sdl2::EventPump) -> Self {
        Input { events }
    }
    fn poll(&mut self, keys: &mut [u8; 16]) -> bool {
        let pressed: Vec<Keycode> = self
            .events
            .keyboard_state()
            .pressed_scancodes()
            .filter_map(Keycode::from_scancode)
            .collect();

        keys.iter_mut().for_each(|x| *x = 0);

        for i in pressed {
            let key = match i {
                Keycode::Num1 => Some(0x1),
                Keycode::Num2 => Some(0x2),
                Keycode::Num3 => Some(0x3),
                Keycode::Num4 => Some(0xc),
                Keycode::Q => Some(0x4),
                Keycode::W => Some(0x5),
                Keycode::E => Some(0x6),
                Keycode::R => Some(0xd),
                Keycode::A => Some(0x7),
                Keycode::S => Some(0x8),
                Keycode::D => Some(0x9),
                Keycode::F => Some(0xe),
                Keycode::Z => Some(0xa),
                Keycode::X => Some(0x0),
                Keycode::C => Some(0xb),
                Keycode::V => Some(0xf),
                _ => None,
            };
            if let Some(index) = key {
                keys[index as usize] = 1;
            }
        }
        for event in self.events.poll_iter() {
            if let Event::Quit { .. } = event {
                return true;
            }
        }
        false
    }
}

struct Chip {
    memory: [u8; 4096],
    registers: [u8; 16],
    index_register: usize,
    pc: usize,
    stack: [u16; 16],
    sp: usize,
    sound_timer: u8,
    delay_timer: u8,
    input: [u8; 16],
    vram: [[u8; 64]; 32],
    opcode: u16,
}

impl Chip {
    fn new() -> Self {
        let mut chip = Chip {
            memory: [0; 4096],
            registers: [0; 16],
            index_register: 0,
            pc: 0,
            stack: [0; 16],
            sp: 0,
            sound_timer: 0,
            delay_timer: 0,
            input: [0; 16],
            vram: [[0; CHIP8_WIDTH]; CHIP8_HEIGHT],
            opcode: 0,
        };
        chip.pc = PROGRAM_START;
        chip.insert_font();
        chip
    }
    fn cycle(&mut self) {
        self.opcode = self.memory[self.pc] as u16;
        self.opcode <<= 8;
        self.opcode |= self.memory[self.pc + 1] as u16;
        self.pc += 2;
        self.run_opcode();
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }
    fn read(&mut self, address: String) {
        let rom = std::fs::read(address).unwrap();
        for (i, &value) in rom.iter().enumerate() {
            self.memory[PROGRAM_START + i] = value;
        }
    }
    fn run_opcode(&mut self) {
        let nibbles = (
            ((self.opcode & 0xF000) >> 12) as u8,
            ((self.opcode & 0x0F00) >> 8) as u8,
            ((self.opcode & 0x00F0) >> 4) as u8,
            (self.opcode & 0x000F) as u8,
        );
        let nnn = (self.opcode & 0x0FFF) as usize;
        let kk = (self.opcode & 0x00FF) as u8;
        let x = nibbles.1 as usize;
        let y = nibbles.2 as usize;
        let n = nibbles.3 as usize;
        match nibbles {
            (0x00, 0x00, 0x0E, 0x00) => self.op_00e0(),
            (0x00, 0x00, 0x0E, 0x0E) => self.op_00ee(),
            (0x01, _, _, _) => self.op_1nnn(nnn),
            (0x02, _, _, _) => self.op_2nnn(nnn),
            (0x03, _, _, _) => self.op_3xkk(x, kk),
            (0x04, _, _, _) => self.op_4xkk(x, kk),
            (0x05, _, _, 0x00) => self.op_5xy0(x, y),
            (0x06, _, _, _) => self.op_6xkk(x, kk),
            (0x07, _, _, _) => self.op_7xkk(x, kk),
            (0x08, _, _, 0x00) => self.op_8xy0(x, y),
            (0x08, _, _, 0x01) => self.op_8xy1(x, y),
            (0x08, _, _, 0x02) => self.op_8xy2(x, y),
            (0x08, _, _, 0x03) => self.op_8xy3(x, y),
            (0x08, _, _, 0x04) => self.op_8xy4(x, y),
            (0x08, _, _, 0x05) => self.op_8xy5(x, y),
            (0x08, _, _, 0x06) => self.op_8xy6(x, y),
            (0x08, _, _, 0x07) => self.op_8xy7(x, y),
            (0x08, _, _, 0x0E) => self.op_8xye(x, y),
            (0x09, _, _, 0x00) => self.op_9xy0(x, y),
            (0x0A, _, _, _) => self.op_annn(nnn),
            (0x0B, _, _, _) => self.op_bnnn(nnn),
            (0x0C, _, _, _) => self.op_cxkk(x, kk),
            (0x0D, _, _, _) => self.op_dxyn(x, y, n),
            (0x0E, _, 0x09, 0x0E) => self.op_ex9e(x),
            (0x0E, _, 0x0A, 0x01) => self.op_exa1(x),
            (0x0F, _, 0x00, 0x07) => self.op_fx07(x),
            (0x0F, _, 0x00, 0x0A) => self.op_fx0a(x),
            (0x0F, _, 0x01, 0x05) => self.op_fx15(x),
            (0x0F, _, 0x01, 0x08) => self.op_fx18(x),
            (0x0F, _, 0x01, 0x0E) => self.op_fx1e(x),
            (0x0F, _, 0x02, 0x09) => self.op_fx29(x),
            (0x0F, _, 0x03, 0x03) => self.op_fx33(x),
            (0x0F, _, 0x05, 0x05) => self.op_fx55(x),
            (0x0F, _, 0x06, 0x05) => self.op_fx65(x),
            _ => (),
        }
    }
    fn insert_font(&mut self) {
        for i in 0..FONT_SIZE {
            self.memory[FONT_ADDRESS + i] = FONTS[i];
        }
    }
    fn op_00e0(&mut self) {
        self.vram
            .iter_mut()
            .for_each(|x| x.iter_mut().for_each(|x| *x = 0));
    }
    fn op_00ee(&mut self) {
        self.sp -= 1;
        self.pc = self.stack[self.sp] as usize;
    }
    fn op_1nnn(&mut self, nnn: usize) {
        self.pc = nnn;
    }
    fn op_2nnn(&mut self, nnn: usize) {
        self.stack[self.sp] = self.pc as u16;
        self.sp += 1;
        self.pc = nnn;
    }
    fn op_3xkk(&mut self, x: usize, kk: u8) {
        if self.registers[x] == kk {
            self.pc += 2;
        }
    }
    fn op_4xkk(&mut self, x: usize, kk: u8) {
        if self.registers[x] != kk {
            self.pc += 2;
        }
    }
    fn op_5xy0(&mut self, x: usize, y: usize) {
        if self.registers[x] == self.registers[y] {
            self.pc += 2;
        }
    }
    fn op_6xkk(&mut self, x: usize, kk: u8) {
        self.registers[x] = kk;
    }
    fn op_7xkk(&mut self, x: usize, kk: u8) {
        let vx = self.registers[x] as u16;
        let val = kk as u16;
        let result = val + vx;
        self.registers[x] = result as u8;
    }
    fn op_8xy0(&mut self, x: usize, y: usize) {
        self.registers[x] = self.registers[y];
    }
    fn op_8xy1(&mut self, x: usize, y: usize) {
        self.registers[x] |= self.registers[y];
    }
    fn op_8xy2(&mut self, x: usize, y: usize) {
        self.registers[x] &= self.registers[y];
    }
    fn op_8xy3(&mut self, x: usize, y: usize) {
        self.registers[x] ^= self.registers[y];
    }
    fn op_8xy4(&mut self, x: usize, y: usize) {
        let vx = self.registers[x] as u16;
        let vy = self.registers[y] as u16;
        let result = vx + vy;

        self.registers[x] = result as u8;

        if result > 0xFF {
            self.registers[0x0F] = 1;
        } else {
            self.registers[0x0F] = 0;
        }
    }
    fn op_8xy5(&mut self, x: usize, y: usize) {
        self.registers[0x0F] = if self.registers[x] > self.registers[y] {
            1
        } else {
            0
        };
        self.registers[x] = self.registers[x].wrapping_sub(self.registers[y]);
    }
    fn op_8xy6(&mut self, x: usize, _y: usize) {
        self.registers[0x0F] = if (self.registers[x] & 0x1) == 1 { 1 } else { 0 };
        self.registers[x] >>= 1;
    }
    fn op_8xy7(&mut self, x: usize, y: usize) {
        self.registers[0x0F] = if self.registers[y] > self.registers[x] {
            1
        } else {
            0
        };
        self.registers[x] = self.registers[y].wrapping_sub(self.registers[x]);
    }
    fn op_8xye(&mut self, x: usize, _y: usize) {
        self.registers[0x0F] = (self.registers[x] & 0x80) >> 7;
        self.registers[x] <<= 1;
    }
    fn op_9xy0(&mut self, x: usize, y: usize) {
        if self.registers[x] != self.registers[y] {
            self.pc += 2;
        }
    }
    fn op_annn(&mut self, nnn: usize) {
        self.index_register = nnn as usize;
    }
    fn op_bnnn(&mut self, nnn: usize) {
        self.pc = nnn + self.registers[0x00] as usize;
    }
    fn op_cxkk(&mut self, x: usize, kk: u8) {
        let mut rng = rand::thread_rng();
        self.registers[x] = rng.gen::<u8>() & kk;
    }
    fn op_dxyn(&mut self, x: usize, y: usize, n: usize) {
        self.registers[0x0f] = 0;
        for byte in 0..n {
            let y = (self.registers[y] as usize + byte) % CHIP8_HEIGHT;
            for bit in 0..8 {
                let x = (self.registers[x] as usize + bit) % CHIP8_WIDTH;
                let color = (self.memory[self.index_register + byte] >> (7 - bit)) & 1;
                self.registers[0x0f] |= color & self.vram[y][x];
                self.vram[y][x] ^= color;
            }
        }
    }
    fn op_ex9e(&mut self, x: usize) {
        if self.input[self.registers[x] as usize] != 0 {
            self.pc += 2;
        }
    }
    fn op_exa1(&mut self, x: usize) {
        if self.input[self.registers[x] as usize] == 0 {
            self.pc += 2;
        }
    }
    fn op_fx07(&mut self, x: usize) {
        self.registers[x] = self.delay_timer;
    }
    fn op_fx0a(&mut self, x: usize) {
        let mut changed = false;
        for (index, input) in self.input.into_iter().enumerate() {
            if input == 1 {
                changed = true;
                self.registers[x] = index as u8;
            }
        }
        if changed {
            self.pc -= 2;
        }
    }
    fn op_fx15(&mut self, x: usize) {
        self.delay_timer = self.registers[x];
    }
    fn op_fx18(&mut self, x: usize) {
        self.sound_timer = self.registers[x];
    }
    fn op_fx1e(&mut self, x: usize) {
        self.index_register += self.registers[x] as usize;
    }
    fn op_fx29(&mut self, x: usize) {
        self.index_register = FONT_ADDRESS + (5 * self.registers[x]) as usize;
    }
    fn op_fx33(&mut self, x: usize) {
        self.memory[self.index_register] = self.registers[x] / 100;
        self.memory[self.index_register + 1] = (self.registers[x] % 100) / 10;
        self.memory[self.index_register + 2] = self.registers[x] % 10;
    }
    fn op_fx55(&mut self, x: usize) {
        for i in 0..=x {
            self.memory[self.index_register + i] = self.registers[x];
        }
    }
    fn op_fx65(&mut self, x: usize) {
        for i in 0..=x {
            self.registers[i] = self.memory[self.index_register + i];
        }
    }
}

fn main() {
    let multiplier = 10;
    let mut processor = Chip::new();
    let mut renderer = Renderer::new(multiplier);
    let mut input = Input::new(renderer.context.event_pump().unwrap());
    let cycle_delay = 2000000;
    let mut last_cycle = std::time::Instant::now();

    processor.read("/home/gustavo/CLionProjects/RustTest/breakout.rom".to_owned());
    let mut quit: bool = false;
    while !quit {
        let current = std::time::Instant::now();
        let dt = current - last_cycle;
        if dt > std::time::Duration::from_nanos(cycle_delay) {
            last_cycle = std::time::Instant::now();
            quit = input.poll(&mut processor.input);
            processor.cycle();
            renderer.update(&processor.vram);
        }
    }
}
