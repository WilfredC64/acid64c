// Copyright (C) 2022 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::env;

pub struct Config {
    pub hvsc_location: Option<String>,
    pub host_name_sid_device: Option<String>,
    pub host_name_ultimate_device: Option<String>,
    pub display_stil: bool,
    pub display_devices: bool,
    pub adjust_clock: bool,
    pub device_numbers: Vec<i32>,
    pub song_number: i32,
    pub filename: String
}

impl Config {
    pub fn read() -> Result<Config, String> {
        let mut hvsc_location = None;
        let mut host_name_sid_device = None;
        let mut host_name_ultimate_device = None;
        let mut display_stil = false;
        let mut display_devices = false;
        let mut adjust_clock = false;
        let mut device_numbers = vec![-1];
        let mut song_number = -1;
        let filename = env::args().last().unwrap();

        for argument in env::args().filter(|arg| arg.len() > 1 && arg.starts_with('-')) {
            match &argument[1..2] {
                "c" => adjust_clock = true,
                "d" => device_numbers = Self::parse_argument_numbers("Device number", &argument[2..])?,
                "h" => match &argument[2..3] {
                    "s" => host_name_sid_device = Some(argument[3..].to_string()),
                    "u" => host_name_ultimate_device = Some(argument[3..].to_string()),
                    _ => {}
                },
                "i" => display_stil = true,
                "l" => hvsc_location = Some(argument[2..].to_string()),
                "p" => display_devices = true,
                "s" => song_number = Self::parse_argument_number("Song number", &argument[2..])?,
                _ => return Err(format!("Unknown option: {argument}"))
            }
        }

        Ok(Config {
            hvsc_location,
            host_name_sid_device,
            host_name_ultimate_device,
            display_stil,
            display_devices,
            adjust_clock,
            device_numbers,
            song_number,
            filename
        })
    }

    fn parse_argument_numbers(arg_name: &str, arg_values: &str) -> Result<Vec<i32>, String> {
        let values = arg_values.split(',');
        let mut numbers = vec![];
        for value in values {
            let result = Self::parse_argument_number(arg_name, value);
            if result.is_err() {
                return Err(result.err().unwrap());
            }
            numbers.push(result.unwrap());
        }
        Ok(numbers)
    }

    fn parse_argument_number(arg_name: &str, arg_value: &str) -> Result<i32, String> {
        let number = match arg_value.parse::<i32>() {
            Ok(i) => i,
            Err(_e) => return Err(format!("{arg_name} must be a valid number and must be higher than 0."))
        };

        if number >= 1 {
            Ok(number - 1)
        } else {
            Err(format!("{arg_name} must be higher than 0."))
        }
    }
}
