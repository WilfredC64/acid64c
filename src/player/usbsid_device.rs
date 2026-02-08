// Copyright (C) 2025 - 2026 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::sid_device::{DeviceId, DeviceInfo, DeviceResponse, SamplingMethod, SidClock, SidDevice, SidModel, SidWrite};
use super::{ABORTING, ABORTED, MIN_CYCLE_SID_WRITE};

use std::sync::atomic::{Ordering, AtomicI32, AtomicU32, AtomicBool};
use std::{sync::Arc};
use std::collections::VecDeque;
use std::time::Duration;
use ringbuf::{CachingProd, HeapRb, SharedRb};
use ringbuf::producer::Producer;
use ringbuf::storage::Heap;
use ringbuf::traits::Split;
use crate::player::usbsid_scheduler::{UsbSidCommand, UsbSidScheduler, USBSID_DEVICE_NAME};
use crossbeam_channel::{Sender, Receiver, bounded};

const ERROR_MSG_DEVICE_COUNT_CHANGED: &str = "Number of devices is changed.";
const ERROR_MSG_DEVICE_FAILURE: &str = "Failure occurred during interaction with device.";
const ERROR_MSG_NO_USBSID_FOUND: &str = "No USBSID device found.";

pub const MAX_CYCLES_IN_BUFFER: u32 = 63*312*5; // ~100ms of PAL C64 time
pub const SID_WRITES_BUFFER_SIZE: usize = 2*1024;

const MAX_CYCLES_PER_WRITE: u32 = 1000;
const CMD_TIMEOUT_IN_MILLIS: u64 = 500;

const DUMMY_REG: u8 = 0x1e;

pub struct UsbsidDeviceFacade {
    pub usbsid_device: UsbsidDevice
}

impl SidDevice for UsbsidDeviceFacade {
    fn get_device_id(&mut self, _dev_nr: i32) -> DeviceId { DeviceId::Usbsid }

    fn disconnect(&mut self, _dev_nr: i32) {
        self.usbsid_device.disconnect();
    }

    fn is_connected(&mut self, _dev_nr: i32) -> bool {
        self.usbsid_device.is_connected()
    }

    fn get_last_error(&mut self, _dev_nr: i32) -> Option<String> {
        self.usbsid_device.get_last_error()
    }

    fn test_connection(&mut self, dev_nr: i32) {
        self.usbsid_device.test_connection(dev_nr);
    }

    fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        self.usbsid_device.can_pair_devices(dev1, dev2)
    }

    fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.usbsid_device.get_device_count()
    }

    fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo {
        self.usbsid_device.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, _dev_nr: i32, sid_count: i32) {
        self.usbsid_device.set_sid_count(sid_count);
    }

    fn set_sid_position(&mut self, _dev_nr: i32, _sid_position: i8) {
        // not supported
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32, sid_model: SidModel) {
        self.usbsid_device.set_sid_model(dev_nr, sid_socket, sid_model);
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.usbsid_device.set_sid_clock(sid_clock);
    }

    fn set_sampling_method(&mut self, _dev_nr: i32, _sampling_method: SamplingMethod) {
        // not supported
    }

    fn set_sid_header(&mut self, _dev_nr: i32, _sid_header: Vec<u8>) {
        // not supported
    }

    fn set_fade_in(&mut self, _dev_nr: i32, _time_millis: u32) {
        // not supported
    }

    fn set_fade_out(&mut self, _dev_nr: i32, _time_millis: u32) {
        // not supported
    }

    fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool) {
        self.usbsid_device.silent_all_sids(dev_nr, write_volume);
    }

    fn silent_active_sids(&mut self, dev_nr: i32, write_volume: bool) {
        self.usbsid_device.silent_active_sids(dev_nr, write_volume);
    }

    fn reset_all_sids(&mut self, dev_nr: i32) {
        self.usbsid_device.reset_all_sids(dev_nr);
    }

    fn reset_active_sids(&mut self, dev_nr: i32) {
        self.usbsid_device.reset_active_sids(dev_nr);
    }

    fn reset_all_buffers(&mut self, dev_nr: i32) {
        self.usbsid_device.reset_all_buffers(dev_nr);
    }

    fn enable_turbo_mode(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn disable_turbo_mode(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        self.usbsid_device.dummy_write(dev_nr, cycles);
    }

    fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.usbsid_device.write(dev_nr, cycles, reg, data)
    }

    fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.usbsid_device.try_write(dev_nr, cycles, reg, data)
    }

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        self.usbsid_device.retry_write(dev_nr)
    }

    fn force_flush(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn set_native_device_clock(&mut self, _enabled: bool) {
        // not supported
    }

    fn get_device_clock(&mut self, _dev_nr: i32) -> SidClock {
        self.usbsid_device.get_device_clock()
    }

    fn has_remote_sidplayer(&mut self, _dev_nr: i32) -> bool {
        false
    }

    fn send_sid(&mut self, _dev_nr: i32, _filename: &str, _song_number: i32, _sid_data: &[u8], _ssl_data: &[u8]) {
        // not supported
    }

    fn stop_sid(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn set_cycles_in_fifo(&mut self, _dev_nr: i32, _cycles: u32) {
        // not supported
    }
}

pub struct UsbsidDevice {
    queue: CachingProd<Arc<SharedRb<Heap<SidWrite>>>>,
    temp_queue: VecDeque<SidWrite>,
    device_names: Vec<DeviceInfo>,
    device_count: i32,
    sid_count: i32,
    number_of_sids: i32,
    sid_clock: SidClock,
    device_id: Vec<u8>,
    device_base_reg: Vec<u8>,
    device_index: Vec<u8>,
    abort_type: Arc<AtomicI32>,
    last_error: Option<String>,
    device_mappings: Vec<i32>,
    device_socket_count: Vec<i32>,
    device_init_done: Vec<bool>,
    usbsid_scheduler: UsbSidScheduler,
    in_cmd_sender: Sender<(UsbSidCommand, i32)>,
    in_cmd_receiver: Receiver<(UsbSidCommand, i32)>,
    active_device_index: i32,
    usbsid_aborted: Arc<AtomicBool>,
    cycles_in_buffer: Arc<AtomicU32>,
}

impl UsbsidDevice {
    pub fn new(abort_type: Arc<AtomicI32>) -> UsbsidDevice {
        let usbsid_aborted = Arc::new(AtomicBool::new(false));

        let cycles_in_buffer = Arc::new(AtomicU32::new(0));
        let rb = HeapRb::<SidWrite>::new(SID_WRITES_BUFFER_SIZE);
        let (prod, cons) = rb.split();

        let usbsid_scheduler = UsbSidScheduler::new(
            Some(cons),
            usbsid_aborted.clone(),
            cycles_in_buffer.clone()
        );

        let (in_cmd_sender, in_cmd_receiver) = bounded(0);

        UsbsidDevice {
            queue: prod,
            temp_queue: VecDeque::new(),
            device_names: vec![],
            device_count: 0,
            sid_count: 0,
            number_of_sids: 0,
            sid_clock: SidClock::Pal,
            device_id: vec![],
            device_base_reg: vec![],
            device_index: vec![],
            abort_type,
            last_error: None,
            device_mappings: vec![],
            device_socket_count: vec![],
            device_init_done: vec![],
            usbsid_scheduler,
            in_cmd_sender,
            in_cmd_receiver,
            active_device_index: 0,
            usbsid_aborted,
            cycles_in_buffer,
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.disconnect();
        self.last_error = None;

        let usbsid_config = self.usbsid_scheduler.start(Receiver::clone(&self.in_cmd_receiver)).unwrap_or_default();

        let device_names = usbsid_config.devices.clone();
        self.device_count = device_names.len() as i32;

        if self.device_count > 0 {
            let mut dev_config_count = 0;

            for i in 0..self.device_count {
                let socket_count = device_names[i as usize].socket_count;
                for j in 0..socket_count {
                    let device_name = format!("{}-{}", USBSID_DEVICE_NAME, dev_config_count + 1);
                    self.device_names.push(DeviceInfo {
                        id: device_name.clone(),
                        name: device_name,
                        socket_count: 1
                    });
                    self.device_index.push(dev_config_count);
                    self.device_base_reg.push((j * 0x20) as u8);
                    self.device_mappings.push(i);
                    self.device_socket_count.push(socket_count);
                    dev_config_count += 1;
                }
            }
            self.sid_count = self.device_index.len() as i32;
        } else {
            return Err(ERROR_MSG_NO_USBSID_FOUND.to_string())
        }

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.init_device_settings();
    }

    pub fn disconnect_with_error(&mut self, error_message: String) {
        self.last_error = Some(error_message);
        self.disconnect();
    }

    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.device_count > 0 && !self.is_usbsid_aborted()
    }

    pub fn test_connection(&mut self, dev_nr: i32) {
        if self.is_connected() {
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, DUMMY_REG, 0);
        } else {
            self.disconnect_with_error(ERROR_MSG_DEVICE_COUNT_CHANGED.to_string());
        }
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        dev1 != dev2 && self.device_mappings[dev1 as usize] == self.device_mappings[dev2 as usize]
    }

    pub fn get_device_count(&self) -> i32 {
        self.sid_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo {
        self.device_names[dev_nr as usize].clone()
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;
    }

    pub fn set_sid_model(&mut self, _dev_nr: i32, _sid_socket: i32, sid_model: SidModel) {
        self.send_command(UsbSidCommand::SetModel, sid_model as i32);
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
        self.send_command(UsbSidCommand::SetClock, sid_clock as i32);
    }

    pub fn silent_all_sids(&mut self, dev_nr: i32, _write_volume: bool) {
        self.send_command(UsbSidCommand::MuteAll, dev_nr);
    }

    pub fn silent_active_sids(&mut self, dev_nr: i32, _write_volume: bool) {
        self.send_command(UsbSidCommand::MuteAll, dev_nr);
    }

    pub fn reset_all_sids(&mut self, dev_nr: i32) {
        self.send_command(UsbSidCommand::ResetAll, dev_nr);
    }

    pub fn reset_active_sids(&mut self, dev_nr: i32) {
        if self.is_connected() {
            for sid_nr in 0..self.number_of_sids as u8 {
                let base_reg = self.map_device_to_reg(dev_nr, sid_nr * 0x20);
                self.send_command(UsbSidCommand::Reset, base_reg as i32);
            }
        }
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        self.send_command(UsbSidCommand::ClearBuffer, dev_nr);
        self.temp_queue.clear();
    }

    pub fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        if self.is_connected() {
            let base_reg = self.device_base_reg[dev_nr as usize];
            self.try_write(dev_nr, cycles, base_reg + DUMMY_REG, 0);
        }
    }

    pub fn get_device_clock(&self) -> SidClock {
        self.sid_clock
    }

    pub fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.try_write(dev_nr, cycles, reg, data)
    }

    pub fn retry_write(&mut self, _dev_nr: i32) -> DeviceResponse {
        self.write_temp_queue()
    }

    pub fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        if self.is_player_aborted() {
            self.usbsid_aborted.store(true, Ordering::SeqCst);
            self.disconnect();
            return DeviceResponse::Ok
        }

        if !self.is_connected() {
            self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
            return DeviceResponse::Error
        }

        let new_dev_index = self.device_mappings[dev_nr as usize];
        if new_dev_index != self.active_device_index {
            self.send_command(UsbSidCommand::SetDevice, new_dev_index);
            self.active_device_index = new_dev_index;
        }

        let mut cycles = cycles;
        while cycles > MAX_CYCLES_PER_WRITE {
            cycles -= MAX_CYCLES_PER_WRITE - MIN_CYCLE_SID_WRITE;

            self.temp_queue.push_back(SidWrite {
                reg: DUMMY_REG,
                data: 0x00,
                cycles: (MAX_CYCLES_PER_WRITE - MIN_CYCLE_SID_WRITE) as u16
            });
        }

        let reg = self.map_device_to_reg(dev_nr, reg);
        self.temp_queue.push_back(SidWrite { reg, data, cycles: cycles as u16 } );

        if self.cycles_in_buffer.load(Ordering::Relaxed) >= MAX_CYCLES_IN_BUFFER {
            return DeviceResponse::Busy
        }

        self.write_temp_queue()
    }

    fn write_temp_queue(&mut self) -> DeviceResponse {
        if self.temp_queue.is_empty() {
            return DeviceResponse::Ok;
        }

        let slice = self.temp_queue.make_contiguous();
        let pushed_count = self.queue.push_slice(slice);

        if pushed_count > 0 {
            let cycles_added: u32 = slice[..pushed_count]
                .iter()
                .map(|w| w.cycles as u32)
                .sum();

            self.cycles_in_buffer.fetch_add(cycles_added, Ordering::Relaxed);
            self.temp_queue.drain(..pushed_count);
        }

        if self.temp_queue.is_empty() {
            DeviceResponse::Ok
        } else {
            DeviceResponse::Busy
        }
    }

    fn init_device_settings(&mut self) {
        self.device_count = 0;
        self.sid_count = 0;
        self.number_of_sids = 0;
        self.sid_clock = SidClock::Pal;
        self.temp_queue.clear();

        self.device_id = vec![];
        self.device_base_reg = vec![];
        self.device_index = vec![];
        self.device_mappings = vec![];
        self.device_init_done = vec![];

        self.cycles_in_buffer.store(0, Ordering::Relaxed);
    }

    fn map_device_to_reg(&self, dev_nr: i32, reg: u8) -> u8 {
        let reg = self.filter_reg_for_unsupported_writes(dev_nr, reg);
        let base_reg = self.device_base_reg[dev_nr as usize];
        let socket_count = self.device_socket_count[dev_nr as usize];
        let socket_wrap = ((socket_count * 0x20) - 1) as u8;
        (reg + base_reg) & socket_wrap
    }

    fn filter_reg_for_unsupported_writes(&self, dev_nr: i32, reg: u8) -> u8 {
        let socket_count = self.device_socket_count[dev_nr as usize];
        if (reg as i32) >= socket_count * 0x20 {
            DUMMY_REG
        } else {
            reg
        }
    }

    fn send_command(&mut self, command: UsbSidCommand, dev_nr: i32) {
        if self.is_connected() && self.in_cmd_sender.send_timeout((command, dev_nr), Duration::from_millis(CMD_TIMEOUT_IN_MILLIS)).is_err() {
            self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
        }
    }

    fn is_player_aborted(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type == ABORTED || abort_type == ABORTING
    }

    fn is_usbsid_aborted(&self) -> bool {
        self.usbsid_aborted.load(Ordering::SeqCst)
    }
}
