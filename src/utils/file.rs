// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
use std::fs::File;
use std::io::{self, BufRead, BufReader, Error};
use std::path::PathBuf;
use encoding_rs::WINDOWS_1252;
use encoding_rs_io::DecodeReaderBytesBuilder;

pub fn read_buffer_as_lines(buffer: &[u8]) -> impl Iterator<Item = io::Result<String>> + '_ {
    BufReader::new(DecodeReaderBytesBuilder::new()
        .encoding(Some(WINDOWS_1252))
        .build(buffer)).lines()
}

pub fn read_text_file_as_lines(config_path: &PathBuf, max_file_size: Option<u64>) -> Result<Box<dyn Iterator<Item = io::Result<String>>>, String> {
    read_lines(config_path, max_file_size)
        .map_err(|error| format!("Error reading file: {} -> {}", config_path.display(), error))
}

fn read_lines(filename: &PathBuf, max_file_size: Option<u64>) -> io::Result<Box<dyn Iterator<Item = io::Result<String>>>> {
    let file = File::open(filename)?;
    if let Some(max_file_size) = max_file_size && file.metadata()?.len() > max_file_size {
        return Err(Error::new(io::ErrorKind::InvalidData, "File too large"));
    }

    Ok(Box::new(BufReader::new(DecodeReaderBytesBuilder::new()
        .encoding(Some(WINDOWS_1252))
        .build(file)).lines()))
}
