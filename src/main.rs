// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod console_player;
mod player;
mod utils;

use std::env;
use std::process::exit;
use self::console_player::ConsolePlayer;
use self::player::Player;

fn main() {
    match run() {
        Ok(_) => {}
        Err(message) => {
            eprintln!("ERROR: {}", message);
            exit(1);
        }
    }
}

fn parse_argument_number(arg_name: &str, arg_value: &str) -> Result<i32, String> {
    let number = match arg_value.parse::<i32>() {
        Ok(i) => i,
        Err(_e) => {
            return Err(format!("{} must be a valid number and must be higher than 0.", arg_name));
        }
    };

    if number >= 1 {
        Ok(number - 1)
    } else {
        Err(format!("{} must be higher than 0.", arg_name))
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        print_usage();
        return Ok(());
    }

    let mut hvsc_location = None;
    let mut display_stil = false;
    let mut display_devices = false;
    let mut device_number = -1;
    let mut song_number = -1;
    let filename = env::args().last().unwrap();

    let mut player = Player::new();

    for argument in env::args().filter(|arg| arg.len() > 1 && arg.starts_with("-")) {
        match &argument[1..2] {
            "d" => device_number = parse_argument_number("Device number", &argument[2..])?,
            "h" => {
                let host_name = argument.chars().skip(2).collect();
                player.set_host_name(host_name);
            },
            "i" => display_stil = true,
            "l" => hvsc_location = Some(argument.chars().skip(2).collect()),
            "p" => display_devices = true,
            "s" => song_number = parse_argument_number("Song number", &argument[2..])?,
            _ => ()
        }
    }

    player.set_device_number(device_number);
    player.init_devices()?;

    if display_devices {
        print_device_names(player.get_device_names());
        return Ok(());
    }

    player.set_song_number(song_number);
    player.load_file(filename)?;
    player.setup_sldb_and_stil(hvsc_location, display_stil)?;

    let version = player.get_library_version();
    if version < 0x202 {
        return Err("acid64pro.dll version 2.02 or higher required.".to_string());
    }

    println!("ACID64 library version v{}.{:02}", version >> 8, version & 0xff);

    let mut console_player = ConsolePlayer::new(player, display_stil);
    console_player.play()?;
    Ok(())
}

fn print_usage() {
    println!("ACID64 Console v1.04 - Copyright (c) 2003-2019 Wilfred Bos");
    println!("\nUsage: acid64c <options> <file_name>");
    println!("\n<Options>");
    println!("  -d{{device_number}}: set device number (1..n), default is 1");
    println!("  -h{{host_name}}: host name or ip of network sid device, default is localhost");
    println!("  -i: display STIL info if present");
    println!("  -l{{hvsc_location}}: specify the HVSC location for song length and STIL info");
    println!("  -p: print available devices");
    println!("  -s{{song_number}}: set song number (1..n), default is start song in SID file");
}

fn print_device_names(device_names: Vec<String>) {
    if device_names.len() > 0 {
        println!("Available devices:");
        for (i, device_name) in device_names.iter().enumerate() {
            println!("{:2}: {}", i + 1, device_name);
        }
    } else {
        println!("No devices were found.");
    }
}