// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
pub const MIN_SID_HEADER_SIZE: usize = 0x76;

pub fn is_sid_file(source: &[u8]) -> bool {
    source.len() >= MIN_SID_HEADER_SIZE && matches!(&source[0..4], b"RSID" | b"PSID")
}
