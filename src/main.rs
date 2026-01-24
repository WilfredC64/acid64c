// Copyright (C) 2019 - 2025 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod config;
mod console_player;
mod player;
mod utils;

use std::env;
use std::process::exit;
use self::config::Config;
use self::console_player::ConsolePlayer;
use self::player::Player;

fn main() {
    if env::args().count() <= 1 {
        print_usage();
        return;
    }

    match run() {
        Ok(_) => {}
        Err(message) => {
            eprintln!("\nERROR: {message}");
            exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let mut player = Player::new();

    let version = player.get_library_version();
    if version < 0x210 {
        return Err("acid64pro.dll version 2.1.0 or higher required.".to_string());
    }

    print_library_version(version);

    let config = Config::read()?;

    if config.adjust_clock {
        player.set_adjust_clock(true);
    }

    if let Some(host_name) = config.host_name_sid_device {
        player.set_sid_device_host_name(host_name);
    }

    if let Some(host_name) = config.host_name_ultimate_device {
        player.set_ultimate_device_host_name(host_name);
    }

    player.set_device_numbers(&config.device_numbers);

    if config.display_devices {
        player.setup_c64_instance()?;
        player.init_devices()?;
        let device_names = player.get_device_names();
        print_device_names(device_names.lock().to_vec());
        return Ok(());
    }

    player.setup_sldb_and_stil(config.hvsc_location, config.display_stil)?;
    player.set_file_name(&config.filename);
    player.set_song_to_play(config.song_number);

    let mut console_player = ConsolePlayer::new(player, config.display_stil);
    console_player.play()?;
    Ok(())
}

fn print_usage() {
    println!("ACID64 Console v1.10 - Copyright (c) 2003-2025 Wilfred Bos");
    println!("\nUsage: acid64c <options> <file_name>");
    println!("\n<Options>");
    println!("  -c: adjust clock for devices that don't support PAL/NTSC clock");
    println!("  -d{{device_number,n}}: set device numbers (1..n) for each SID chip, default is 1");
    println!("  -hs{{host_name}}: host name or IP of network sid device, default is localhost");
    println!("  -hu{{ip_address}}: IP of Ultimate device");
    println!("  -i: display STIL info if present");
    println!("  -l{{hvsc_location}}: specify the HVSC location for song length and STIL info");
    println!("  -p: print available devices");
    println!("  -s{{song_number}}: set song number (1..n), default is start song in SID file");
}

fn print_device_names(device_names: Vec<String>) {
    if !device_names.is_empty() {
        println!("Available devices:");
        for (i, device_name) in device_names.iter().enumerate() {
            println!("{:2}: {}", i + 1, device_name);
        }
    } else {
        println!("No devices were found.");
    }
}

fn print_library_version(version: i32) {
    println!("ACID64 library version v{}.{}.{}", version >> 8, (version >> 4) & 0x0f, version & 0x0f);
}
