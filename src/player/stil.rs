// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use crate::utils::file;

const DOCUMENTS_FOLDER: &str = "DOCUMENTS";
const STIL_FILE_NAME: &str = "STIL.txt";
const BUG_LIST_FILE_NAME: &str = "BUGlist.txt";
const MAX_STIL_FILE_SIZE: u64 = 1024 * 1024 * 1024;

pub struct Stil {
    pub stil_info: HashMap<String, String>,
    pub global_comments: HashMap<String, String>,
}

impl Stil {
    pub fn new() -> Stil {
        Stil {
            stil_info: HashMap::<String, String>::new(),
            global_comments: HashMap::<String, String>::new(),
        }
    }

    pub fn get_entry(&self, sid_file_name: &str) -> Option<String> {
        let sid_file_name = sid_file_name.to_ascii_lowercase();
        let global_entries = self.get_global_entries(&sid_file_name);

        if let Some(stil_entry) = self.stil_info.get(&sid_file_name) {
            if let Some(global_entries) = global_entries {
                Some(global_entries + "\n" + stil_entry)
            } else {
                Some(stil_entry.to_string())
            }
        } else {
            global_entries
        }
    }

    pub fn load(&mut self, hvsc_path: &str) -> Result<(), String> {
        let stil_file = Path::new(hvsc_path).join(DOCUMENTS_FOLDER).join(STIL_FILE_NAME);
        if !stil_file.exists() {
            return Err(format!("STIL file not found: {}", stil_file.display()));
        }

        self.stil_info.clear();
        self.global_comments.clear();

        let lines: Vec<String> = file::read_text_file(&stil_file, Some(MAX_STIL_FILE_SIZE))?;
        self.process_lines(lines);

        let bug_list_file = Path::new(hvsc_path).join(DOCUMENTS_FOLDER).join(BUG_LIST_FILE_NAME);
        if bug_list_file.exists() {
            let lines: Vec<String> = file::read_text_file(&bug_list_file, Some(MAX_STIL_FILE_SIZE))?;
            self.process_lines(lines);
        }
        Ok(())
    }

    pub fn load_from_buffer(&mut self, buffer: &[u8]) {
        self.stil_info.clear();
        self.global_comments.clear();

        let lines: Vec<String> = file::read_buffer_as_string(buffer);
        self.process_lines(lines);
    }

    fn process_lines(&mut self, lines: Vec<String>) {
        let mut stil_entry: Vec<String> = vec![];
        let mut stil_filename = "".to_string();
        let mut global = false;

        for line in lines {
            let stil_text = line.trim();
            let first_char = stil_text.chars().next().unwrap_or('#');

            match first_char {
                '#' => continue,
                '/' => {
                    self.add_stil_entry(&mut stil_filename, &mut stil_entry, global);
                    stil_entry.clear();

                    global = stil_text.ends_with('/');
                    stil_filename = stil_text.to_ascii_lowercase();
                    continue;
                },
                _ => {
                    stil_entry.push(line.clone());
                }
            }
        }
        self.add_stil_entry(&mut stil_filename, &mut stil_entry, global);
    }

    fn add_stil_entry(&mut self, stil_filename: &mut String, stil_entry: &mut Vec<String>, global: bool) {
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
                global_entries.push(global_comment.to_string())
            }
        }

        if !global_entries.is_empty() {
            Some(global_entries.join("\n"))
        } else {
            None
        }
    }
}
