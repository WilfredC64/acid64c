// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
use std::io::Error;
use std::path::{Path, PathBuf};

use crate::utils::file;
use ahash::AHashMap;

const DOCUMENTS_FOLDER: &str = "DOCUMENTS";
const STIL_FILE_NAME: &str = "STIL.txt";
const BUG_LIST_FILE_NAME: &str = "BUGlist.txt";
const MAX_STIL_FILE_SIZE: u64 = 1024 * 1024 * 1024;
const MIN_STIL_LINES_CAPACITY: usize = 150;
const MIN_STIL_ENTRIES_CAPACITY: usize = 20_000;
const MIN_GLOBAL_ENTRIES_CAPACITY: usize = 300;

pub struct Stil {
    stil_info: AHashMap<String, String>,
    global_comments: AHashMap<String, String>,
}

impl Stil {
    pub fn new() -> Stil {
        Stil {
            stil_info: AHashMap::with_capacity(MIN_STIL_ENTRIES_CAPACITY),
            global_comments: AHashMap::with_capacity(MIN_GLOBAL_ENTRIES_CAPACITY)
        }
    }

    pub fn get_entry(&self, sid_file_name: &str) -> Option<String> {
        let sid_file_name = sid_file_name.to_ascii_lowercase();
        let global_entries = self.get_global_entries(&sid_file_name);

        self.stil_info.get(&sid_file_name)
            .map(|stil_entry| {
                global_entries.as_ref()
                    .map_or(stil_entry.to_string(), |global_lines| global_lines.to_owned() + "\n" + stil_entry)
            }).or(global_entries)
    }

    pub fn load(&mut self, hvsc_path_or_stil_file: &str) -> Result<(), String> {
        let hvsc_path = PathBuf::from(hvsc_path_or_stil_file);
        let stil_file = if !hvsc_path.is_file() {
            Self::find_stil_file(&hvsc_path, STIL_FILE_NAME)?
        } else {
            hvsc_path.to_path_buf()
        };

        self.stil_info.clear();
        self.global_comments.clear();

        let mut lines = file::read_text_file_as_lines(&stil_file, Some(MAX_STIL_FILE_SIZE))?;
        self.process_lines(&mut lines)?;

        if !hvsc_path.is_file() {
            let bug_list_file = Self::find_stil_file(&hvsc_path, BUG_LIST_FILE_NAME);
            if let Ok(bug_list_file) = bug_list_file {
                let mut lines = file::read_text_file_as_lines(&bug_list_file, Some(MAX_STIL_FILE_SIZE))?;
                self.process_lines(&mut lines)?;
            }
        }
        Ok(())
    }

    pub fn load_from_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        self.stil_info.clear();
        self.global_comments.clear();

        let mut lines = file::read_buffer_as_lines(buffer);
        self.process_lines(&mut lines)
    }

    fn process_lines<T>(&mut self, text_lines: &mut T) -> Result<(), String>
    where
        T: Iterator<Item = Result<String, Error>>
    {
        let mut stil_filename = "".to_string();
        let mut global = false;
        Self::validate_file_format(text_lines, &mut stil_filename, &mut global)?;

        let mut stil_entry: Vec<String> = Vec::with_capacity(MIN_STIL_LINES_CAPACITY);

        for line in text_lines {
            let line = line.map_err(|error| format!("Error reading STIL file -> {error}"))?;
            self.process_line(&line, &mut stil_entry, &mut stil_filename, &mut global);
        }
        self.add_stil_entry(&stil_filename, &stil_entry, global);
        Ok(())
    }

    fn process_line(&mut self, line: &str, stil_entry: &mut Vec<String>, stil_filename: &mut String, global: &mut bool) {
        let stil_text = line.trim_end();
        let first_char = stil_text.chars().next().unwrap_or('#');

        match first_char {
            '#' => (),
            '/' => {
                self.add_stil_entry(stil_filename, stil_entry, *global);
                stil_entry.clear();

                *global = stil_text.ends_with('/');
                *stil_filename = stil_text.to_ascii_lowercase();
            },
            _ => {
                stil_entry.push(line.to_string());
            }
        }
    }

    fn find_stil_file(hvsc_path: &Path, filename: &str) -> Result<PathBuf, String> {
        for &folder in &[DOCUMENTS_FOLDER, ""] {
            let stil_file = hvsc_path.join(folder).join(filename);
            if stil_file.exists() {
                return Ok(stil_file);
            }
        }
        Err(format!("STIL file not found in: {}", hvsc_path.to_string_lossy()))
    }

    fn add_stil_entry(&mut self, stil_filename: &String, stil_entry: &[String], global: bool) {
        if !stil_entry.is_empty() {
            if global {
                self.global_comments
                    .entry(stil_filename.to_owned())
                    .and_modify(|text| *text += &("\n".to_string() + &stil_entry.join("\n")))
                    .or_insert(stil_entry.join("\n"));
            } else {
                self.stil_info.entry(stil_filename.to_owned())
                    .and_modify(|text| *text += &("\n".to_string() + &stil_entry.join("\n")))
                    .or_insert(stil_entry.join("\n"));
            }
        }
    }

    fn get_global_entries(&self, sid_file_name: &str) -> Option<String> {
        let mut global_entries: Vec<String> = vec![];
        let mut path = Path::new(sid_file_name);

        while let Some(parent_path) = path.parent() {
            path = parent_path;
            let parent_path = parent_path.to_str().unwrap().to_string() + "/";

            if let Some(global_comment) = self.global_comments.get(&parent_path) {
                global_entries.push(global_comment.to_string());
            }
        }

        if !global_entries.is_empty() {
            Some(global_entries.join("\n"))
        } else {
            None
        }
    }

    fn validate_file_format<T>(text_lines: &mut T, stil_filename: &mut String, global: &mut bool) -> Result<(), String>
    where
        T: Iterator<Item = Result<String, Error>>
    {
        const MAX_LINES_TO_VALIDATE: usize = 50;

        for (index, line) in text_lines.enumerate() {
            let line = line.map_err(|error| format!("Error reading STIL file -> {error}"))?;
            let trimmed_line = line.trim();

            if index >= MAX_LINES_TO_VALIDATE {
                break;
            }

            let first_char = trimmed_line.chars().next().unwrap_or('#');
            match first_char {
                '#' => continue,
                '/' => {
                    *global = trimmed_line.ends_with('/');
                    *stil_filename = trimmed_line.to_ascii_lowercase();
                    return Ok(());
                },
                _ => break
            }
        }

        Err("STIL file format error".to_string())
    }
}
