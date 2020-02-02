// Copyright (C) 2019 - 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod clock;

use crate::player::{Player, PlayerCommand};
use crate::utils::keyboard;
use self::clock::Clock;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};

const LOOP_RATE_IN_MS: u64 = 100;

pub struct ConsolePlayer {
    player: Arc<Mutex<Player>>,
    player_cmd_sender: SyncSender<PlayerCommand>,
    display_stil: bool,
    paused: bool,
    aborted: Arc<AtomicBool>
}

impl ConsolePlayer {
    pub fn new(player: Player, display_stil: bool) -> ConsolePlayer {
        let player_cmd_sender = player.get_channel_sender();
        let player_arc = Arc::new(Mutex::new(player));
        let aborted = player_arc.lock().unwrap().get_aborted_ref();

        ConsolePlayer {
            player: player_arc,
            player_cmd_sender,
            display_stil,
            paused: false,
            aborted
        }
    }

    pub fn play(&mut self) -> Result<(), String> {
        self.print_info();

        let mut clock = self.setup_and_display_clock();
        clock.start();

        let number_of_tunes = self.player.lock().unwrap().get_number_of_songs();
        let mut player_thread = self.start_player();

        self.paused = false;
        loop {
            if let Some(key) = keyboard::get_char_from_input() {
                match key {
                    'p' => {
                        self.pause_or_resume_player();
                        clock.pause(self.paused);
                    },
                    '0' ..= '9' | '+' | '=' | '-' | '_' => {
                        let mut song_number = keyboard::convert_num_key_to_number(key);
                        let invalid_song_nr = song_number != -1 && number_of_tunes - 1 < song_number;

                        if invalid_song_nr == false || song_number == -1 {
                            self.stop_player(player_thread);
                            song_number = match key {
                                '+' | '=' => self.player.lock().unwrap().get_next_song(),
                                '-' | '_' => self.player.lock().unwrap().get_prev_song(),
                                _ => song_number
                            };

                            self.player.lock().unwrap().set_song_to_play(song_number)?;
                            self.refresh_info(&mut clock);
                            player_thread = self.start_player();
                        }
                    },
                    keyboard::ESC_KEY => break,
                    _ => ()
                };
            }

            clock.refresh_clock();

            if self.is_aborted() {
                break;
            }
            thread::sleep(Duration::from_millis(LOOP_RATE_IN_MS));
        }

        clock.stop();
        self.stop_player(player_thread);
        Ok(())
    }

    fn pause_or_resume_player(&mut self) -> () {
        if self.paused {
            self.send_command(PlayerCommand::Play);
        } else {
            self.send_command(PlayerCommand::Pause);
        }

        self.paused = !self.paused;
    }

    #[inline]
    fn stop_player(&mut self, player_thread: thread::JoinHandle<()>) {
        self.aborted.store(true, Ordering::SeqCst);
        let _ = player_thread.join();
    }

    #[inline]
    fn start_player(&mut self) -> thread::JoinHandle<()> {
        self.aborted.store(false, Ordering::SeqCst);

        let player_clone = Arc::clone(&self.player);
        let player_thread = thread::spawn(move || {
            player_clone.lock().unwrap().play();
        });
        player_thread
    }

    #[inline]
    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    fn refresh_info(&mut self, clock: &mut Clock) {
        clock.stop();
        self.print_info();
        let song_length_in_milli = self.player.lock().unwrap().get_song_length();
        let clock_display = Self::get_clock_display(song_length_in_milli);
        print!("{}", clock_display);
        clock.start();
    }

    #[inline]
    fn send_command(&mut self, command: PlayerCommand) {
        let _ = self.player_cmd_sender.send(command);
    }

    fn convert_song_length(song_length: i32) -> String {
        let song_length_in_seconds = (song_length + 500) / 1000;
        Clock::convert_seconds_to_time_string(song_length_in_seconds as u32, false)
    }

    fn setup_and_display_clock(&mut self) -> Clock {
        let song_length_in_milli = self.player.lock().unwrap().get_song_length();
        let clock_display = ConsolePlayer::get_clock_display(song_length_in_milli);
        print!("{}", clock_display);

        let mut clock = Clock::new();
        clock.set_clock_display_length(clock_display.len() - 1);
        clock
    }

    fn get_clock_display(song_length_in_milli: i32) -> String {
        if song_length_in_milli > 0 {
            format!("(00:00 - {})", ConsolePlayer::convert_song_length(song_length_in_milli))
        } else {
            "(00:00)".to_string()
        }
    }

    pub fn print_info(&mut self) {
        self.print_filename();
        self.print_sid_model();
        self.print_c64_model();
        self.print_sid_description();
        self.print_stil_info();
        self.print_device_info();

        print!("\nPress escape key to exit... ");
    }

    fn print_filename(&mut self) {
        let filename = self.player.lock().unwrap().get_filename();
        if let Some(filename) = filename {
            let path = Path::new(&filename);
            println!("\nFile            : {}", path.file_name().unwrap().to_str().unwrap());
        }
    }

    fn print_sid_model(&mut self) {
        let sid_model = self.player.lock().unwrap().get_sid_model();
        let sid_model_display = match sid_model {
            1 => "MOS 6581",
            2 => "MOS 8580",
            3 => "MOS 6581/8580",
            _ => "Unknown"
        };
        println!("SID Model       : {}", sid_model_display);
    }

    fn print_c64_model(&mut self) {
        let c64_model = self.player.lock().unwrap().get_c64_version();
        let c64_model_display = match c64_model {
            1 => "PAL",
            2 => "NTSC",
            3 => "PAL/NTSC",
            _ => "Unknown"
        };
        println!("Clock Frequency : {}", c64_model_display);
    }

    fn print_sid_description(&mut self) {
        let mut player = self.player.lock().unwrap();
        let title = player.get_title();
        let author = player.get_author();
        let released = player.get_released();

        if (title.len() > 32) && (author.len() == 0) && (released.len() == 0) {
            println!("\n       Sidplayer 64 info");
            println!("================================");
            println!("{}", title.trim_end());
        } else {
            println!("\nTitle           : {}", title);
            println!("Author          : {}", author);
            println!("Released        : {}", released);
        }
    }

    fn print_stil_info(&mut self) {
        if self.display_stil {
            let stil_entry = self.player.lock().unwrap().get_stil_entry();
            if stil_entry.is_some() {
                println!("\nSTIL Info");
                println!("---------\n{}", stil_entry.unwrap());
            }
        }
    }

    fn print_device_info(&mut self) {
        let mut player= self.player.lock().unwrap();
        let device_number = player.get_device_number();
        let song_number = player.get_song_number();
        let number_of_songs = player.get_number_of_songs();
        let device_info = player.get_device_info(device_number);

        println!("\nPlaying song {} of {} on device {}: {}", song_number + 1, number_of_songs, device_number + 1, device_info);
    }
}
