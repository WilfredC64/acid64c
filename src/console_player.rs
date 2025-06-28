// Copyright (C) 2019 - 2025 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod clock;

use crate::player::{Player, PlayerCommand, ABORT_NO, ABORT_TO_QUIT, ABORT_FOR_COMMAND, PlayerOutput, SidInfo, ABORTED};
use crate::utils::keyboard;
use self::clock::Clock;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::{thread, time::Duration};
use std::time::Instant;
use parking_lot::Mutex;

const LOOP_RATE_IN_MS: u64 = 50;
const FAST_FORWARD_STOP_DELAY_IN_MILLIS: u128 = 600;
const LOOP_TIME_OUT_MILLIS: u128 = 3000;

pub struct ConsolePlayer {
    player: Arc<Mutex<Player>>,
    player_cmd_sender: SyncSender<PlayerCommand>,
    display_stil: bool,
    paused: bool,
    abort_type: Arc<AtomicI32>,
    sid_loaded: Arc<AtomicBool>,
    fast_forward_in_progress: Arc<AtomicBool>,
    last_fast_forward: Arc<Mutex<Instant>>,
    player_output: Arc<Mutex<PlayerOutput>>,
    sid_info: Arc<Mutex<SidInfo>>,
    device_names: Arc<Mutex<Vec<String>>>
}

impl ConsolePlayer {
    pub fn new(player: Player, display_stil: bool) -> ConsolePlayer {
        let fast_forward_in_progress = Arc::new(AtomicBool::new(false));
        let last_fast_forward = Arc::new(Mutex::new(Instant::now()));

        let player_cmd_sender = player.get_channel_sender();
        let player_arc = Arc::new(Mutex::new(player));
        let player_output = player_arc.lock().get_player_output();
        let sid_info = player_arc.lock().get_sid_info_ref();
        let abort_type = player_arc.lock().get_aborted_ref();
        let sid_loaded = player_arc.lock().get_sid_loaded_ref();
        let device_names = player_arc.lock().get_device_names();

        ConsolePlayer {
            player: player_arc,
            player_cmd_sender,
            display_stil,
            paused: false,
            abort_type,
            sid_loaded,
            fast_forward_in_progress,
            last_fast_forward,
            player_output,
            sid_info,
            device_names
        }
    }

    pub fn play(&mut self) -> Result<(), String> {
        let mut clock = Clock::new();
        let mut player_thread = self.start_player(&mut clock);

        let last_error = self.get_last_error();
        if let Some(last_error) = last_error {
            self.abort_type.store(ABORTED, Ordering::SeqCst);
            return Err(last_error);
        }

        let remote_sidplayer_active = self.player_output.lock().has_remote_sidplayer;

        self.print_info();

        let song_length = self.sid_info.lock().song_length;
        self.display_clock(&mut clock, song_length);
        clock.start();

        let number_of_tunes = self.sid_info.lock().number_of_songs;
        self.paused = false;

        loop {
            if let Some(key) = keyboard::get_char_from_input() {
                match key {
                    'p' | 'P' => {
                        self.disable_fast_forward(&mut clock);
                        self.pause_or_resume_player();
                        clock.pause(self.paused);

                        if !self.paused {
                            if remote_sidplayer_active {
                                clock.set_clock(0);
                            } else {
                                clock.set_clock(self.player_output.lock().time as usize);
                            }
                        }
                    },
                    '0' ..= '9' | '+' | '=' | '-' | '_' => {
                        let mut song_number = keyboard::convert_num_key_to_number(key);
                        let invalid_song_nr = song_number != -1 && number_of_tunes - 1 < song_number;

                        if !invalid_song_nr || song_number == -1 {
                            self.stop_player(player_thread);
                            song_number = match key {
                                '+' | '=' => self.player.lock().get_next_song(),
                                '-' | '_' => self.player.lock().get_prev_song(),
                                _ => song_number
                            };

                            let old_song_number = self.player_output.lock().song_number;

                            self.player.lock().set_song_to_play(song_number);
                            player_thread = self.start_player(&mut clock);

                            clock.stop();
                            if old_song_number != song_number {
                                self.refresh_info();
                            }
                            clock.start();

                            keyboard::flush_keyboard_buffer();
                        }
                    },
                    keyboard::RIGHT_KEY => {
                        if !remote_sidplayer_active {
                            self.toggle_fast_forward(&mut clock);
                            continue;
                        }
                    },
                    keyboard::LEFT_KEY => {
                        if !remote_sidplayer_active {
                            self.disable_fast_forward(&mut clock);
                            continue;
                        }
                    },
                    keyboard::ESC_KEY => break,
                    _ => ()
                };
            }

            if self.fast_forward_in_progress.load(Ordering::SeqCst) {
                clock.set_clock(self.player_output.lock().time as usize);
            }

            clock.refresh_clock();

            if self.is_aborted() {
                break;
            }
            thread::sleep(Duration::from_millis(LOOP_RATE_IN_MS));
        }

        clock.stop();
        self.stop_player(player_thread);
        self.player.lock().stop_player();

        let last_error = self.player.lock().get_last_error();
        if let Some(last_error) = last_error {
            return Err(format!("{last_error}\nExiting!"));
        }

        Ok(())
    }

    fn get_last_error(&self) -> Option<String> {
        self.player_output.lock().last_error.clone()
    }

    fn pause_or_resume_player(&mut self) {
        if self.paused {
            self.play_tune();
        } else {
            self.pause_tune();
        }
    }

    fn play_tune(&mut self) {
        self.send_command(PlayerCommand::Play);
        self.paused = false;
    }

    fn pause_tune(&mut self) {
        self.send_command(PlayerCommand::Pause);
        self.paused = true;
    }

    fn enable_fast_forward(&mut self) {
        let ff_in_progress = self.fast_forward_in_progress.load(Ordering::SeqCst);
        if !ff_in_progress {
            if !self.is_aborted() {
                self.send_command(PlayerCommand::EnableFastForward);
            } else {
                self.player.lock().enable_fast_forward();
            }
            self.fast_forward_in_progress.store(true, Ordering::SeqCst);

            if self.paused {
                self.play_tune();
            }
        }
    }

    fn disable_fast_forward(&mut self, clock: &mut Clock) {
        let ff_in_progress = self.fast_forward_in_progress.load(Ordering::SeqCst);
        if ff_in_progress {
            if !self.is_aborted() {
                self.send_command(PlayerCommand::DisableFastForward);
            } else {
                self.player.lock().disable_fast_forward();
            }
            self.fast_forward_in_progress.store(false, Ordering::SeqCst);

            clock.set_clock(self.player_output.lock().time as usize);
        }
    }

    fn toggle_fast_forward(&mut self, clock: &mut Clock) {
        let ff_in_progress = self.fast_forward_in_progress.load(Ordering::SeqCst);
        if !ff_in_progress {
            *self.last_fast_forward.lock() = Instant::now();
            self.enable_fast_forward();
        } else if self.last_fast_forward.lock().elapsed().as_millis() > FAST_FORWARD_STOP_DELAY_IN_MILLIS {
            self.disable_fast_forward(clock);
        } else {
            *self.last_fast_forward.lock() = Instant::now();
        }
    }

    fn stop_player(&mut self, player_thread: thread::JoinHandle<()>) {
        self.abort_type.store(ABORT_TO_QUIT, Ordering::SeqCst);
        let _ = player_thread.join();
        self.abort_type.store(ABORTED, Ordering::SeqCst);
    }

    fn start_player(&mut self, clock: &mut Clock) -> thread::JoinHandle<()> {
        self.paused = false;

        self.disable_fast_forward(clock);

        self.abort_type.store(ABORT_NO, Ordering::SeqCst);
        self.sid_loaded.store(false, Ordering::SeqCst);

        let player_clone = Arc::clone(&self.player);
        let player_thread = thread::spawn(move || {
            player_clone.lock().play();
        });

        if let Err(e) = self.wait_until_sid_is_loaded() {
            self.player_output.lock().last_error = Some(e);
            self.abort_type.store(ABORTED, Ordering::SeqCst);
        }

        player_thread
    }

    fn wait_until_sid_is_loaded(&mut self) -> Result<(), String> {
        let start_time = Instant::now();
        while !self.sid_loaded.load(Ordering::SeqCst) && !self.is_aborted() {
            thread::sleep(Duration::from_millis(LOOP_RATE_IN_MS));

            if start_time.elapsed().as_millis() > LOOP_TIME_OUT_MILLIS {
                return Err("Timeout while loading SID file".to_string());
            }
        }
        Ok(())
    }

    fn is_aborted(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type != ABORT_NO
    }

    fn refresh_info(&mut self) {
        println!();
        self.print_info();
        let song_length_in_milli = self.sid_info.lock().song_length;
        let clock_display = Self::get_clock_display(song_length_in_milli);
        print!("{clock_display}");
    }

    fn send_command(&mut self, command: PlayerCommand) {
        self.abort_type.store(ABORT_FOR_COMMAND, Ordering::SeqCst);
        let _ = self.player_cmd_sender.send(command);
    }

    fn convert_song_length(song_length: i32) -> String {
        let song_length_in_seconds = (song_length + 500) / 1000;
        Clock::convert_seconds_to_time_string(song_length_in_seconds as u32, false)
    }

    fn display_clock(&mut self, clock: &mut Clock, song_length_in_milli: i32) {
        let clock_display = ConsolePlayer::get_clock_display(song_length_in_milli);
        clock.set_clock_display_length(clock_display.len() - 1);
        print!("{clock_display}");
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
        let filename = self.sid_info.lock().filename.clone();
        let path = Path::new(&filename);
        println!("\nFile            : {}", path.file_name().unwrap().to_str().unwrap());
    }

    fn print_sid_model(&mut self) {
        let sid_model = self.sid_info.lock().sid_models[0];
        let sid_model_display = match sid_model {
            1 => "MOS 6581",
            2 => "MOS 8580",
            3 => "MOS 6581/8580",
            _ => "Unknown"
        };
        println!("SID Model       : {sid_model_display}");
    }

    fn print_c64_model(&mut self) {
        let c64_model = self.sid_info.lock().clock_frequency;
        let c64_model_display = match c64_model {
            1 => "PAL",
            2 => "NTSC",
            3 => "PAL/NTSC",
            _ => "Unknown"
        };
        println!("Clock Frequency : {c64_model_display}");
    }

    fn print_sid_description(&mut self) {
        let sid_info = self.sid_info.lock();
        let title = sid_info.title.to_string();
        let author = sid_info.author.to_string();
        let released = sid_info.released.to_string();

        if (title.len() > 32) && author.is_empty() && released.is_empty() {
            println!("\n       Sidplayer 64 info");
            println!("================================");
            println!("{}", title.trim_end());
        } else {
            println!("\nTitle           : {title}");
            println!("Author          : {author}");
            println!("Released        : {released}");
        }
    }

    fn print_stil_info(&mut self) {
        if self.display_stil {
            let sid_info = self.sid_info.lock();
            if let Some(stil_entry) = &sid_info.stil_entry {
                println!("\nSTIL Info");
                println!("---------\n{stil_entry}");
            }
        }
    }

    fn print_device_info(&mut self) {
        let sid_info= self.sid_info.lock();
        let player_output = self.player_output.lock();
        let number_of_songs = sid_info.number_of_songs;
        let number_of_sids = sid_info.number_of_sids;
        let song_number = player_output.song_number;
        let device_number = player_output.device_number;

        if number_of_sids > 1 {
            println!("\nPlaying song {} of {} on devices:", song_number + 1, number_of_songs);
            for i in 0..number_of_sids {
                println!("SID {} -> {:>2}: {}", i + 1, device_number + 1, self.device_names.lock()[device_number as usize]);
            }
        } else {
            println!("\nPlaying song {} of {} on device {}: {}", song_number + 1, number_of_songs, device_number + 1, self.device_names.lock()[device_number as usize]);
        }
    }
}
