// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};
use std::time::Duration;

pub const ESC_KEY: char = '\x1b';

pub fn get_char_from_input() -> Option<char> {
    if poll(Duration::from_millis(0)).unwrap() {
        read_char().unwrap()
    } else {
        None
    }
}

pub fn convert_num_key_to_number(key: char) -> i32 {
    match key {
        '1' ..= '9' => (key as u8 - b'0' - 1) as i32,
        '0' => 9,
        _ => -1
    }
}

fn read_char() -> Result<Option<char>, ()> {
    let event = read().unwrap();
    match event {
        Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) => return Ok(Some(c)),
        Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => return Ok(Some(ESC_KEY)),
        _ => ()
    }
    return Ok(None)
}
