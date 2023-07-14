// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

pub fn read_buffer_as_string(buffer: &[u8]) -> Vec<String> {
    let lines = BufReader::new(DecodeReaderBytesBuilder::new()
        .encoding(Some(WINDOWS_1252))
        .build(buffer)).lines();
    lines.flatten().collect()
}

pub fn read_text_file(config_path: &PathBuf, max_file_size: Option<u64>) -> Result<Vec<String>, String> {
    let lines = read_lines(config_path, max_file_size);
    lines.map_err(|error| format!("Error reading file: {} -> {}", config_path.display(), error))
}

fn read_lines(filename: &PathBuf, max_file_size: Option<u64>) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    if let Some(max_file_size) = max_file_size {
        if file.metadata()?.len() > max_file_size {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "File too large"));
        }
    }

    let lines = BufReader::new(
        DecodeReaderBytesBuilder::new()
            .encoding(Some(WINDOWS_1252))
            .build(file)).lines();
    Ok(lines.flatten().collect())
}
