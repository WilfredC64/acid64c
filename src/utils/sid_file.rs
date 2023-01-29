// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
pub const MIN_SID_HEADER_SIZE: usize = 0x76;

pub fn is_sid_file(source: &[u8]) -> bool {
    source.len() >= MIN_SID_HEADER_SIZE &&
        (source[0] == b'R' || source[0] == b'P') && source[1] == b'S' && source[2] == b'I' && source[3] == b'D'
}
