// Copyright (C) 2019 - 2021 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod acid64_library;
mod clock_adjust;
mod hardsid_usb;
mod hardsid_usb_device;
mod network_sid_device;
mod sid_device;
mod sid_devices;

use std::sync::atomic::{Ordering, AtomicI32};
use std::sync::{Arc, Mutex};
use std::{thread, time};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use crate::utils::{hvsc, network};
use self::acid64_library::Acid64Library;
use self::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse};
use self::sid_devices::{SidDevices, SidDevicesFacade};

const PAL_CYCLES_PER_SECOND: u32 = 312 * 63 * 50;
const NTSC_CYCLES_PER_SECOND: u32 = 263 * 65 * 60;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT_NUMBER: &str = "6581";

const MIN_CYCLE_SID_WRITE: u32 = 8;

const SID_MODEL_8580: i32 = 2;

const PAUSE_SLEEP_MS: u64 = 10;
const ABORT_DEVICE_DELAY: u64 = 20;

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
    Stop
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
            _ => panic!("Unknown value: {}", value),
        }
    }
}

pub struct Player {
    acid64_lib: Acid64Library,
    c64_instance: usize,
    sid_device: Option<Box<dyn SidDevice + Send>>,
    filename: Option<String>,
    device_number: i32,
    device_numbers: Vec<i32>,
    song_number: i32,
    host_name: String,
    port: String,
    abort_type: Arc<AtomicI32>,
    cmd_sender: SyncSender<PlayerCommand>,
    cmd_receiver: Receiver<PlayerCommand>,
    paused: bool,
    sid_written: bool,
    last_sid_write: [u8; 256],
    device_names: Arc<Mutex<Vec<String>>>,
    adjust_clock: bool
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
            filename: None,
            device_number: 0,
            device_numbers: vec![],
            song_number: 0,
            host_name: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT_NUMBER.to_string(),
            abort_type: Arc::new(AtomicI32::new(ABORT_NO)),
            cmd_sender,
            cmd_receiver,
            paused: false,
            sid_written: false,
            last_sid_write: [0; 256],
            device_names: Arc::new(Mutex::new(Vec::new())),
            adjust_clock: false
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
        self.device_number = *device_numbers.get(0).unwrap_or(&-1);

        self.device_numbers = device_numbers;
    }

    pub fn set_host_name(&mut self, host_name: String) {
        self.host_name = host_name;
    }

    pub fn get_library_version(&self) -> i32 {
        self.acid64_lib.get_version()
    }

    pub fn get_aborted_ref(&self) -> Arc<AtomicI32> {
        Arc::clone(&self.abort_type)
    }

    pub fn play(&mut self) {
        let cycles_per_second = self.get_cycles_per_second();

        let mut delay_cycles: u32 = 0;
        let mut idle_count: u32 = 0;

        self.sid_written = false;
        self.paused = false;
        self.abort_type.store(ABORT_NO, Ordering::SeqCst);

        let mut device_state = DeviceResponse::Ok;

        while !self.should_quit() {
            self.process_player_commands();

            if self.paused {
                thread::sleep(time::Duration::from_millis(PAUSE_SLEEP_MS));
                continue;
            }

            if !self.sid_device.as_mut().unwrap().is_connected(self.device_number) {
                break;
            }

            if device_state == DeviceResponse::Busy {
                device_state = self.sid_device.as_mut().unwrap().retry_write(self.device_number);
                continue;
            }

            self.acid64_lib.run(self.c64_instance);
            let sid_command = SidCommand::from_integer(self.acid64_lib.get_command(self.c64_instance));

            match sid_command {
                SidCommand::Delay => {
                    delay_cycles += self.acid64_lib.get_cycles(self.c64_instance) as u32;
                },
                SidCommand::Write => {
                    device_state = self.process_sid_write(delay_cycles);
                    delay_cycles = 0;
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
        };

        self.abort_type.store(ABORTING, Ordering::SeqCst);

        self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
        thread::sleep(time::Duration::from_millis(ABORT_DEVICE_DELAY));
        self.sid_device.as_mut().unwrap().silent_all_sids(self.device_number, true);

        self.abort_type.store(ABORTED, Ordering::SeqCst);
    }

    #[inline]
    fn process_player_commands(&mut self) {
        let recv_result = self.cmd_receiver.try_recv();

        if let Ok(result) = recv_result {
            self.abort_type.store(ABORT_NO, Ordering::SeqCst);

            match result {
                PlayerCommand::Play => {
                    if self.paused {
                        self.sid_device.as_mut().unwrap().reset_active_sids(self.device_number);
                        self.write_last_sid_writes();
                    }
                    self.paused = false;
                },
                PlayerCommand::Pause => {
                    let device = self.sid_device.as_mut().unwrap();
                    device.reset_all_buffers(self.device_number);
                    device.silent_all_sids(self.device_number, false);
                    self.paused = true;
                },
                _ => ()
            }
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
        let mut device_names = self.device_names.lock().unwrap();
        device_names.clear();
        device_names.extend_from_slice(new_device_names);
    }

    fn get_cycles_per_second(&self) -> u32 {
        let c64_model = self.acid64_lib.get_c64_version(self.c64_instance);

        match c64_model {
            2 => NTSC_CYCLES_PER_SECOND,
            _ => PAL_CYCLES_PER_SECOND
        }
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
            if !network::is_local_ip_address(&self.host_name) {
                return Err(format!("{} is not in the local network or invalid.", self.host_name));
            }

            let mut devices = SidDevices::new(Arc::clone(&self.abort_type));
            devices.connect(&self.host_name, &self.port)?;
            devices.set_native_device_clock(!self.adjust_clock);

            let sid_device = SidDevicesFacade{ devices };
            self.sid_device = Some(Box::new(sid_device));

            self.refresh_device_names();
        }
        Ok(())
    }

    pub fn load_file<S>(&mut self, filename: S) -> Result<(), String> where S: Into<String> {
        let filename = filename.into();
        let is_loaded = self.acid64_lib.load_file(self.c64_instance, &filename);

        if !is_loaded {
            Err(format!("File '{}' could not be loaded.", filename))
        } else {
            self.filename = Some(filename);
            self.acid64_lib.skip_silence(self.c64_instance, true);
            self.acid64_lib.enable_volume_fix(self.c64_instance, true);

            self.init_devices()?;
            self.configure_sid_device(false)?;
            self.set_song_to_play(-1)
        }
    }

    pub fn get_number_of_sids(&self) -> i32 {
        self.acid64_lib.get_number_of_sids(self.c64_instance)
    }

    #[inline]
    fn should_quit(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type == ABORT_TO_QUIT
    }

    #[inline]
    fn process_sid_write(&mut self, delay_cycles: u32) -> DeviceResponse {
        let cycles = delay_cycles + self.acid64_lib.get_cycles(self.c64_instance) as u32;
        let register = self.acid64_lib.get_register(self.c64_instance);
        let data = self.acid64_lib.get_data(self.c64_instance);

        self.last_sid_write[register as usize] = data;
        self.write_to_sid(self.device_number, cycles, register, data)
    }

    #[inline]
    fn write_to_sid(&mut self, device_number: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.sid_device.as_mut().unwrap().try_write(device_number, cycles, reg, data)
    }

    #[inline]
    fn write_last_sid_write(&mut self, reg: u8) {
        self.write_to_sid(self.device_number, MIN_CYCLE_SID_WRITE, reg, self.last_sid_write[reg as usize]);
    }

    fn write_last_sid_writes(&mut self) {
        let number_of_sids = self.acid64_lib.get_number_of_sids(self.c64_instance);

        for sid_number in 1..=number_of_sids {
            self.write_voice_regs(1, sid_number);
            self.write_voice_regs(2, sid_number);
            self.write_voice_regs(3, sid_number);

            self.write_filter_and_volume_regs(sid_number);
        }
    }

    #[inline]
    fn write_voice_regs(&mut self, voice_number: i32, sid_number: i32) {
        let reg_base: u8 = ((voice_number - 1) * 7) as u8;
        let sid_base: u8 = ((sid_number - 1) * 3) as u8;

        self.write_last_sid_write(sid_base + reg_base + 0x03);
        self.write_last_sid_write(sid_base + reg_base + 0x02);
        self.write_last_sid_write(sid_base + reg_base + 0x01);
        self.write_last_sid_write(sid_base + reg_base);
        self.write_last_sid_write(sid_base + reg_base + 0x06);
        self.write_last_sid_write(sid_base + reg_base + 0x05);
        self.write_last_sid_write(sid_base + reg_base + 0x04);
    }

    #[inline]
    fn write_filter_and_volume_regs(&mut self, sid_number: i32) {
        let sid_base: u8 = ((sid_number - 1) * 3) as u8;

        self.write_last_sid_write(sid_base + 0x15);
        self.write_last_sid_write(sid_base + 0x16);
        self.write_last_sid_write(sid_base + 0x17);
        self.write_last_sid_write(sid_base + 0x18);
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

        self.sid_device.as_mut().unwrap().reset_all_buffers(self.device_number);
        self.sid_device.as_mut().unwrap().reset_all_sids(self.device_number);

        self.song_number = song_number;

        self.acid64_lib.set_song_to_play(self.c64_instance, song_number);
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
    }

    fn get_valid_device_number(&mut self, device_number: i32) -> i32 {
        if device_number == -1 {
            let sid_model = self.acid64_lib.get_sid_model(self.c64_instance, 0);

            if sid_model == SID_MODEL_8580 {
                1
            } else {
                0
            }
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
                    let device_number = self.get_valid_device_number(device_found as i32);
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
