// Copyright (C) 2019 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Duration;

pub const ESC_KEY: char = '\x1b';
pub const LEFT_KEY: char = '\x25';
pub const RIGHT_KEY: char = '\x27';

pub fn get_char_from_input() -> Option<char> {
    if poll(Duration::from_millis(0)).unwrap_or(false) {
        read_char()
    } else {
        None
    }
}

pub fn convert_num_key_to_number(key: char) -> i32 {
    match key {
        '1' ..= '9' => key as i32 - '1' as i32,
        '0' => 9,
        _ => -1
    }
}

fn read_char() -> Option<char> {
    if let Ok(Event::Key(KeyEvent{ code, kind, .. })) = read() {
        if kind == KeyEventKind::Press {
            match code {
                KeyCode::Char(c) => return Some(c),
                KeyCode::Esc => return Some(ESC_KEY),
                KeyCode::Right => return Some(RIGHT_KEY),
                KeyCode::Left => return Some(LEFT_KEY),
                _ => ()
            }
        }
    }
    None
}
