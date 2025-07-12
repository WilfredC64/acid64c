// Copyright (C) 2019 - 2025 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

pub mod sid_device;

mod acid64_library;
mod clock_adjust;
mod hardsid_usb;
mod hardsid_usb_device;
mod network_sid_device;
mod sidblaster_usb_device;
mod sidblaster_scheduler;
mod sid_data_processor;
mod sid_devices;
mod sid_info;
mod sldb;
mod stil;
mod ultimate_device;

use parking_lot::Mutex;
use std::fs::read;
use std::io::Error;
use std::sync::atomic::{Ordering, AtomicI32, AtomicBool};
use std::sync::Arc;
use std::{thread, time};
use std::collections::VecDeque;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use thread_priority::{set_current_thread_priority, ThreadPriority};
#[cfg(windows)]
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

use self::acid64_library::Acid64Library;
use self::sid_data_processor::{SidDataProcessor, SidWrite};
use self::sid_device::{DeviceResponse, DUMMY_REG, SamplingMethod, SidClock, SidDevice, SidModel};
use self::sid_devices::{SidDevices, SidDevicesFacade};
use self::stil::Stil;
use self::sldb::Sldb;

use crate::utils::hvsc;
pub use self::sid_info::SidInfo;

const PAL_CYCLES_PER_SECOND: u32 = 312 * 63 * 50;
const NTSC_CYCLES_PER_SECOND: u32 = 263 * 65 * 60;
const ONE_MHZ_CYCLES_PER_SECOND: u32 = 1000000;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT_NUMBER: &str = "6581";

const DEFAULT_ULTIMATE_HOST: &str = "";
const DEFAULT_ULTIMATE_PORT_NUMBER: &str = "80";

const MIN_CYCLE_SID_WRITE: u32 = 8;
const MIN_CYCLE_SID_WRITE_FAST_FORWARD: u32 = 8;

const SID_MODEL_8580: i32 = 2;

const BUSY_WAIT_MILLIS: u64 = 1;
const PAUSE_SLEEP_MILLIS: u64 = 10;
const ABORT_DEVICE_DELAY_MILLIS: u64 = 20;

const DEFAULT_SONG_LENGTH_IN_MILLIS: i32 = 300000;

pub const ABORT_NO: AbortType = 0;
pub const ABORT_TO_QUIT: AbortType = 1;
pub const ABORT_FOR_COMMAND: AbortType = 2;
pub const ABORTING: AbortType = 3;
pub const ABORTED: AbortType = 4;
pub type AbortType = i32;

#[allow(dead_code)]
pub enum PlayerCommand {
    Play,
    Pause,
    Stop,
    EnableFastForward,
    DisableFastForward
}

#[derive(Copy, Clone)]
enum SidCommand {
    Idle = 0,
    Delay,
    Write,
    Read,
    NextPart,
    InitDone,
    SeekDone,
    SkipSilenceDone
}

impl SidCommand {
    pub fn from_integer(value: i32) -> SidCommand {
        match value {
            0 => SidCommand::Idle,
            1 => SidCommand::Delay,
            2 => SidCommand::Write,
            3 => SidCommand::Read,
            4 => SidCommand::NextPart,
            5 => SidCommand::InitDone,
            6 => SidCommand::SeekDone,
            7 => SidCommand::SkipSilenceDone,
            _ => panic!("Unknown value: {value}"),
        }
    }
}

#[derive(Default)]
pub struct PlayerOutput {
    pub time: u32,
    pub device_number: i32,
    pub song_number: i32,
    pub has_remote_sidplayer: bool,
    pub last_error: Option<String>
}

pub struct Player {
    acid64_lib: Acid64Library,
    c64_instance: usize,
    sid_device: Option<Box<dyn SidDevice + Send>>,
    sid_data_processor: SidDataProcessor,
    filename: Option<String>,
    md5_hash: String,
    device_number: i32,
    device_numbers: Vec<i32>,
    song_number: i32,
    host_name_sid_device: String,
    port_sid_device: String,
    host_name_ultimate: String,
    port_ultimate: String,
    abort_type: Arc<AtomicI32>,
    cmd_sender: SyncSender<PlayerCommand>,
    cmd_receiver: Receiver<PlayerCommand>,
    paused: bool,
    sid_written: bool,
    last_sid_write: [u8; 256],
    redo_buffer: VecDeque<SidWrite>,
    device_names: Arc<Mutex<Vec<String>>>,
    adjust_clock: bool,
    fast_forward_speed: i32,
    total_cycles: u32,
    output: Arc<Mutex<PlayerOutput>>,
    sid_info: Arc<Mutex<SidInfo>>,
    stil: Stil,
    sldb: Sldb
}

impl Drop for Player {
    fn drop(&mut self) {
        self.close_c64_instance();

        #[cfg(windows)]
        unsafe {
            timeEndPeriod(1);
        }
    }
}

impl Player {
    pub fn new() -> Player {
        #[cfg(windows)]
        unsafe {
            timeBeginPeriod(1);
        }

        let (cmd_sender, cmd_receiver) = sync_channel(0);

        Player {
            acid64_lib: Acid64Library::load().expect("acid64pro library could not be loaded"),
            c64_instance: 0,
            sid_device: None,
            sid_data_processor: SidDataProcessor::new(),
            filename: None,
            md5_hash: "".to_string(),
            device_number: 0,
            device_numbers: vec![],
            song_number: 0,
            host_name_sid_device: DEFAULT_HOST.to_string(),
            port_sid_device: DEFAULT_PORT_NUMBER.to_string(),
            host_name_ultimate: DEFAULT_ULTIMATE_HOST.to_string(),
            port_ultimate: DEFAULT_ULTIMATE_PORT_NUMBER.to_string(),
            abort_type: Arc::new(AtomicI32::new(ABORT_NO)),
            cmd_sender,
            cmd_receiver,
            paused: false,
            sid_written: false,
            last_sid_write: [0; 256],
            redo_buffer: VecDeque::new(),
            device_names: Arc::new(Mutex::new(Vec::new())),
            adjust_clock: false,
            fast_forward_speed: 1,
            total_cycles: 0,
            output: Arc::new(Mutex::new(PlayerOutput::default())),
            sid_info: Arc::new(Mutex::new(SidInfo::new())),
            stil: Stil::new(),
            sldb: Sldb::new()
        }
    }

    pub fn get_channel_sender(&self) -> SyncSender<PlayerCommand> {
        SyncSender::clone(&self.cmd_sender)
    }

    pub fn set_device_numbers(&mut self, device_numbers: &[i32]) {
        self.device_number = *device_numbers.first().unwrap_or(&-1);

        self.device_numbers = device_numbers.to_owned();
    }

    pub fn set_sid_device_host_name(&mut self, host_name: String) {
        self.host_name_sid_device = host_name;
    }

    pub fn set_ultimate_device_host_name(&mut self, host_name: String) {
        self.host_name_ultimate = host_name;
    }

    pub fn get_library_version(&self) -> i32 {
        self.acid64_lib.get_version()
    }

    pub fn get_aborted_ref(&self) -> Arc<AtomicI32> {
        Arc::clone(&self.abort_type)
    }

    pub fn get_sid_info_ref(&mut self) -> Arc<Mutex<SidInfo>> {
        Arc::clone(&self.sid_info)
    }

    fn close_c64_instance(&mut self) {
        if self.c64_instance > 0 {
            self.acid64_lib.close_c64_instance(self.c64_instance);
            self.c64_instance = 0;
        }
    }

    pub fn play(&mut self, sid_loaded: Arc<AtomicBool>) {
        self.setup_c64_instance();
        self.play_loop(sid_loaded);
        self.close_c64_instance();
    }

    pub fn play_loop(&mut self, sid_loaded: Arc<AtomicBool>) {
        let loaded = self.load_file();

        if loaded.is_err() {
            self.abort_type.store(ABORTED, Ordering::SeqCst);
            self.output.lock().last_error = Some(loaded.err().unwrap());
            return;
        }

        let inited = self.init_song(self.song_number);

        if inited.is_err() {
            self.abort_type.store(ABORTED, Ordering::SeqCst);
            self.output.lock().last_error = Some(inited.err().unwrap());
            return;
        }

        sid_loaded.store(true, Ordering::SeqCst);

        let cycles_per_second = self.get_cycles_per_second();

        let mut idle_count: u32 = 0;

        self.total_cycles = 0;
        self.sid_written = false;
        self.paused = false;
        self.abort_type.store(ABORT_NO, Ordering::SeqCst);

        self.redo_buffer.clear();

        if self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number) {
            if let Some(filename) = self.filename.clone() {
                self.send_sid(&filename, self.song_number);
            }
        }

        self.sid_data_processor.init(0);
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, 0);

        let mut device_state = DeviceResponse::Ok;

        let _ = set_current_thread_priority(ThreadPriority::Max);

        while !self.should_quit() {
            self.process_player_command();

            if self.paused {
                thread::sleep(time::Duration::from_millis(PAUSE_SLEEP_MILLIS));
                continue;
            }

            if device_state == DeviceResponse::Busy {
                if self.should_quit() {
                    break;
                }

                self.update_player_output();

                let next_event = self.sid_data_processor.get_next_event_in_millis();
                if next_event >= 10 {
                    thread::sleep(time::Duration::from_millis(BUSY_WAIT_MILLIS));
                }

                device_state = self.sid_device.as_mut().unwrap().retry_write(self.device_number);
                continue;
            }

            if !self.redo_buffer.is_empty() {
                device_state = self.process_redo_buffer();

                self.update_player_output();
            } else {
                self.acid64_lib.run(self.c64_instance);
                self.update_player_output();
                let sid_command = SidCommand::from_integer(self.acid64_lib.get_command(self.c64_instance));

                match sid_command {
                    SidCommand::Delay => {
                        device_state = self.process_sid_write(DUMMY_REG, 0);
                    },
                    SidCommand::Write => {
                        let reg = self.acid64_lib.get_register(self.c64_instance);
                        let data = self.acid64_lib.get_data(self.c64_instance);

                        device_state = self.process_sid_write(reg, data);
                        idle_count = 0;
                    },
                    SidCommand::Read => {
                        idle_count = 0;
                    },
                    SidCommand::Idle => {
                        if self.sid_written {
                            idle_count += cycles_per_second / 1000;

                            if idle_count >= cycles_per_second {
                                self.sid_device.as_mut().unwrap().dummy_write(self.device_number, cycles_per_second);
                                idle_count -= cycles_per_second
                            }
                        }
                    },
                    _ => (),
                }
            }
        };

        self.abort_type.store(ABORTING, Ordering::SeqCst);

        if self.sid_device.as_mut().unwrap().is_connected(self.device_number) {
            self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
            thread::sleep(time::Duration::from_millis(ABORT_DEVICE_DELAY_MILLIS));
            self.sid_device.as_mut().unwrap().silent_all_sids(self.device_number, true);
        }

        self.fast_forward_speed = 1;

        self.abort_type.store(ABORTED, Ordering::SeqCst);
    }

    pub fn stop_player(&mut self) {
        if let Some(ref mut sid_device) = self.sid_device {
            if self.device_number != -1 && !self.paused && sid_device.has_remote_sidplayer(self.device_number) {
                if sid_device.is_connected(self.device_number) {
                    sid_device.stop_sid(self.device_number);
                } else {
                    self.abort_type.store(ABORT_TO_QUIT, Ordering::SeqCst);
                }
            }
        }
    }

    pub fn get_device_names(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.device_names)
    }

    pub fn get_last_error(&mut self) -> Option<String> {
        self.sid_device.as_mut().unwrap().get_last_error(self.device_number)
    }

    pub fn get_player_output(&mut self) -> Arc<Mutex<PlayerOutput>> {
        Arc::clone(&self.output)
    }

    pub fn enable_fast_forward(&mut self) {
        if !self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number) {
            self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
            self.fast_forward_speed = -1;
            self.sid_device.as_mut().unwrap().enable_turbo_mode(self.device_number);
            self.rewrite_buffer();
        }
    }

    pub fn disable_fast_forward(&mut self) {
        if !self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number) {
            self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
            self.fast_forward_speed = 1;
            self.sid_device.as_mut().unwrap().disable_turbo_mode(self.device_number);
            self.rewrite_buffer();
        }
    }

    pub fn setup_sldb_and_stil(&mut self, hvsc_location: Option<String>, load_stil: bool) -> Result<(), String> {
        let mut hvsc_root = self.get_hvsc_root_location(hvsc_location)?;

        if hvsc_root.is_none() {
            if let Some(filename) = &self.filename {
                hvsc_root = hvsc::get_hvsc_root(filename);
            }
        }

        if let Some(hvsc_root) = hvsc_root {
            self.sldb.load(&hvsc_root)?;

            if load_stil {
                self.stil.load(&hvsc_root)?;
            }
        }
        Ok(())
    }

    pub fn set_adjust_clock(&mut self, adjust_clock: bool) {
        self.adjust_clock = adjust_clock;
    }

    pub fn init_devices(&mut self) -> Result<(), String> {
        if self.sid_device.is_none() {
            let mut devices = SidDevices::new(Arc::clone(&self.abort_type))
                .connect_hardsid_device()
                .connect_sidblaster()
                .connect_network_device(&self.host_name_sid_device, &self.port_sid_device)
                .connect_ultimate_device(&self.host_name_ultimate, &self.port_ultimate);

            if !devices.has_devices() && devices.has_errors() {
                return Err(devices.errors());
            }

            devices.set_native_device_clock(!self.adjust_clock);

            let sid_device = SidDevicesFacade{ devices };
            self.sid_device = Some(Box::new(sid_device));

            self.refresh_device_names();
        }

        self.configure_sid_device(false)?;
        Ok(())
    }

    pub fn set_file_name(&mut self, filename: &str) {
        self.filename = Some(filename.to_string());
    }

    pub fn get_next_song(&self) -> i32 {
        let number_of_songs = self.get_number_of_songs();
        (self.song_number + 1) % number_of_songs
    }

    pub fn get_prev_song(&mut self) -> i32 {
        let number_of_songs = self.get_number_of_songs();
        (self.song_number + number_of_songs - 1) % number_of_songs
    }

    pub fn set_song_to_play(&mut self, song_number: i32) {
        self.song_number = song_number;
        self.output.lock().song_number = song_number;
    }

    fn setup_c64_instance(&mut self) {
        self.c64_instance = self.acid64_lib.create_c64_instance();

        if self.c64_instance == 0 {
            panic!("C64 instance couldn't be created.");
        }
    }

    fn is_aborted_for_command(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type == ABORT_FOR_COMMAND
    }

    fn send_sid(&mut self, filename: &str, song_number: i32) {
        let sid_data = if filename.ends_with(".mus") || filename.ends_with(".str") {
            Self::read_mus_files(filename)
        } else {
            read(filename)
        };

        if let Ok(sid_data) = sid_data {
            self.acid64_lib.skip_silence(self.c64_instance, false);
            self.acid64_lib.enable_volume_fix(self.c64_instance, false);

            self.redo_buffer.clear();
            self.sid_data_processor.init(0);
            self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, 0);

            self.sid_written = false;

            let ssl_data = self.generate_ssl_data();
            self.sid_device.as_mut().unwrap().send_sid(self.device_number, filename, song_number, &sid_data, &ssl_data);

            if !self.sid_device.as_mut().unwrap().is_connected(self.device_number) {
                self.abort_type.store(ABORT_TO_QUIT, Ordering::SeqCst);
            }
        }
    }

    fn process_player_command(&mut self) {
        if self.is_aborted_for_command() {
            self.abort_type.store(ABORT_NO, Ordering::SeqCst);
        }

        let recv_result = self.cmd_receiver.try_recv();

        if let Ok(result) = recv_result {
            match result {
                PlayerCommand::Play => {
                    if self.paused {
                        self.sid_device.as_mut().unwrap().reset_active_sids(self.device_number);
                        self.reactivate_voices();
                        self.sid_device.as_mut().unwrap().force_flush(self.device_number);

                        self.rewrite_buffer();

                        if self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number) {
                            if let Some(filename) = self.filename.clone() {
                                let _ = self.restart_song();
                                self.send_sid(&filename, self.song_number);
                            }
                        }
                    }
                    self.paused = false;
                },
                PlayerCommand::Pause => {
                    let device = self.sid_device.as_mut().unwrap();
                    device.reset_all_buffers(self.device_number);
                    device.silent_all_sids(self.device_number, false);

                    self.stop_player();

                    self.paused = true;
                },
                PlayerCommand::EnableFastForward => {
                    self.enable_fast_forward();
                },
                PlayerCommand::DisableFastForward => {
                    self.disable_fast_forward();
                },
                _ => ()
            }
        }
    }

    fn process_redo_buffer(&mut self) -> DeviceResponse {
        let mut total_cycles = 0;

        while !self.redo_buffer.is_empty() {
            let sid_write = self.redo_buffer.pop_front().unwrap();
            let cycles = self.adjust_cycles(sid_write.cycles_real);

            self.sid_data_processor.write(cycles, sid_write.reg, sid_write.data, sid_write.cycles_real);

            let device_response = self.write_to_sid(self.device_number, cycles, sid_write.reg, sid_write.data);
            if device_response == DeviceResponse::Busy {
                return device_response;
            }

            total_cycles += cycles;
            if total_cycles > 1000 {
                break;
            }
        }

        let cycles_in_fifo = self.sid_data_processor.get_cycles_in_fifo();
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, cycles_in_fifo);

        if self.redo_buffer.is_empty() {
            self.sid_device.as_mut().unwrap().disable_turbo_mode(self.device_number);
        }

        DeviceResponse::Ok
    }

    fn read_mus_files(filename: &str) -> Result<Vec<u8>, Error> {
        if filename.ends_with(".mus") {
            if let Ok(data_mus) = read(filename) {
                let str_filename = filename.strip_suffix(".mus").unwrap().to_string() + ".str";
                if let Ok(data_str) = read(str_filename) {
                    Ok([data_mus, data_str].concat())
                } else {
                    Ok(data_mus)
                }
            } else {
                Err(Error::other("Error loading mus file"))
            }
        } else if filename.ends_with(".str") {
            if let Ok(data_str) = read(filename) {
                let mus_filename = filename.strip_suffix(".str").unwrap().to_string() + ".mus";
                if let Ok(data_mus) = read(mus_filename) {
                    Ok([data_mus, data_str].concat())
                } else {
                    Err(Error::other("Error loading mus file"))
                }
            } else {
                Err(Error::other("Error loading str file"))
            }
        } else {
            read(filename)
        }
    }

    fn generate_ssl_data(&mut self) -> Vec<u8>{
        let mut song_lengths_in_millis = vec![];
        for song_number in 0..self.get_number_of_songs() {
            self.acid64_lib.set_song_to_play(self.c64_instance, song_number);
            song_lengths_in_millis.push(self.get_song_length(song_number));
        }

        self.acid64_lib.set_song_to_play(self.c64_instance, self.song_number);

        let mut song_lengths_in_bcd = vec![];
        for song_length in song_lengths_in_millis {
            let seconds_total = (song_length + 500) / 1000;
            let seconds = seconds_total % 60;
            let seconds = Self::int_to_bcd(seconds);

            let minutes = seconds_total / 60 % 100;
            let minutes = Self::int_to_bcd(minutes);
            song_lengths_in_bcd.push(minutes as u8);
            song_lengths_in_bcd.push(seconds as u8);
        }
        song_lengths_in_bcd
    }

    fn int_to_bcd(value: i32) -> i32 {
        let mut value = value;
        let mut result = 0;
        let mut shift = 0;
        while value != 0 {
            result += (value % 10) << shift;
            value /= 10;
            shift += 4;
        }
        result
    }

    fn restart_song(&mut self) -> Result<(), String> {
        self.set_song_to_play(self.song_number);
        self.init_song(self.song_number)
    }

    fn update_player_output(&mut self) {
        self.sid_data_processor.process_sid_write_fifo();

        let mut output = self.output.lock();
        output.time = self.sid_data_processor.get_time_in_millis();
    }

    fn refresh_device_names(&mut self) {
        let mut device_names = Vec::new();

        let device_count = self.sid_device.as_mut().unwrap().get_device_count(self.device_number);
        for i in 0..device_count {
            let device_name = self.sid_device.as_mut().unwrap().get_device_info(i).name;
            device_names.push(device_name);
        }

        self.set_device_names(&device_names);
    }

    fn set_device_names(&mut self, new_device_names: &[String]) {
        let mut device_names = self.device_names.lock();
        device_names.clear();
        device_names.extend_from_slice(new_device_names);
    }

    fn get_cycles_per_second(&mut self) -> u32 {
        let device_clock = self.sid_device.as_mut().unwrap().get_device_clock(self.device_number);
        match device_clock {
            SidClock::Pal => PAL_CYCLES_PER_SECOND,
            SidClock::Ntsc => NTSC_CYCLES_PER_SECOND,
            SidClock::OneMhz => ONE_MHZ_CYCLES_PER_SECOND
        }
    }

    fn get_song_length(&self, song_number: i32) -> i32 {
        self.sldb.get_song_length(&self.md5_hash, song_number).unwrap_or(DEFAULT_SONG_LENGTH_IN_MILLIS)
    }

    fn get_stil_entry(&self) -> Option<String> {
        let hvsc_filename = self.sldb.get_hvsc_filename(&self.md5_hash);

        if let Some(hvsc_filename) = hvsc_filename {
            return self.stil.get_entry(&hvsc_filename);
        }
        None
    }

    fn get_number_of_songs(&self) -> i32 {
        self.sid_info.lock().number_of_songs
    }

    fn load_file(&mut self) -> Result<(), String> {
        if let Some(ref filename) = self.filename {
            let is_loaded = self.acid64_lib.load_file(self.c64_instance, filename);

            if !is_loaded {
                return Err(format!("File '{filename}' could not be loaded."))
            }

            if self.sldb.is_new_md5_hash_used() {
                self.md5_hash = self.acid64_lib.get_md5_hash(self.c64_instance);
            } else {
                self.md5_hash = self.acid64_lib.get_ancient_md5_hash(self.c64_instance);
            }

            self.retrieve_sid_info();

            self.init_devices()?;
            self.configure_sid_device(false)?;
            Ok(())
        } else {
            Err("Filename is not set.".to_string())
        }
    }

    fn retrieve_sid_info(&mut self) {
        let mut sid_info = self.sid_info.lock();
        sid_info.title = self.acid64_lib.get_title(self.c64_instance);
        sid_info.author = self.acid64_lib.get_author(self.c64_instance);
        sid_info.released = self.acid64_lib.get_released(self.c64_instance);
        sid_info.load_address = self.acid64_lib.get_load_address(self.c64_instance);
        sid_info.load_end_address = self.acid64_lib.get_load_end_address(self.c64_instance);
        sid_info.init_address = self.acid64_lib.get_init_address(self.c64_instance);
        sid_info.play_address = self.acid64_lib.get_play_address(self.c64_instance);
        sid_info.number_of_songs = self.acid64_lib.get_number_of_songs(self.c64_instance);
        sid_info.default_song = self.acid64_lib.get_default_song(self.c64_instance);
        sid_info.clock_frequency = self.acid64_lib.get_c64_version(self.c64_instance);
        sid_info.speed_flag = self.acid64_lib.get_speed_flag(self.c64_instance);
        sid_info.speed_flags = self.acid64_lib.get_speed_flags(self.c64_instance);
        sid_info.file_type = self.acid64_lib.get_file_type(self.c64_instance);
        sid_info.free_memory_address = self.acid64_lib.get_free_memory_address(self.c64_instance);
        sid_info.free_memory_end_address = self.acid64_lib.get_free_memory_end_address(self.c64_instance);
        sid_info.filename = self.filename.clone().unwrap_or_default();
        sid_info.file_format = self.acid64_lib.get_file_format(self.c64_instance);
        sid_info.basic_sid = self.acid64_lib.is_basic_sid(self.c64_instance);
        sid_info.md5_hash = self.md5_hash.clone();

        let song_length = self.get_song_length(self.song_number);
        sid_info.song_length = song_length;

        sid_info.stil_entry = self.get_stil_entry();

        self.set_sid_chip_info(&mut sid_info);
        self.set_mus_info(&mut sid_info);
    }

    fn set_sid_chip_info(&self, sid_info: &mut SidInfo) {
        let mut sid_models = Vec::new();
        let mut sid_addresses = Vec::new();

        let number_of_sids = self.acid64_lib.get_number_of_sids(self.c64_instance);
        for sid_nr in 0..number_of_sids {
            let sid_model = self.acid64_lib.get_sid_model(self.c64_instance, sid_nr);
            sid_models.push(sid_model);

            let sid_address = self.acid64_lib.get_sid_address(self.c64_instance, sid_nr);
            sid_addresses.push(sid_address);
        }

        sid_info.number_of_sids = number_of_sids;
        sid_info.sid_models = sid_models;
        sid_info.sid_addresses = sid_addresses;
    }

    fn set_mus_info(&self, sid_info: &mut SidInfo) {
        let mut mus_text = [0; 32*5];
        self.acid64_lib.get_mus_text(self.c64_instance, &mut mus_text);
        sid_info.mus_text = mus_text;

        let mut mus_colors = [0; 32 * 5];
        self.acid64_lib.get_mus_colors(self.c64_instance, &mut mus_colors);
        sid_info.mus_colors = mus_colors;
    }

    fn should_quit(&mut self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type == ABORT_TO_QUIT || !self.sid_device.as_mut().unwrap().is_connected(self.device_number)
    }

    fn process_sid_write(&mut self, reg: u8, data: u8) -> DeviceResponse {
        let cycles_real = self.acid64_lib.get_cycles(self.c64_instance) as u32;
        let cycles = self.adjust_cycles(cycles_real);

        self.total_cycles = cycles_real;
        self.last_sid_write[reg as usize] = data;

        self.sid_data_processor.write(cycles, reg, data, cycles_real);
        let cycles_in_fifo = self.sid_data_processor.get_cycles_in_fifo();
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, cycles_in_fifo);

        let device_response = self.write_to_sid(self.device_number, cycles, reg, data);
        if device_response == DeviceResponse::Busy {
            return device_response;
        }

        DeviceResponse::Ok
    }

    fn write_to_sid(&mut self, device_number: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.sid_device.as_mut().unwrap().try_write(device_number, cycles, reg, data)
    }

    fn write_to_sid_direct(&mut self, device_number: i32, cycles: u32, reg: u8, data: u8) {
        self.sid_device.as_mut().unwrap().write(device_number, cycles, reg, data);
    }

    fn rewrite_buffer(&mut self) {
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, 0);

        let buffer = self.sid_data_processor.get_buffer_copy();

        if !buffer.is_empty() {
            for sid_write in buffer.iter().rev() {
                self.redo_buffer.push_front(SidWrite::new(sid_write.reg, sid_write.data, sid_write.cycles, sid_write.cycles_real));
            }

            self.sid_data_processor.clear_buffer();

            self.sid_device.as_mut().unwrap().enable_turbo_mode(self.device_number);
        }
    }

    fn reactivate_voices(&mut self) {
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, 0);

        let number_of_sids = self.acid64_lib.get_number_of_sids(self.c64_instance);

        for sid_nr in 0..number_of_sids {
            let sid_base = (sid_nr * 0x20) as u8;

            self.reactivate_voice(0, sid_base);
            self.reactivate_voice(1, sid_base);
            self.reactivate_voice(2, sid_base);

            self.write_last_sid_write(sid_base + 0x15);
            self.write_last_sid_write(sid_base + 0x16);

            self.write_last_sid_write(sid_base + 0x17);
            self.write_last_sid_write(sid_base + 0x18);
        }
    }

    fn reactivate_voice(&mut self, voice_nr: u8, sid_base: u8) {
        let voice_offset = voice_nr * 7;
        let reg_base = sid_base + voice_offset;

        self.write_last_sid_write(reg_base + 0x03);
        self.write_last_sid_write(reg_base + 0x02);

        self.write_last_sid_write(reg_base + 0x05);
        self.write_last_sid_write(reg_base + 0x06);

        let data_ctrl_reg = self.sid_data_processor.get_last_sid_write(reg_base + 0x04);

        if data_ctrl_reg & 0x01 == 0x00 {
            if !self.sid_data_processor.is_note_finished(reg_base) {
                self.write_to_sid_direct(self.device_number, MIN_CYCLE_SID_WRITE, reg_base + 0x04, data_ctrl_reg | 0x01);
                self.write_to_sid_direct(self.device_number, 40000, reg_base + 0x04, data_ctrl_reg);
            }
        } else {
            self.write_to_sid_direct(self.device_number, MIN_CYCLE_SID_WRITE, reg_base + 0x04, data_ctrl_reg);
        }

        self.write_last_sid_write(reg_base);
        self.write_last_sid_write(reg_base + 0x01);
    }

    fn write_last_sid_write(&mut self, reg: u8) {
        self.write_to_sid(self.device_number, MIN_CYCLE_SID_WRITE, reg, self.last_sid_write[reg as usize]);
    }

    fn adjust_cycles(&mut self, cycles: u32) -> u32 {
        if self.fast_forward_speed == -1 {
            MIN_CYCLE_SID_WRITE_FAST_FORWARD
        } else if self.fast_forward_speed > 1 && cycles > MIN_CYCLE_SID_WRITE_FAST_FORWARD {
            let ff_cycles = cycles / (self.fast_forward_speed as u32);
            if ff_cycles < MIN_CYCLE_SID_WRITE_FAST_FORWARD {
                MIN_CYCLE_SID_WRITE_FAST_FORWARD
            } else {
                ff_cycles
            }
        } else {
            cycles
        }
    }

    fn get_hvsc_root_location(&self, hvsc_location: Option<String>) -> Result<Option<String>, String> {
        if let Some(hvsc_location) = hvsc_location {
            let hvsc_root = hvsc::get_hvsc_root(&hvsc_location);

            if hvsc_root.is_none() {
                return Err("Specified HVSC location is not valid.".to_string());
            }
            return Ok(hvsc_root);
        }
        Ok(None)
    }

    fn configure_sid_device(&mut self, should_reset: bool) -> Result<(), String> {
        let number_of_sids = self.acid64_lib.get_number_of_sids(self.c64_instance);
        self.fix_device_numbers(number_of_sids)?;

        self.sid_device.as_mut().unwrap().set_sid_count(self.device_number, number_of_sids);
        self.sid_device.as_mut().unwrap().set_sid_position(self.device_number, 50);

        self.configure_sid_model(number_of_sids);
        self.configure_sid_clock();

        self.sid_device.as_mut().unwrap().set_sampling_method(self.device_number, SamplingMethod::Best);
        if should_reset {
            self.sid_device.as_mut().unwrap().reset_all_sids(self.device_number);
        }

        let has_remote_sidplayer = self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number);
        self.output.lock().has_remote_sidplayer = has_remote_sidplayer;
        Ok(())
    }

    fn init_song(&mut self, song_number: i32) -> Result<(), String> {
        let song_number = if song_number == -1 {
            self.sid_info.lock().default_song
        } else {
            song_number
        };

        let number_of_songs = self.sid_info.lock().number_of_songs;

        if song_number < 0 || song_number >= number_of_songs {
            return Err(format!("Song number {} doesn't exist.", song_number + 1));
        }

        self.redo_buffer.clear();
        self.sid_data_processor.init(0);
        self.sid_device.as_mut().unwrap().set_cycles_in_fifo(self.device_number, 0);
        self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
        self.sid_device.as_mut().unwrap().reset_all_sids(self.device_number);

        self.song_number = song_number;

        self.acid64_lib.set_song_to_play(self.c64_instance, song_number);

        self.acid64_lib.skip_silence(self.c64_instance, true);
        self.acid64_lib.enable_volume_fix(self.c64_instance, true);

        Ok(())
    }

    fn configure_sid_model(&mut self, number_of_sids: i32) {
        let sid_info = self.sid_info.lock();
        for i in 0..number_of_sids {
            let device_number = self.device_numbers.get(i as usize).unwrap_or(&0);
            let sid_model = sid_info.sid_models.get(i as usize).unwrap_or(&0);
            match sid_model {
                2 => self.sid_device.as_mut().unwrap().set_sid_model(*device_number, i, SidModel::Mos8580),
                _ => self.sid_device.as_mut().unwrap().set_sid_model(*device_number, i, SidModel::Mos6581)
            }
        }
    }

    fn configure_sid_clock(&mut self) {
        let c64_model = self.sid_info.lock().clock_frequency;

        match c64_model {
            2 => self.sid_device.as_mut().unwrap().set_sid_clock(self.device_number, SidClock::Ntsc),
            _ => self.sid_device.as_mut().unwrap().set_sid_clock(self.device_number, SidClock::Pal)
        }

        let device_clock = self.sid_device.as_mut().unwrap().get_device_clock(self.device_number);
        self.sid_data_processor.set_sid_clock(device_clock);
    }

    fn get_valid_device_number(&mut self, device_number: i32) -> i32 {
        if device_number == -1 {
            (self.sid_info.lock().sid_models.first().copied().unwrap_or(0) == SID_MODEL_8580) as i32
        } else {
            device_number
        }
    }

    fn fix_device_numbers(&mut self, number_of_sids: i32) -> Result<(), String> {
        let mut device_number = 0;

        for i in 0..number_of_sids {
            device_number = match self.device_numbers.get(i as usize) {
                Some(device_found) => {
                    let device_found = *device_found;
                    let device_number = self.get_valid_device_number(device_found);
                    let _ = std::mem::replace(&mut self.device_numbers[i as usize], device_number);
                    device_number
                }
                None => {
                    self.device_numbers.push(device_number);
                    device_number
                }
            };

        }

        self.device_number = self.get_valid_device_number(self.device_number);
        self.output.lock().device_number = self.device_number;

        self.validate_device_numbers()
    }

    fn validate_device_numbers(&mut self) -> Result<(), String> {
        let device_count = self.sid_device.as_mut().unwrap().get_device_count(self.device_number);

        let mut prev_device = 0;
        for i in 0..self.device_numbers.len() as i32 {
            let device_number = self.device_numbers[i as usize];
            if device_number + 1 > device_count {
                return Err(format!("Device number {} doesn't exist, there are only {} devices.", device_number + 1, device_count));
            }

            if i > 0 && !self.sid_device.as_mut().unwrap().can_pair_devices(prev_device, device_number) {
                return Err(format!("Device number {} can't be used together with device {}. Specify a different second device with option -dX,Y", prev_device + 1, device_number + 1));
            }
            prev_device = device_number;
        }

        Ok(())
    }
}
