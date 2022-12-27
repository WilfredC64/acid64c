// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};
use std::time::Duration;

pub const ESC_KEY: char = '\x1b';
pub const LEFT_KEY: char = '\x25';
pub const RIGHT_KEY: char = '\x27';

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
    if let Event::Key(KeyEvent{ code, .. }) = read().unwrap() {
        match code {
            KeyCode::Char(c) => return Ok(Some(c)),
            KeyCode::Esc => return Ok(Some(ESC_KEY)),
            KeyCode::Right => return Ok(Some(RIGHT_KEY)),
            KeyCode::Left => return Ok(Some(LEFT_KEY)),
            _ => ()
        }
    }
    Ok(None)
}
