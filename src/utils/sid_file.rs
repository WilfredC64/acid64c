// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
pub const SID_FILE_FORMAT_VERSION_OFFSET: usize = 0x05;
pub const SID_HEADER_SIZE_OFFSET: usize = 0x07;
pub const SID_SONG_COUNT_OFFSET: usize = 0x0f;
pub const SID_DEFAULT_SONG_OFFSET: usize = 0x11;
pub const SID_TITLE_OFFSET: usize = 0x16;
pub const SID_FLAGS_OFFSET: usize = 0x77;

pub const SID_HEADER_SIZE: usize = 0x7c;

pub const FLAG_BUILTIN_MUS_PLAYER: u8 = 0x01;
pub const FLAG_NTSC: u8 = 0x08;
pub const FLAG_8580: u8 = 0x20;

const MIN_SID_HEADER_SIZE: usize = 0x76;

pub fn is_sid_file(source: &[u8]) -> bool {
    source.len() >= MIN_SID_HEADER_SIZE && matches!(&source[0..4], b"RSID" | b"PSID")
}
