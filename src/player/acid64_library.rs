// Copyright (C) 2019 - 2022 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use encoding::{Encoding, DecoderTrap, EncoderTrap};
use encoding::all::WINDOWS_1252;
use libloading::{Library, Symbol};
use std::ffi::{CString, CStr};
use std::mem;

#[cfg(target_arch = "x86")]
pub struct Acid64Library {
    a64lib: Library
}

#[allow(dead_code)]
impl Acid64Library {
    fn new(a64lib: Library) -> Acid64Library {
        Acid64Library {
            a64lib
        }
    }

    pub fn load() -> Result<Acid64Library, String> {
        let a64lib = unsafe { Library::new("acid64pro") };
        if a64lib.is_err() {
            return Err("acid64pro library could not be loaded.".to_string());
        }
        Ok(Acid64Library::new(a64lib.unwrap()))
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

    pub fn check_sldb(&self, filename: &str) -> bool {
        unsafe {
            let filename_converted = Self::convert_string_to_ansi_pchar(filename);
            (self.a64lib.get(b"checkSldb").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(filename_converted)
        }
    }

    pub fn check_sldb_from_buffer(&self, buffer: &[u8]) -> bool {
        unsafe {
            (self.a64lib.get(b"checkSldbFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer.as_ptr(), buffer.len() as i32)
        }
    }

    pub fn load_sldb(&self, filename: &str) -> bool {
        unsafe {
            let filename_converted = Self::convert_string_to_ansi_pchar(filename);
            (self.a64lib.get(b"loadSldb").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(filename_converted)
        }
    }

    pub fn load_sldb_from_buffer(&self, buffer: &[u8]) -> bool {
        unsafe {
            (self.a64lib.get(b"loadSldbFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer.as_ptr(), buffer.len() as i32)
        }
    }

    pub fn get_filename(&self, md5_hash: &str) -> String {
        unsafe {
            let md5_hash_converted = Self::convert_string_to_ansi_pchar(md5_hash);
            let filename = (self.a64lib.get(b"getFilename").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> *const i8>)(md5_hash_converted);
            Self::convert_pchar_to_ansi_string(filename).unwrap_or_default()
        }
    }

    pub fn load_stil(&self, hvsc_location: &str) -> bool {
        unsafe {
            let hvsc_location_converted = Self::convert_string_to_ansi_pchar(hvsc_location);
            (self.a64lib.get(b"loadStil").unwrap() as Symbol<unsafe extern "stdcall" fn(*const i8) -> bool>)(hvsc_location_converted)
        }
    }

    pub fn load_stil_from_buffer(&self, buffer: &[u8]) -> bool {
        unsafe {
            (self.a64lib.get(b"loadStilFromBuffer").unwrap() as Symbol<unsafe extern "stdcall" fn(*const u8, i32) -> bool>)(buffer.as_ptr(), buffer.len() as i32)
        }
    }

    pub fn run(&self, c64_instance: usize) {
        unsafe {
            (self.a64lib.get(b"run").unwrap() as Symbol<unsafe extern "stdcall" fn(usize)>)(c64_instance);
        }
    }

    pub fn load_file(&self, c64_instance: usize, filename: &str) -> bool {
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
            Self::convert_pchar_to_ansi_string(title_cstyle).unwrap_or_default()
        }
    }

    pub fn get_author(&self, c64_instance: usize) -> String {
        unsafe {
            let author_cstyle = (self.a64lib.get(b"getAuthor").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(author_cstyle).unwrap_or_default()
        }
    }

    pub fn get_released(&self, c64_instance: usize) -> String {
        unsafe {
            let released_cstyle = (self.a64lib.get(b"getReleased").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(released_cstyle).unwrap_or_default()
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

    pub fn get_sid_model(&self, c64_instance: usize, sid_nr: i32) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSidModel").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, i32) -> i32>)(c64_instance, sid_nr)
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
            Self::convert_pchar_to_ansi_string(md5_hash_cstyle).unwrap_or_default()
        }
    }

    pub fn get_ancient_md5_hash(&self, c64_instance: usize) -> String {
        unsafe {
            let md5_hash_cstyle = (self.a64lib.get(b"getAncientMd5Hash").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(md5_hash_cstyle).unwrap_or_default()
        }
    }

    pub fn get_stil_entry(&self, c64_instance: usize) -> Option<String> {
        unsafe {
            let stil_text_cstyle = (self.a64lib.get(b"getStilEntry").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(stil_text_cstyle)
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

    pub fn get_memory_usage_ram(&self, c64_instance: usize, buffer: &mut [u8; 0x10000]) {
        unsafe {
            (self.a64lib.get(b"getMemoryUsageRam").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
        }
    }

    pub fn get_memory_usage_rom(&self, c64_instance: usize, buffer: &mut [u8; 0x10000]) {
        unsafe {
            (self.a64lib.get(b"getMemoryUsageRom").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
        }
    }

    pub fn get_memory(&self, c64_instance: usize, buffer: &mut [u8; 0x10000]) {
        unsafe {
            (self.a64lib.get(b"getMemory").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
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

    pub fn get_sid_address(&self, c64_instance: usize, sid_nr: i32) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSidAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, i32) -> i32>)(c64_instance, sid_nr)
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

    pub fn get_speed_flags(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getSpeedFlags").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_frequency(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getFrequency").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_mus_text(&self, c64_instance: usize, buffer: &mut [u8; 32*5]) {
        unsafe {
            (self.a64lib.get(b"getMusText").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
        }
    }

    pub fn get_mus_colors(&self, c64_instance: usize, buffer: &mut [u8; 32*5]) {
        unsafe {
            (self.a64lib.get(b"getMusColors").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
        }
    }

    pub fn get_file_type(&self, c64_instance: usize) -> String {
        unsafe {
            let file_type_cstyle = (self.a64lib.get(b"getFileType").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(file_type_cstyle).unwrap_or_default()
        }
    }

    pub fn get_file_format(&self, c64_instance: usize) -> String {
        unsafe {
            let file_format_cstyle = (self.a64lib.get(b"getFileFormat").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> *const i8>)(c64_instance);
            Self::convert_pchar_to_ansi_string(file_format_cstyle).unwrap_or_default()
        }
    }

    pub fn is_basic_sid(&self, c64_instance: usize) -> bool {
        unsafe {
            (self.a64lib.get(b"isBasicSid").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> bool>)(c64_instance)
        }
    }

    pub fn get_free_memory_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getFreeMemoryAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_free_memory_end_address(&self, c64_instance: usize) -> i32 {
        unsafe {
            (self.a64lib.get(b"getFreeMemoryEndAddress").unwrap() as Symbol<unsafe extern "stdcall" fn(usize) -> i32>)(c64_instance)
        }
    }

    pub fn get_last_sid_writes(&self, c64_instance: usize, buffer: &mut [u8; 256]) {
        unsafe {
            (self.a64lib.get(b"getLastSidWrites").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u8, i32)>)(c64_instance, buffer.as_mut_ptr(), buffer.len() as i32);
        }
    }

    pub fn get_last_sid_write_times(&self, c64_instance: usize, buffer: &mut [u32; 256]) {
        unsafe {
            (self.a64lib.get(b"getLastSidWriteTimes").unwrap() as Symbol<unsafe extern "stdcall" fn(usize, *mut u32, i32)>)(c64_instance, buffer.as_mut_ptr(), mem::size_of_val(buffer) as i32);
        }
    }

    #[inline]
    fn convert_string_to_ansi_pchar(text: &str) -> *const i8 {
        CString::new(WINDOWS_1252.encode(text, EncoderTrap::Ignore).unwrap()).unwrap().into_raw()
    }

    #[inline]
    unsafe fn convert_pchar_to_ansi_string(text: *const i8) -> Option<String> {
        if text.is_null() {
            None
        } else {
            Some(WINDOWS_1252.decode(CStr::from_ptr(text).to_bytes(), DecoderTrap::Ignore).unwrap())
        }
    }
}
