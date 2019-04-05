// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

mod acid64_library;
mod network_sid_device;

use crate::utils::{hvsc, network};

use self::acid64_library::Acid64Library;
use self::network_sid_device::{NetworkSidDevice, SidClock, SamplingMethod};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{thread, time};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

const PAL_CYCLES_PER_SECOND: u32 = 312 * 63 * 50;
const NTSC_CYCLES_PER_SECOND: u32 = 263 * 65 * 60;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT_NUMBER: &str = "6581";

const MIN_CYCLE_SID_WRITE: u32 = 32;

const SID_MODEL_8580: i32 = 2;

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
    InitDone
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
            _ => panic!("Unknown value: {}", value),
        }
    }
}

pub struct Player {
    acid64_lib: Acid64Library,
    c64_instance: usize,
    network_sid_device: Option<NetworkSidDevice>,
    filename: String,
    device_number: i32,
    song_number: i32,
    host_name: String,
    port: String,
    aborted: Arc<AtomicBool>,
    cmd_sender: SyncSender<PlayerCommand>,
    cmd_receiver: Receiver<PlayerCommand>,
    paused: bool,
    last_sid_write: [u8; 256]
}

impl Drop for Player {
    fn drop(&mut self) {
        if self.c64_instance > 0 {
            self.acid64_lib.close_c64_instance(self.c64_instance);
        }
    }
}

impl Player {
    pub fn new(filename: String) -> Player {
        let (cmd_sender, cmd_receiver) = sync_channel(0);

        let mut player_properties = Player {
            acid64_lib: Acid64Library::new(),
            c64_instance: 0,
            network_sid_device: None,
            filename,
            device_number: 0,
            song_number: 0,
            host_name: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT_NUMBER.to_string(),
            aborted: Arc::new(AtomicBool::new(false)),
            cmd_sender,
            cmd_receiver,
            paused: false,
            last_sid_write: [0; 256]
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

    pub fn init(&mut self) -> Result<(), String> {
        self.init_devices()?;
        self.load_file(self.c64_instance, self.filename.to_owned())?;
        Ok(())
    }

    pub fn get_channel_sender(&self) -> SyncSender<PlayerCommand> {
        SyncSender::clone(&self.cmd_sender)
    }

    pub fn set_device_number(&mut self, device_number: i32) {
        self.device_number = device_number;
    }

    pub fn set_song_number(&mut self, song_number: i32) {
        self.song_number = song_number;
    }

    pub fn set_host_name(&mut self, host_name: String) {
        self.host_name = host_name;
    }

    pub fn get_library_version(&self) -> i32 {
        self.acid64_lib.get_version()
    }

    pub fn get_aborted_ref(&mut self) -> Arc<AtomicBool> {
        Arc::clone(&self.aborted)
    }

    pub fn play(&mut self) {
        let cycles_per_second = self.get_cycles_per_second();

        let mut delay_cycles: u32 = 0;
        let mut idle_count: u32 = 0;

        self.paused = false;
        self.aborted.store(false, Ordering::SeqCst);

        while !self.is_aborted() {
            self.process_player_commands();

            if self.paused {
                thread::sleep(time::Duration::from_millis(20));
                continue;
            }

            self.acid64_lib.run(self.c64_instance);
            let sid_command = SidCommand::from_integer(self.acid64_lib.get_command(self.c64_instance));

            match sid_command {
                SidCommand::Delay => {
                    delay_cycles += self.acid64_lib.get_cycles(self.c64_instance) as u32;
                },
                SidCommand::Write => {
                    let _ = self.process_sid_write(delay_cycles);
                    delay_cycles = 0;
                    idle_count = 0;
                },
                SidCommand::Read => {
                    idle_count = 0;
                },
                SidCommand::Idle => {
                    idle_count += cycles_per_second / 1000;

                    if idle_count >= cycles_per_second {
                        self.network_sid_device.as_mut().unwrap().dummy_write(0, cycles_per_second);
                        idle_count -= cycles_per_second
                    }
                },
                _ => (),
            }
        };

        self.network_sid_device.as_mut().unwrap().flush_buffers(0);
    }

    fn process_player_commands(&mut self) {
        let recv_result = self.cmd_receiver.try_recv();
        if recv_result.is_ok() {
            match recv_result.unwrap() {
                PlayerCommand::Play => {
                    if self.paused {
                        self.write_last_sid_writes();
                    }
                    self.paused = false;
                },
                PlayerCommand::Pause => {
                    self.network_sid_device.as_mut().unwrap().flush_buffers(0);
                    self.paused = true;
                },
                _ => ()
            }
        }
    }

    pub fn get_device_names(&mut self) -> Vec<String> {
        let mut vec = Vec::new();

        let device_count = self.network_sid_device.as_mut().unwrap().get_device_count();

        for i in 0..device_count {
            let device_info = self.network_sid_device.as_mut().unwrap().get_device_info(i);
            vec.push(device_info);
        }
        vec
    }

    pub fn get_cycles_per_second(&mut self) -> u32 {
        let c64_model = self.acid64_lib.get_c64_version(self.c64_instance);
        match c64_model {
            2 => NTSC_CYCLES_PER_SECOND,
            _ => PAL_CYCLES_PER_SECOND
        }
    }

    pub fn get_song_length(&mut self) -> i32 {
        self.acid64_lib.get_song_length(self.c64_instance)
    }

    pub fn get_filename(&self) -> String {
        self.filename.clone()
    }

    pub fn get_sid_model(&mut self) -> i32 {
        self.acid64_lib.get_sid_model(self.c64_instance)
    }

    pub fn get_c64_version(&mut self) -> i32 {
        self.acid64_lib.get_c64_version(self.c64_instance)
    }

    pub fn get_title(&mut self) -> String {
        self.acid64_lib.get_title(self.c64_instance)
    }

    pub fn get_author(&mut self) -> String {
        self.acid64_lib.get_author(self.c64_instance)
    }

    pub fn get_released(&mut self) -> String {
        self.acid64_lib.get_released(self.c64_instance)
    }

    pub fn get_stil_entry(&mut self) -> Option<String> {
        self.acid64_lib.get_stil_entry(self.c64_instance)
    }

    pub fn get_device_number(&self) -> i32 {
        self.device_number
    }

    pub fn get_song_number(&self) -> i32 {
        self.song_number
    }

    pub fn get_number_of_songs(&mut self) -> i32 {
        self.acid64_lib.get_number_of_songs(self.c64_instance)
    }

    pub fn get_device_info(&mut self, device_number: i32) -> String {
        self.network_sid_device.as_mut().unwrap().get_device_info(device_number)
    }

    pub fn setup_sldb_and_stil(&mut self, hvsc_location: Option<String>, load_stil: bool) -> Result<(), String> {
        let mut hvsc_root = self.get_hvsc_root_location(hvsc_location)?;

        if hvsc_root.is_none() {
            hvsc_root = hvsc::get_hvsc_root(self.filename.as_ref());
        }

        if hvsc_root.is_some() {
            self.load_sldb(hvsc_root.as_ref().unwrap())?;

            if load_stil {
                self.acid64_lib.load_stil(hvsc_root.as_ref().unwrap().to_owned());
            }
        }
        Ok(())
    }

    fn init_devices(&mut self) -> Result<(), String> {
        if self.network_sid_device.is_none() {
            let host_name = self.host_name.to_owned();

            let is_local_ip = network::is_local_ip_address(host_name.to_owned());

            if !is_local_ip {
                return Err(format!("{} is not in the local network or invalid.", host_name));
            }

            self.network_sid_device = Some(NetworkSidDevice::new(host_name, self.port.to_owned(), Arc::clone(&self.aborted)));
        }
        Ok(())
    }

    #[inline]
    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    #[inline]
    fn process_sid_write(&mut self, delay_cycles: u32) -> u32 {
        let cycles = delay_cycles + self.acid64_lib.get_cycles(self.c64_instance) as u32;
        let register = self.acid64_lib.get_register(self.c64_instance);
        let data = self.acid64_lib.get_data(self.c64_instance);

        self.write_to_sid(0, cycles, register, data);

        self.last_sid_write[register as usize] = data;
        cycles
    }

    #[inline]
    fn write_to_sid(&mut self, device_number: i32, cycles: u32, reg: u8, data: u8) {
        self.network_sid_device.as_mut().unwrap().write(device_number, cycles, reg, data);
    }

    #[inline]
    fn write_last_sid_write(&mut self, reg: u8) {
        self.write_to_sid(0, MIN_CYCLE_SID_WRITE, reg, self.last_sid_write[reg as usize]);
    }

    fn write_last_sid_writes(&mut self) {
        let number_of_sids = self.acid64_lib.get_number_of_sids(self.c64_instance);

        for sid_number in 1..=number_of_sids  {
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
        self.write_last_sid_write(sid_base + reg_base + 0x00);
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

    fn get_hvsc_root_location(&mut self, hvsc_location: Option<String>) -> Result<Option<String>, String> {
        if hvsc_location.is_some() {
            let hvsc_root = hvsc::get_hvsc_root(hvsc_location.as_ref().unwrap());

            if hvsc_root.is_none() {
                return Err("Specified HVSC location is not valid.".to_string());
            }
            return Ok(hvsc_root);
        }
        Ok(None)
    }

    fn load_sldb(&mut self, hvsc_root: &str) -> Result<(), String> {
        let is_sldb = self.acid64_lib.check_sldb(hvsc_root.to_string());
        if !is_sldb {
            return Err("Song length database is not found or not a database.".to_string());
        }

        let is_sldb_loaded = self.acid64_lib.load_sldb(hvsc_root.to_string());
        if !is_sldb_loaded {
            return Err("Song length database could not be loaded.".to_string());
        }
        Ok(())
    }

    fn load_file(&mut self, c64_instance: usize, filename: String) -> Result<(), String> {
        let is_loaded = self.acid64_lib.load_file(c64_instance, filename.to_owned());
        if !is_loaded {
            Err(format!("File '{}' could not be loaded.", filename).to_string())
        } else {
            self.filename = filename;
            self.configure_sid_device(self.c64_instance)?;
            Ok(())
        }
    }

    fn configure_sid_device(&mut self, c64_instance: usize) -> Result<(), String> {
        self.acid64_lib.skip_silence(c64_instance, true);
        self.acid64_lib.enable_volume_fix(c64_instance, true);

        let number_of_sids = self.acid64_lib.get_number_of_sids(c64_instance);
        self.network_sid_device.as_mut().unwrap().set_sid_count(number_of_sids);
        self.network_sid_device.as_mut().unwrap().set_sid_position(50);
        self.configure_sid_model(number_of_sids)?;
        self.configure_sid_clock(c64_instance);
        self.network_sid_device.as_mut().unwrap().set_sampling_method(SamplingMethod::BEST);

        self.set_song_to_play(self.song_number)?;

        self.network_sid_device.as_mut().unwrap().reset_sid(0);
        Ok(())
    }

    pub fn get_next_song(&mut self) -> i32 {
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
            return Err(format!("Song number {} doesn't exist.", song_number).to_string());
        }

        self.song_number = song_number;

        self.acid64_lib.set_song_to_play(self.c64_instance, song_number);
        Ok(())
    }

    pub fn configure_sid_model(&mut self, number_of_sids: i32) -> Result<(), String> {
        if self.device_number == -1 {
            let sid_model = self.acid64_lib.get_sid_model(self.c64_instance);

            if sid_model == SID_MODEL_8580 {
                self.device_number = 1;
            } else {
                self.device_number = 0;
            }
        }

        let device_count = self.network_sid_device.as_mut().unwrap().get_device_count();

        if self.device_number + 1 > device_count {
            return Err(format!("Device number {} doesn't exist, there are only {} devices.", self.device_number + 1, device_count));
        }

        for i in 0..number_of_sids {
            self.network_sid_device.as_mut().unwrap().set_sid_model(i, self.device_number);
        }
        Ok(())
    }

    pub fn configure_sid_clock(&mut self, c64_instance: usize) {
        let c64_model = self.acid64_lib.get_c64_version(c64_instance);
        match c64_model {
            2 => self.network_sid_device.as_mut().unwrap().set_sid_clock(SidClock::NTSC),
            _ => self.network_sid_device.as_mut().unwrap().set_sid_clock(SidClock::PAL)
        }
    }
}