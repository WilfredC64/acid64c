// Copyright (C) 2019 - 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use encoding::{Encoding, DecoderTrap, EncoderTrap};
use encoding::all::ISO_8859_1;
use libloading::{Library, Symbol};
use std::ffi::{CString, CStr};
use std::ptr::null;

pub struct Acid64Library {
    a64lib: Library
}

#[allow(dead_code)]
impl Acid64Library {
    pub fn new() -> Acid64Library {
        Acid64Library {
            a64lib: Library::new("acid64pro").expect("acid64pro library could not be found."),
        }
    }

    pub fn get_version(&self) -> i32 {
        unsafe {
            (self.a64lib.get(b"getVersion").unwrap() as Symbol<unsafe extern "stdcall" fn() -> i32>)()
        }
    }

    pub fn create_c64_instance(&self) -> usize {
        unsafe {
            (self.a64lib.get(b"createC64Instance").unwrap() as Symbol<unsafe extern "stdcall" fn() -> usize>)()
        }
    }

    pub fn close_c64_instance(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"closeC64Instance").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance)
        }
    }

    pub fn check_sldb(&self, filename: String) -> bool {
        unsafe {
            let filename_converted = Self::convert_string_to_ansi_pchar(filename);
            (self.a64lib.get(b"checkSldb").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(filename_converted)
        }
    }

    pub fn check_sldb_from_buffer(&self, buffer: *const u8, size: i32) -> bool {
        unsafe {
            (self.a64lib.get(b"checkSldbFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer, size)
        }
    }

    pub fn load_sldb(&self, filename: String) -> bool {
        unsafe {
            let filename_converted = Self::convert_string_to_ansi_pchar(filename);
            (self.a64lib.get(b"loadSldb").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(filename_converted)
        }
    }

    pub fn load_sldb_from_buffer(&self, buffer: *const u8, size: i32) -> bool {
        unsafe {
            (self.a64lib.get(b"loadSldbFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer, size)
        }
    }

    pub fn get_filename(&self, md5_hash: String) -> String {
        unsafe {
            let md5_hash_converted = Self::convert_string_to_ansi_pchar(md5_hash);
            let filename = (self.a64lib.get(b"getFilename").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> *const i8>)(md5_hash_converted);
            Self::convert_pchar_to_ansi_string(filename)
        }
    }

    pub fn load_stil(&self, hvsc_location: String) -> bool {
        unsafe {
            let hvsc_location_converted = Self::convert_string_to_ansi_pchar(hvsc_location);
            (self.a64lib.get(b"loadStil").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(hvsc_location_converted)
        }
    }

    pub fn load_stil_from_buffer(&self, buffer: *const u8, size: i32) -> bool {
        unsafe {
            (self.a64lib.get(b"loadStilFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer, size)
        }
    }

    pub fn run(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"run").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance);
        }
    }

    pub fn load_file(&self, c64_instance: usize, filename: String) -> bool {
        unsafe {
            let filename_converted = Self::convert_string_to_ansi_pchar(filename);
            (self.a64lib.get(b"loadFile").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *const i8) -> bool>)(c64_instance, filename_converted)
        }
    }

    pub fn get_command(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getCommand").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_register(&self, c64_instance: usize) -> u8 {
        unsafe {
            (self.a64lib.get(b"getRegister").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> u8>)(c64_instance)
        }
    }

    pub fn get_data(&self, c64_instance: usize) -> u8 {
        unsafe {
            (self.a64lib.get(b"getData").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> u8>)(c64_instance)
        }
    }

    pub fn get_cycles(&self, c64_instance: usize) -> u16 {
        unsafe {
            (self.a64lib.get(b"getCycles").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> u16>)(c64_instance)
        }
    }

    pub fn get_title(&self, c64_instance: usize) -> String {
        unsafe {
            let title_cstyle = (self.a64lib.get(b"getTitle").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(title_cstyle)
        }
    }

    pub fn get_author(&self, c64_instance: usize) -> String {
        unsafe {
            let author_cstyle = (self.a64lib.get(b"getAuthor").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(author_cstyle)
        }
    }

    pub fn get_released(&self, c64_instance: usize) -> String {
        unsafe {
            let released_cstyle = (self.a64lib.get(b"getReleased").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(released_cstyle)
        }
    }

    pub fn get_number_of_songs(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getNumberOfSongs").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_default_song(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getDefaultSong").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_load_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getLoadAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_load_end_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getLoadEndAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_play_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getPlayAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_init_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getInitAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_sid_model(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSidModel").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_c64_version(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getC64Version").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_time(&self, c64_instance: usize) -> u32 {
        unsafe {
            (self.a64lib.get(b"getTime").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> u32>)(c64_instance)
        }
    }

    pub fn get_song_length(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSongLength").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_md5_hash(&self, c64_instance: usize) -> String {
        unsafe {
            let md5_hash_cstyle = (self.a64lib.get(b"getMd5Hash").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(md5_hash_cstyle)
        }
    }

    pub fn get_ancient_md5_hash(&self, c64_instance: usize) -> String {
        unsafe {
            let md5_hash_cstyle = (self.a64lib.get(b"getAncientMd5Hash").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(md5_hash_cstyle)
        }
    }

    pub fn get_stil_entry(&self, c64_instance: usize) -> Option<String> {
        unsafe {
            let stil_text_cstyle = (self.a64lib.get(b"getStilEntry").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            stil_text_cstyle.as_ref().map(|s| Self::convert_pchar_to_ansi_string(s))
        }
    }

    pub fn set_song_to_play(&self, c64_instance: usize, song_to_play: i32) {
        unsafe {
            (self.a64lib.get(b"setSongToPlay").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, i32)>)(c64_instance, song_to_play);
        }
    }

    pub fn set_c64_version(&self, c64_instance: usize, c64_version: i32) {
        unsafe {
            (self.a64lib.get(b"setC64Version").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, i32)>)(c64_instance, c64_version);
        }
    }

    pub fn press_buttons(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"pressButtons").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance);
        }
    }

    pub fn enable_fixed_startup(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"enableFixedStartup").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance);
        }
    }

    pub fn skip_silence(&self, c64_instance: usize, enabled: bool) {
        unsafe {
            (self.a64lib.get(b"skipSilence").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, bool)>)(c64_instance, enabled);
        }
    }

    pub fn enable_volume_fix(&self, c64_instance: usize, enabled: bool) {
        unsafe {
            (self.a64lib.get(b"enableVolumeFix").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, bool)>)(c64_instance, enabled);
        }
    }

    pub fn get_memory_usage_ram(&self, c64_instance: usize, buffer: *mut u8, size: i32) {
        unsafe {
            (self.a64lib.get(b"getMemoryUsageRam").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer, size);
        }
    }

    pub fn get_memory_usage_rom(&self, c64_instance: usize, buffer: *mut u8, size: i32) {
        unsafe {
            (self.a64lib.get(b"getMemoryUsageRom").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer, size);
        }
    }

    pub fn get_memory(&self, c64_instance: usize, buffer: *mut u8, size: i32) {
        unsafe {
            (self.a64lib.get(b"getMemory").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer, size);
        }
    }

    pub fn clear_mem_usage_on_first_sid_access(&self, c64_instance: usize, clear: bool) {
        unsafe {
            (self.a64lib.get(b"clearMemUsageOnFirstSidAccess").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, bool)>)(c64_instance, clear);
        }
    }

    pub fn clear_mem_usage_after_init(&self, c64_instance: usize, clear: bool) {
        unsafe {
            (self.a64lib.get(b"clearMemUsageAfterInit").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, bool)>)(c64_instance, clear);
        }
    }

    pub fn get_number_of_sids(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getNumberOfSids").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn start_seek(&self, c64_instance: usize, time: u32) {
        unsafe {
            (self.a64lib.get(b"startSeek").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, u32)>)(c64_instance, time);
        }
    }

    pub fn stop_seek(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"stopSeek").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance);
        }
    }

    pub fn get_cpu_load(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getCpuLoad").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_speed_flag(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSpeedFlag").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_frequency(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getFrequency").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    #[inline]
    fn convert_string_to_ansi_pchar(text: String) -> *const i8 {
        CString::new(ISO_8859_1.encode(&text, EncoderTrap::Ignore).unwrap()).unwrap().into_raw()
    }

    #[inline]
    unsafe fn convert_pchar_to_ansi_string(text: *const i8) -> String {
        if text == null() {
            "".to_string()
        } else {
            let c_str = CStr::from_ptr(text);
            ISO_8859_1.decode(c_str.to_bytes(), DecoderTrap::Ignore).unwrap()
        }
    }
}
