// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
use std::io::{self, Error};
use std::path::{Path, PathBuf};

use crate::utils::file;
use ahash::AHashMap;

const DOCUMENTS_FOLDER: &str = "DOCUMENTS";
const OLD_SLDB_FILE_NAME: &str = "Songlengths.txt";
const NEW_SLDB_FILE_NAME: &str = "Songlengths.md5";
const MAX_SLDB_FILE_SIZE: u64 = 1024 * 1024 * 1024;
const MIN_ENTRIES_CAPACITY: usize = 80_000;

pub struct Sldb {
    songlengths: AHashMap<String, (String, String)>,
    new_md5_hash_used: bool
}

impl Sldb {
    pub fn new() -> Sldb {
        Sldb {
            songlengths: AHashMap::with_capacity(MIN_ENTRIES_CAPACITY),
            new_md5_hash_used: true
        }
    }

    pub fn get_song_length(&self, md5_hash: &str, sub_tune: i32) -> Option<i32> {
        let (_, sldb_entry) = self.songlengths.get(md5_hash)?;
        let sub_tune_length = sldb_entry.split_whitespace().nth(sub_tune as usize)?;
        let stripped_length = Self::strip_indicators(sub_tune_length);
        Some(Self::convert_time_to_millis(stripped_length))
    }

    pub fn get_hvsc_filename(&self, md5_hash: &str) -> Option<String> {
        self.songlengths.get(md5_hash)
            .map(|(filename, _)| filename.to_string())
    }

    pub fn is_new_md5_hash_used(&self) -> bool {
        self.new_md5_hash_used
    }

    pub fn load(&mut self, hvsc_path_or_sldb_file: &str) -> Result<(), String> {
        let mut lines = self.get_sldb_lines(hvsc_path_or_sldb_file)?;
        self.process_lines(&mut lines)
    }

    pub fn load_from_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        let mut lines = file::read_buffer_as_lines(buffer);
        self.process_lines(&mut lines)
    }

    pub fn validate(&mut self, hvsc_path_or_sldb_file: &str) -> Result<(), String> {
        let mut lines = self.get_sldb_lines(hvsc_path_or_sldb_file)?;
        Self::validate_file_format(&mut lines)
    }

    pub fn validate_from_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        let mut lines = file::read_buffer_as_lines(buffer);
        Self::validate_file_format(&mut lines)
    }

    fn get_sldb_lines(&mut self, hvsc_path_or_sldb_file: &str) -> Result<impl Iterator<Item = io::Result<String>>, String> {
        let mut sldb_file = PathBuf::from(hvsc_path_or_sldb_file);
        if !sldb_file.is_file() {
            sldb_file = Self::find_song_length_file(&sldb_file)?;
        }

        self.new_md5_hash_used = sldb_file.extension().unwrap_or("md5".as_ref()) == "md5";
        file::read_text_file_as_lines(&sldb_file, Some(MAX_SLDB_FILE_SIZE))
    }

    fn process_lines<T>(&mut self, text_lines: &mut T) -> Result<(), String>
    where
        T: Iterator<Item = Result<String, Error>>
    {
        Self::validate_file_format(text_lines)?;

        let mut song_lengths= "".to_string();
        let mut md5_hash = "".to_string();
        let mut hvsc_filename = "".to_string();

        self.songlengths.clear();

        for line in text_lines {
            let line = line.map_err(|error| format!("Error reading SLDB file -> {error}"))?;
            self.process_line(&line, &mut song_lengths, &mut md5_hash, &mut hvsc_filename);
        }

        self.add_sldb_entry(&hvsc_filename, &song_lengths, &md5_hash);
        Ok(())
    }

    fn process_line(&mut self, line: &str, song_lengths: &mut String, md5_hash: &mut String, hvsc_filename: &mut String) {
        let sldb_text = line.trim();
        let first_char = sldb_text.chars().next().unwrap_or('#');

        match first_char {
            '#' => (),
            ';' => {
                self.add_sldb_entry(hvsc_filename, song_lengths, md5_hash);
                song_lengths.clear();
                *hvsc_filename = sldb_text[2..].to_string();
            },
            _ => {
                if let Some((hash, lengths)) = sldb_text.split_once('=') {
                    *md5_hash = hash.to_string();
                    *song_lengths = lengths.to_string();
                }
            }
        }
    }

    fn find_song_length_file(hvsc_path: &Path) -> Result<PathBuf, String> {
        for &file_name in &[NEW_SLDB_FILE_NAME, OLD_SLDB_FILE_NAME] {
            for &folder in &[DOCUMENTS_FOLDER, ""] {
                let sldb_file = hvsc_path.join(folder).join(file_name);
                if sldb_file.exists() {
                    return Ok(sldb_file);
                }
            }
        }
        Err(format!("Songlengths file not found in: {}", hvsc_path.to_string_lossy()))
    }

    fn add_sldb_entry(&mut self, hvsc_filename: &String, song_lengths: &String, md5_hash: &String) {
        if !song_lengths.is_empty() {
            self.songlengths.insert(md5_hash.to_string(), (hvsc_filename.to_string(), song_lengths.to_string()));
        }
    }

    fn validate_file_format<T>(text_lines: &mut T) -> Result<(), String>
    where
        T: Iterator<Item = Result<String, Error>>
    {
        const MAX_LINES_TO_VALIDATE: usize = 20;

        for (index, line) in text_lines.enumerate() {
            let line = line.map_err(|error| format!("Error reading SLDB file -> {error}"))?;
            let trimmed_line = line.trim_start();

            if trimmed_line.is_empty() {
                if index >= MAX_LINES_TO_VALIDATE {
                    break;
                }
                continue;
            }

            if trimmed_line.starts_with("[Database]") {
                return Ok(());
            }
            break;
        }

        Err("Songlengths file format error".to_string())
    }

    fn strip_indicators(song_length: &str) -> &str {
        song_length.find('(').map_or(song_length, |index| &song_length[..index])
    }

    fn convert_time_to_millis(song_length: &str) -> i32 {
        let (time, millis) = song_length.split_once('.').unwrap_or((song_length, "0"));
        let (minutes, seconds) = time.split_once(':').unwrap_or(("5", "0"));

        let minutes = minutes.parse::<i32>().unwrap_or(5);
        let seconds = seconds.parse::<i32>().unwrap_or(0);
        let millis = millis.parse::<i32>().unwrap_or(0);
        (minutes * 60 + seconds) * 1000 + millis
    }
}
