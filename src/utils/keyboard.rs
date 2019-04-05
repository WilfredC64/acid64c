// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crossterm_input::{TerminalInput, AsyncReader, InputEvent, KeyEvent};
use crossterm_screen::Screen;

pub const ESC_KEY: char = '\x1b';

pub fn get_input_reader() -> AsyncReader {
    let screen = Screen::new(true);
    let input = TerminalInput::from_output(&screen.stdout);
    input.read_async()
}

pub fn get_char_from_input(input_event: Option<InputEvent>) -> Option<char> {
    if let Some(input_event) = input_event {
        if let InputEvent::Keyboard(key_event) = input_event {
            match key_event {
                KeyEvent::Char(c) => return Some(c),
                KeyEvent::Esc => return Some(ESC_KEY),
                _ => ()
            }
        }
    }
    None
}

pub fn convert_num_key_to_number(key: char) -> i32 {
    match key {
        '1' ... '9' => (key as u8 - b'0' - 1) as i32,
        '0' => 9,
        _ => -1
    }
}