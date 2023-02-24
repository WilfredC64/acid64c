// Copyright (C) 2019 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod acid64_library;
mod clock_adjust;
mod hardsid_usb;
mod hardsid_usb_device;
mod network_sid_device;
mod sid_data_processor;
mod sid_device;
mod sid_devices;
mod ultimate_device;

use parking_lot::Mutex;
use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind, Read};
use std::sync::atomic::{Ordering, AtomicI32};
use std::sync::Arc;
use std::{thread, time};
use std::collections::VecDeque;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use crate::utils::hvsc;
use self::acid64_library::Acid64Library;
use self::sid_data_processor::{SidDataProcessor, SidWrite};
use self::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse, DUMMY_REG};
use self::sid_devices::{SidDevices, SidDevicesFacade};

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

#[derive(Copy, Clone)]
pub struct PlayerOutput {
    pub time: u32,
}

pub struct Player {
    acid64_lib: Acid64Library,
    c64_instance: usize,
    sid_device: Option<Box<dyn SidDevice + Send>>,
    sid_data_processor: SidDataProcessor,
    filename: Option<String>,
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
}

impl Drop for Player {
    fn drop(&mut self) {
        if self.c64_instance > 0 {
            self.acid64_lib.close_c64_instance(self.c64_instance);
        }
    }
}

impl Player
{
    pub fn new() -> Player {
        let (cmd_sender, cmd_receiver) = sync_channel(0);

        let mut player_properties = Player {
            acid64_lib: Acid64Library::load().expect("acid64pro library could not be loaded"),
            c64_instance: 0,
            sid_device: None,
            sid_data_processor: SidDataProcessor::new(),
            filename: None,
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
            output: Arc::new(Mutex::new(PlayerOutput { time: 0 })),
        };

        player_properties.setup_c64_instance();
        player_properties
    }

    fn setup_c64_instance(&mut self) {
        self.c64_instance = self.acid64_lib.create_c64_instance();

        if self.c64_instance == 0 {
            panic!("C64 instance couldn't be created.");
        }
    }

    pub fn get_channel_sender(&self) -> SyncSender<PlayerCommand> {
        SyncSender::clone(&self.cmd_sender)
    }

    pub fn set_device_numbers(&mut self, device_numbers: Vec<i32>) {
        self.device_number = *device_numbers.first().unwrap_or(&-1);

        self.device_numbers = device_numbers;
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

    pub fn play(&mut self) {
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

    pub fn stop_player(&mut self) {
        if self.sid_device.is_some() && self.device_number != -1 && !self.paused && self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number) {
            if self.sid_device.as_mut().unwrap().is_connected(self.device_number) {
                self.sid_device.as_mut().unwrap().stop_sid(self.device_number);
            } else {
                self.abort_type.store(ABORT_TO_QUIT, Ordering::SeqCst);
            }
        }
    }

    fn send_sid(&mut self, filename: &str, song_number: i32) {
        let sid_data = if filename.ends_with(".mus") || filename.ends_with(".str") {
            Self::read_mus_files(filename)
        } else {
            Self::read_file(filename)
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

    fn read_mus_files(filename: &str) -> Result<Vec<u8>, Error> {
        if filename.ends_with(".mus") {
            if let Ok(data_mus) = Self::read_file(filename) {
                let str_filename = filename.strip_suffix(".mus").unwrap().to_string() + ".str";
                if let Ok(data_str) = Self::read_file(&str_filename) {
                    Ok([data_mus, data_str].concat())
                } else {
                    Ok(data_mus)
                }
            } else {
                Err(Error::new(ErrorKind::Other, "Error loading mus file"))
            }
        } else if filename.ends_with(".str") {
            if let Ok(data_str) = Self::read_file(filename) {
                let mus_filename = filename.strip_suffix(".str").unwrap().to_string() + ".mus";
                if let Ok(data_mus) = Self::read_file(&mus_filename) {
                    Ok([data_mus, data_str].concat())
                } else {
                    Err(Error::new(ErrorKind::Other, "Error loading mus file"))
                }
            } else {
                Err(Error::new(ErrorKind::Other, "Error loading str file"))
            }
        } else {
            Self::read_file(filename)
        }
    }

    fn generate_ssl_data(&mut self) -> Vec<u8>{
        let mut song_lengths_in_millis = vec![];
        for song_number in 0..self.get_number_of_songs() {
            self.acid64_lib.set_song_to_play(self.c64_instance, song_number);
            song_lengths_in_millis.push(self.acid64_lib.get_song_length(self.c64_instance));
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
        while value > 0 {
            result |= (value % 10) << shift;
            value /= 10;
            shift += 4;
        }
        result
    }

    fn read_file(filename: &str) -> io::Result<Vec<u8>> {
        let mut data = vec![];
        File::open(filename)?.read_to_end(&mut data)?;
        Ok(data)
    }

    pub fn restart_song(&mut self) -> Result<(), String> {
        self.set_song_to_play(self.song_number)
    }

    pub fn update_player_output(&mut self) {
        self.sid_data_processor.process_sid_write_fifo();

        let mut output = self.output.lock();
        output.time = self.sid_data_processor.get_time_in_millis();
    }

    fn is_aborted_for_command(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type == ABORT_FOR_COMMAND
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

    pub fn get_device_names(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.device_names)
    }

    pub fn get_last_error(&mut self) -> Option<String> {
        self.sid_device.as_mut().unwrap().get_last_error(self.device_number)
    }

    fn refresh_device_names(&mut self) {
        let mut device_names = Vec::new();

        let device_count = self.sid_device.as_mut().unwrap().get_device_count(self.device_number);
        for i in 0..device_count {
            let device_name = self.sid_device.as_mut().unwrap().get_device_info(i);
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

    pub fn get_player_output(&mut self) -> Arc<Mutex<PlayerOutput>> {
        Arc::clone(&self.output)
    }

    pub fn get_song_length(&self) -> i32 {
        self.acid64_lib.get_song_length(self.c64_instance)
    }

    pub fn get_filename(&self) -> Option<String> {
        self.filename.clone()
    }

    pub fn get_sid_model(&self) -> i32 {
        self.acid64_lib.get_sid_model(self.c64_instance, 0)
    }

    pub fn get_c64_version(&self) -> i32 {
        self.acid64_lib.get_c64_version(self.c64_instance)
    }

    pub fn get_title(&self) -> String {
        self.acid64_lib.get_title(self.c64_instance)
    }

    pub fn get_author(&self) -> String {
        self.acid64_lib.get_author(self.c64_instance)
    }

    pub fn get_released(&self) -> String {
        self.acid64_lib.get_released(self.c64_instance)
    }

    pub fn get_stil_entry(&self) -> Option<String> {
        self.acid64_lib.get_stil_entry(self.c64_instance)
    }

    pub fn get_device_numbers(&self) -> Vec<i32> {
        self.device_numbers.clone()
    }

    pub fn get_song_number(&self) -> i32 {
        self.song_number
    }

    pub fn get_number_of_songs(&self) -> i32 {
        self.acid64_lib.get_number_of_songs(self.c64_instance)
    }

    pub fn get_device_info(&mut self, device_number: i32) -> String {
        self.sid_device.as_mut().unwrap().get_device_info(device_number)
    }

    pub fn has_remote_sidplayer(&mut self) -> bool {
        self.sid_device.as_mut().unwrap().has_remote_sidplayer(self.device_number)
    }

    pub fn setup_sldb_and_stil(&mut self, hvsc_location: Option<String>, load_stil: bool) -> Result<(), String> {
        let mut hvsc_root = self.get_hvsc_root_location(hvsc_location)?;

        if hvsc_root.is_none() {
            if let Some(filename) = &self.filename {
                hvsc_root = hvsc::get_hvsc_root(filename);
            }
        }

        if let Some(hvsc_root) = hvsc_root {
            self.load_sldb(&hvsc_root)?;

            if load_stil {
                self.acid64_lib.load_stil(&hvsc_root);
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
        Ok(())
    }

    pub fn load_file(&mut self, filename: &str) -> Result<(), String> {
        let is_loaded = self.acid64_lib.load_file(self.c64_instance, filename);

        if !is_loaded {
            Err(format!("File '{filename}' could not be loaded."))
        } else {
            self.filename = Some(filename.to_string());

            self.init_devices()?;
            self.configure_sid_device(false)?;
            self.set_song_to_play(-1)
        }
    }

    pub fn get_number_of_sids(&self) -> i32 {
        self.acid64_lib.get_number_of_sids(self.c64_instance)
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

        for sid_write in buffer.iter().rev() {
            self.redo_buffer.push_front(SidWrite::new(sid_write.reg, sid_write.data, sid_write.cycles, sid_write.cycles_real));
        }

        self.sid_data_processor.clear_buffer();

        self.sid_device.as_mut().unwrap().enable_turbo_mode(self.device_number);
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

    fn load_sldb(&mut self, hvsc_root: &str) -> Result<(), String> {
        let is_sldb = self.acid64_lib.check_sldb(hvsc_root);

        if !is_sldb {
            return Err("Song length database is not found or not a database.".to_string());
        }

        let is_sldb_loaded = self.acid64_lib.load_sldb(hvsc_root);

        if !is_sldb_loaded {
            return Err("Song length database could not be loaded.".to_string());
        }
        Ok(())
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
        Ok(())
    }

    pub fn get_next_song(&self) -> i32 {
        let number_of_songs = self.get_number_of_songs();

        if self.song_number == number_of_songs - 1 {
            0
        } else {
            self.song_number + 1
        }
    }

    pub fn get_prev_song(&mut self) -> i32 {
        if self.song_number == 0 {
            self.get_number_of_songs() - 1
        } else {
            self.song_number - 1
        }
    }

    pub fn set_song_to_play(&mut self, song_number: i32) -> Result<(), String> {
        let song_number = if song_number == -1 {
            self.acid64_lib.get_default_song(self.c64_instance)
        } else {
            song_number
        };

        let number_of_songs = self.acid64_lib.get_number_of_songs(self.c64_instance);

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

    pub fn configure_sid_model(&mut self, number_of_sids: i32) {
        for i in 0..number_of_sids {
            let device_number = self.device_numbers.get(i as usize).unwrap_or(&0);
            self.sid_device.as_mut().unwrap().set_sid_model(*device_number, i);
        }
    }

    pub fn configure_sid_clock(&mut self) {
        let c64_model = self.acid64_lib.get_c64_version(self.c64_instance);

        match c64_model {
            2 => self.sid_device.as_mut().unwrap().set_sid_clock(self.device_number, SidClock::Ntsc),
            _ => self.sid_device.as_mut().unwrap().set_sid_clock(self.device_number, SidClock::Pal)
        }

        let device_clock = self.sid_device.as_mut().unwrap().get_device_clock(self.device_number);
        self.sid_data_processor.set_sid_clock(device_clock);
    }

    fn get_valid_device_number(&mut self, device_number: i32) -> i32 {
        if device_number == -1 {
            i32::from(self.acid64_lib.get_sid_model(self.c64_instance, 0) == SID_MODEL_8580)
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
