// Copyright (C) 2019 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::io::prelude::*;
use std::net::{TcpStream, Shutdown, ToSocketAddrs};
use std::sync::atomic::{Ordering, AtomicI32};
use std::{sync::Arc, str, thread, time};

use crate::utils::network;
use super::sid_device::{DeviceId, DeviceInfo, DeviceResponse, DUMMY_REG, SamplingMethod, SidClock, SidDevice, SidModel};
use super::{ABORT_NO, ABORTING, MIN_CYCLE_SID_WRITE};

const WRITE_BUFFER_SIZE: usize = 1024;      // 1 KB maximum to avoid network overhead
const RESPONSE_BUFFER_SIZE: usize = 260;
const BUFFER_SINGLE_WRITE_SIZE: usize = 4;  // cycles 2 bytes, register 1 byte and data 1 byte
const MAX_SID_WRITES: usize = WRITE_BUFFER_SIZE - BUFFER_SINGLE_WRITE_SIZE;
const WRITE_CYCLES_THRESHOLD: u32 = 63 * 312 / 2;
const CLIENT_WAIT_CYCLES_THRESHOLD: u32 = 20000;
const MIN_WAIT_TIME_BUSY_MILLIS: u64 = 3;
const MIN_CYCLES_AFTER_DELAY: u16 = 0x100;
const BUFFER_HEADER_SIZE: usize = 4;
const DEFAULT_DEVICE_COUNT_INTERFACE_V1: i32 = 2;
const SOCKET_CONNECTION_TIMEOUT: u64 = 1000;

enum CommandResponse {
    Ok = 0,
    Busy,
    Error,
    Read,
    Version,
    Count,
    Info,
    Aborted
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
enum Command {
    Flush = 0,
    TrySetSidCount,
    Mute,
    TryReset,
    TryDelay,
    TryWrite,
    TryRead,
    GetVersion,
    TrySetSampling,
    TrySetClock,
    GetConfigCount,
    GetConfigInfo,
    SetSidPosition,
    SetSidLevel,
    TrySetSidModel,
    SetDelay,
    SetFadeIn,
    SetFadeOut,
    SetSidHeader
}

pub struct NetworkSidDeviceFacade {
    pub ns_device: NetworkSidDevice
}

impl SidDevice for NetworkSidDeviceFacade {
    fn get_device_id(&mut self, _dev_nr: i32) -> DeviceId { DeviceId::NetworkSidDevice }

    fn disconnect(&mut self, _dev_nr: i32) {
        self.ns_device.disconnect();
    }

    fn is_connected(&mut self, _dev_nr: i32) -> bool {
        self.ns_device.is_connected()
    }

    fn get_last_error(&mut self, _dev_nr: i32) -> Option<String> {
        self.ns_device.get_last_error()
    }

    fn test_connection(&mut self, _dev_nr: i32) {
        self.ns_device.test_connection();
    }

    fn can_pair_devices(&mut self, _dev1: i32, _dev2: i32) -> bool {
        true
    }

    fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.ns_device.get_device_count()
    }

    fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo {
        self.ns_device.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, _dev_nr: i32, sid_count: i32) {
        self.ns_device.set_sid_count(sid_count);
    }

    fn set_sid_position(&mut self, _dev_nr: i32, sid_position: i8) {
        self.ns_device.set_sid_position(sid_position);
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32, _sid_model: SidModel) {
        self.ns_device.set_sid_model(dev_nr, sid_socket);
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.ns_device.set_sid_clock(sid_clock);
    }

    fn set_sampling_method(&mut self, _dev_nr: i32, sampling_method: SamplingMethod) {
        self.ns_device.set_sampling_method(sampling_method);
    }

    fn set_sid_header(&mut self, _dev_nr: i32, sid_header: Vec<u8>) {
        self.ns_device.set_sid_header(sid_header);
    }

    fn set_fade_in(&mut self, _dev_nr: i32, time_millis: u32) {
        self.ns_device.set_fade_in(time_millis);
    }

    fn set_fade_out(&mut self, _dev_nr: i32, time_millis: u32) {
        self.ns_device.set_fade_out(time_millis);
    }

    fn silent_all_sids(&mut self, _dev_nr: i32, write_volume: bool) {
        self.ns_device.silent_all_sids(write_volume);
    }

    fn silent_active_sids(&mut self, _dev_nr: i32, write_volume: bool) {
        self.ns_device.silent_all_sids(write_volume);
    }

    fn reset_all_sids(&mut self, _dev_nr: i32) {
        self.ns_device.reset_all_sids();
    }

    fn reset_active_sids(&mut self, _dev_nr: i32) {
        self.ns_device.reset_all_sids();
    }

    fn reset_all_buffers(&mut self, _dev_nr: i32) {
        self.ns_device.reset_all_buffers(0);
    }

    fn enable_turbo_mode(&mut self, _dev_nr: i32) {
        self.ns_device.enable_turbo_mode();
    }

    fn disable_turbo_mode(&mut self, _dev_nr: i32) {
        self.ns_device.disable_turbo_mode();
    }

    fn dummy_write(&mut self, _dev_nr: i32, cycles: u32) {
        self.ns_device.dummy_write(0, cycles);
    }

    fn write(&mut self, _dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.ns_device.write(0, cycles, reg, data)
    }

    fn try_write(&mut self, _dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.ns_device.try_write(0, cycles, reg, data)
    }

    fn retry_write(&mut self, _dev_nr: i32) -> DeviceResponse {
        self.ns_device.retry_write(0)
    }

    fn force_flush(&mut self, _dev_nr: i32) {
        self.ns_device.force_flush(0);
    }

    fn set_native_device_clock(&mut self, _enabled: bool) {
        // not supported
    }

    fn get_device_clock(&mut self, _dev_nr: i32) -> SidClock {
        self.ns_device.get_device_clock()
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

pub struct NetworkSidDevice {
    sid_device: Option<TcpStream>,
    interface_version: i32,
    write_buffer: [u8; WRITE_BUFFER_SIZE],
    response_buffer: [u8; RESPONSE_BUFFER_SIZE],
    buffer_index: usize,
    buffer_cycles: u32,
    device_count: i32,
    number_of_sids: i32,
    sid_clock: SidClock,
    sid_model: i32,
    sampling_method: SamplingMethod,
    turbo_mode: bool,
    last_error: Option<String>,
    abort_type: Arc<AtomicI32>
}

#[allow(dead_code)]
impl NetworkSidDevice {
    pub fn new(abort_type: Arc<AtomicI32>) -> NetworkSidDevice {
        NetworkSidDevice {
            sid_device: None,
            interface_version: 0,
            write_buffer: [0; WRITE_BUFFER_SIZE],
            response_buffer: [0; RESPONSE_BUFFER_SIZE],
            buffer_index: BUFFER_HEADER_SIZE,
            buffer_cycles: 0,
            device_count: 0,
            number_of_sids: 0,
            sid_clock: SidClock::Pal,
            sid_model: 0,
            sampling_method: SamplingMethod::Best,
            turbo_mode: false,
            last_error: None,
            abort_type
        }
    }

    pub fn connect(&mut self, host_name: &str, port: &str) -> Result<(), String> {
        self.disconnect();
        self.last_error = None;

        if !network::is_local_ip_address(host_name) {
            return Err(format!("{host_name} is not in the local network or invalid."));
        }

        let mut addresses = [host_name, port].join(":").to_socket_addrs().unwrap();

        if let Some(socket_address) = addresses.find(|socket| socket.is_ipv4()) {
            if let Ok(stream) = TcpStream::connect_timeout(&socket_address, time::Duration::from_millis(SOCKET_CONNECTION_TIMEOUT)) {
                self.sid_device = Some(stream);

                self.interface_version = self.get_version();

                if self.interface_version >= 2 {
                    self.device_count = self.get_config_count();
                } else {
                    self.device_count = DEFAULT_DEVICE_COUNT_INTERFACE_V1;
                }

                Ok(())
            } else {
                Err(format!("Could not connect to: {}.", &socket_address))
            }
        } else {
            Err(format!("Could not find IPV4 address for: {}.", &host_name))
        }
    }

    pub fn disconnect(&mut self) {
        if self.sid_device.is_some() {
            self.sid_device.as_ref().unwrap().shutdown(Shutdown::Both).ok();
            self.sid_device = None;
        }
        self.init_to_default();
    }

    fn init_to_default(&mut self) {
        self.device_count = 0;
        self.interface_version = 0;
        self.number_of_sids = 0;
        self.sid_clock = SidClock::Pal;
        self.sid_model = 0;
        self.sampling_method = SamplingMethod::Best;
        self.reset_buffer();
    }

    fn disconnect_with_error(&mut self, error_message: String) {
        self.last_error = Some(error_message);
        self.disconnect();
    }

    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.sid_device.is_some()
    }

    fn get_version(&mut self) -> i32 {
        self.try_flush_buffer(Command::GetVersion, 0, None).1[0] as i32
    }

    fn get_config_count(&mut self) -> i32 {
        self.try_flush_buffer(Command::GetConfigCount, 0, None).1[0] as i32
    }

    pub fn test_connection(&mut self) {
        self.try_flush_buffer(Command::GetVersion, 0, None);
    }

    pub fn get_device_count(&self) -> i32 {
        self.device_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo {
        let device_name = if self.interface_version >= 2 {
            let (_, device_name) = self.try_flush_buffer(Command::GetConfigInfo, dev_nr, None);

            if !device_name.is_empty() {
                String::from_utf8(device_name).unwrap()
                    .replace("JSidDevice10_", "Default")
                    .replace('(', " - ")
                    .replace(')', "")
                    .replace('_', " - ")
                    .replace("6581", " 6581")
                    .replace("8580", " 8580")
                    .replace("  ", " ")
            } else {
                "Unknown".to_string()
            }
        } else if dev_nr == 0 {
            "Default 6581".to_string()
        } else {
            "Default 8580".to_string()
        };

        DeviceInfo { id: device_name.clone(), name: device_name }
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;

        if self.interface_version >= 2 {
            self.try_flush_buffer(Command::TrySetSidCount, sid_count, None);
        }
    }

    pub fn set_sid_position(&mut self, sid_position: i8) {
        if self.interface_version >= 2 {
            let mut panning: i8 = if self.number_of_sids > 1 {
                sid_position.clamp(-100, 100)
            } else {
                0
            };

            for sid_number in 0..self.number_of_sids {
                self.try_flush_buffer(Command::SetSidPosition, sid_number, Some(&[panning as u8]));
                panning = -panning;
            }
        }
    }

    pub fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.sid_model = dev_nr;

        if self.interface_version >= 2 && dev_nr < self.device_count {
            self.try_flush_buffer(Command::TrySetSidModel, sid_socket, Some(&[dev_nr as u8]));
        }
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;

        if self.interface_version >= 2 {
            self.try_flush_buffer(Command::TrySetClock, 0, Some(&[sid_clock as u8]));
        }
    }

    pub fn set_sampling_method(&mut self, sampling_method: SamplingMethod) {
        self.sampling_method = sampling_method;

        if self.interface_version >= 2 {
            self.try_flush_buffer(Command::TrySetSampling, 0, Some(&[sampling_method as u8 ^ 1]));
        }
    }

    pub fn set_sid_header(&mut self, sid_header: Vec<u8>) {
        if self.interface_version >= 4 {
            self.try_flush_buffer(Command::SetSidHeader, 0, Some(&sid_header));
        }
    }

    pub fn set_fade_in(&mut self, time_millis: u32) {
        if self.interface_version >= 4 {
            self.try_flush_buffer(Command::SetFadeIn, 0, Some(&time_millis.to_be_bytes()));
        }
    }

    pub fn set_fade_out(&mut self, time_millis: u32) {
        if self.interface_version >= 4 {
            self.try_flush_buffer(Command::SetFadeOut, 0, Some(&time_millis.to_be_bytes()));
        }
    }

    pub fn silent_all_sids(&mut self, write_volume: bool) {
        for i in 0..self.number_of_sids {
            self.silent_sid(i, write_volume);
        }
        self.force_flush(0);
    }

    fn silent_sid(&mut self, dev_nr: i32, write_volume: bool) {
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x00, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x01, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x07, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x08, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0e, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0f, 0);

        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x04, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0b, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x12, 0);

        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x05, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x06, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0c, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0d, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x13, 0);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x14, 0);

        if write_volume {
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x18, 0);
        }
    }

    fn device_reset(&mut self, dev_nr: i32) {
        let default_volume = 0u8;
        let dev_nr = self.convert_device_number(dev_nr);
        self.try_flush_buffer(Command::TryReset, dev_nr, Some(&[default_volume]));

        self.unmute_all_voices(dev_nr);
    }

    fn unmute_all_voices(&mut self, dev_nr: i32) {
        self.try_flush_buffer(Command::Mute, dev_nr, Some(&[0, 0]));
        self.try_flush_buffer(Command::Mute, dev_nr, Some(&[1, 0]));
        self.try_flush_buffer(Command::Mute, dev_nr, Some(&[2, 0]));
        if self.interface_version >= 3 {
            self.try_flush_buffer(Command::Mute, dev_nr, Some(&[3, 0]));
        }
    }

    pub fn reset_all_sids(&mut self) {
        self.device_reset(0);

        for i in 0..self.number_of_sids {
            self.reset_sid(i);
        }

        self.dummy_write(0, 40000);
        self.force_flush(0);
    }

    fn reset_sid(&mut self, dev_nr: i32) {
        if self.number_of_sids > 0 {
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x00, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x01, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x07, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x08, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0e, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0f, 0);

            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x04, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x0b, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, 0x12, 0);

            self.reset_sid_register(dev_nr, 0x02);
            self.reset_sid_register(dev_nr, 0x03);
            self.reset_sid_register(dev_nr, 0x04);
            self.reset_sid_register(dev_nr, 0x05);
            self.reset_sid_register(dev_nr, 0x06);

            self.reset_sid_register(dev_nr, 0x09);
            self.reset_sid_register(dev_nr, 0x0a);
            self.reset_sid_register(dev_nr, 0x0b);
            self.reset_sid_register(dev_nr, 0x0c);
            self.reset_sid_register(dev_nr, 0x0d);

            self.reset_sid_register(dev_nr, 0x10);
            self.reset_sid_register(dev_nr, 0x11);
            self.reset_sid_register(dev_nr, 0x12);
            self.reset_sid_register(dev_nr, 0x13);
            self.reset_sid_register(dev_nr, 0x14);

            self.reset_sid_register(dev_nr, 0x15);
            self.reset_sid_register(dev_nr, 0x16);
            self.reset_sid_register(dev_nr, 0x17);
            self.reset_sid_register(dev_nr, 0x19);
        }
    }

    fn reset_sid_register(&mut self, dev_nr: i32, reg: u8) {
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0xff);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x08);
        self.dummy_write(dev_nr, 50);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x00);
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        self.reset_buffer();
        if self.number_of_sids > 0 {
            self.try_flush_buffer(Command::Flush, dev_nr, None);
        }
    }

    pub fn enable_turbo_mode(&mut self) {
        self.turbo_mode = true;
    }

    pub fn disable_turbo_mode(&mut self) {
        self.turbo_mode = false;
    }

    pub fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        self.write(dev_nr, cycles, DUMMY_REG, 0);
    }

    pub fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        let cycles = self.delay(dev_nr, cycles);
        self.add_to_buffer(reg, data, cycles);

        if (self.buffer_index >= MAX_SID_WRITES) || (self.buffer_cycles >= WRITE_CYCLES_THRESHOLD) {
            self.force_flush(dev_nr);
        }
        DeviceResponse::Ok
    }

    pub fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        let cycles = self.delay(dev_nr, cycles);
        self.add_to_buffer(reg, data, cycles);

        if (self.buffer_index >= MAX_SID_WRITES) || (self.buffer_cycles >= WRITE_CYCLES_THRESHOLD) {
            self.try_write_buffer(Command::TryWrite, dev_nr, None)
        } else {
            DeviceResponse::Ok
        }
    }

    pub fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        if self.buffer_index > BUFFER_HEADER_SIZE {
            self.try_write_buffer(Command::TryWrite, dev_nr, None)
        } else {
            DeviceResponse::Ok
        }
    }

    fn delay(&mut self, dev_nr: i32, cycles: u32) -> u16 {
        if cycles > 0xffff {
            self.split_delay(dev_nr, cycles, MIN_CYCLES_AFTER_DELAY)
        } else {
            cycles as u16
        }
    }

    fn try_write_buffer(&mut self, command: Command, dev_nr: i32, arguments: Option<&[u8]>) -> DeviceResponse {
        if self.is_connected() {
            let dev_nr = self.convert_device_number(dev_nr);
            self.set_command(command, dev_nr as u8, arguments);

            let cycles_sent_to_server = self.buffer_cycles;
            let (device_state, _) = self.flush_buffer();

            match device_state {
                CommandResponse::Ok => {
                    if cycles_sent_to_server > CLIENT_WAIT_CYCLES_THRESHOLD {
                        thread::sleep(time::Duration::from_millis(MIN_WAIT_TIME_BUSY_MILLIS));
                    }
                    DeviceResponse::Ok
                },
                CommandResponse::Busy => {
                    thread::sleep(time::Duration::from_millis(MIN_WAIT_TIME_BUSY_MILLIS));
                    DeviceResponse::Busy
                },
                CommandResponse::Error => DeviceResponse::Error,
                _ => DeviceResponse::Ok
            }
        } else {
            DeviceResponse::Ok
        }
    }

    pub fn force_flush(&mut self, dev_nr: i32) {
        if self.buffer_index > BUFFER_HEADER_SIZE {
            let dev_nr = self.convert_device_number(dev_nr);
            self.try_flush_buffer(Command::TryWrite, dev_nr, None);
        }
    }

    pub fn get_device_clock(&self) -> SidClock {
        self.sid_clock
    }

    fn convert_device_number(&mut self, dev_nr: i32) -> i32 {
        if self.interface_version == 1 {
            return (self.sid_model & 0x01) | ((self.sid_clock as i32) << 1) | ((self.sampling_method as i32) << 2);
        }
        dev_nr
    }

    fn split_delay(&mut self, dev_nr: i32, cycles: u32, minimum_cycles_to_remain: u16) -> u16 {
        let dev_nr = self.convert_device_number(dev_nr);
        self.flush_pending_writes(dev_nr);

        let mut cycles = cycles - minimum_cycles_to_remain as u32;
        while cycles >= 0xffff {
            let (device_state, _) = self.flush_delay(dev_nr, 0xffff);
            if let CommandResponse::Aborted = device_state {
                return 0;
            }
            cycles -= 0xffff;
        }

        if cycles + minimum_cycles_to_remain as u32 > 0xffff {
            self.flush_delay(dev_nr, cycles as u16);
            cycles = 0;
        }

        minimum_cycles_to_remain + cycles as u16
    }

    fn flush_delay(&mut self, dev_nr: i32, cycles: u16) -> (CommandResponse, Vec<u8>) {
        self.try_flush_buffer(Command::TryDelay, dev_nr, Some(&[(cycles >> 8) as u8, (cycles & 0xff) as u8]))
    }

    fn flush_pending_writes(&mut self, dev_nr: i32) {
        if self.buffer_index > BUFFER_HEADER_SIZE {
            self.try_flush_buffer(Command::TryWrite, dev_nr, None);
        }
    }

    fn are_multiple_sid_chips_supported(&mut self) -> bool {
        self.interface_version > 1
    }

    fn add_to_buffer(&mut self, reg: u8, data: u8, cycles: u16) {
        let sid_reg = if !self.are_multiple_sid_chips_supported() && reg >= 0x20 && self.number_of_sids > 1 {
            // version 1 doesn't support stereo mixing, so ignore second SID chip
            DUMMY_REG
        } else {
            reg
        };

        let sid_chip_number = if sid_reg < 0x20 || self.number_of_sids < 2 {
            0
        } else if sid_reg < 0x40 || self.number_of_sids < 3 {
            1
        } else {
            2
        };

        self.write_buffer[self.buffer_index] = (cycles >> 8) as u8;
        self.write_buffer[self.buffer_index + 1] = (cycles & 0xff) as u8;
        self.write_buffer[self.buffer_index + 2] = (sid_chip_number << 5) as u8 + (sid_reg & 0x1f);
        self.write_buffer[self.buffer_index + 3] = data;
        self.buffer_index += 4;
        self.buffer_cycles += cycles as u32;
    }

    fn try_flush_buffer(&mut self, command: Command, dev_nr: i32, arguments: Option<&[u8]>) -> (CommandResponse, Vec<u8>) {
        if self.is_connected() {
            self.set_command(command, dev_nr as u8, arguments);

            let cycles_sent_to_server = self.buffer_cycles;
            let mut idle_time = MIN_WAIT_TIME_BUSY_MILLIS;

            loop {
                let (device_state, result) = self.flush_buffer();

                if let CommandResponse::Busy = device_state {
                    if self.is_aborted() {
                        self.reset_buffer();
                        return (CommandResponse::Aborted, vec![0]);
                    }

                    if !self.turbo_mode {
                        if let Command::TryWrite = command {
                            thread::sleep(time::Duration::from_millis(idle_time));
                        } else {
                            thread::yield_now();
                        }
                    }
                    idle_time = 1;
                    continue;
                } else {
                    if !self.turbo_mode {
                        if let Command::TryWrite = command {
                            if cycles_sent_to_server > CLIENT_WAIT_CYCLES_THRESHOLD {
                                thread::sleep(time::Duration::from_millis(1));
                            }
                        }
                    }

                    return (device_state, result);
                }
            }
        }
        (CommandResponse::Aborted, vec![0])
    }

    fn flush_buffer(&mut self) -> (CommandResponse, Vec<u8>) {
        self.set_data_length(self.buffer_index);

        let response = self.send_data();

        if let CommandResponse::Error = response {
            return (CommandResponse::Error, vec![0]);
        }

        self.read_data()
    }

    fn is_aborted(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type != ABORT_NO && abort_type != ABORTING
    }

    fn set_data_length(&mut self, data_length: usize) {
        let data_length = if self.buffer_index < BUFFER_HEADER_SIZE {
            self.buffer_index = BUFFER_HEADER_SIZE;
            0
        } else {
            data_length - BUFFER_HEADER_SIZE
        };

        self.write_buffer[2] = ((data_length >> 8) & 0xff) as u8;
        self.write_buffer[3] = (data_length & 0xff) as u8;
    }

    fn send_data(&mut self) -> CommandResponse {
        if self.sid_device.is_some() {
            let result = self.sid_device.as_ref().unwrap().write(&self.write_buffer[0..self.buffer_index]);
            match result {
                Ok(size) => {
                    if size != self.buffer_index {
                        self.disconnect_with_error("Failure during network write.".to_string());
                        return self.generate_error()
                    }
                },
                Err(_) => {
                    self.disconnect_with_error("Failure during network write.".to_string());
                    return self.generate_error();
                }
            }
        }

        CommandResponse::Ok
    }

    fn read_data(&mut self) -> (CommandResponse, Vec<u8>) {
        if self.sid_device.is_some() {
            let result = self.sid_device.as_ref().unwrap().read(&mut self.response_buffer);

            match result {
                Ok(size) => {
                    if size == 0 {
                        self.disconnect_with_error("Failure during network write.".to_string());
                        return (self.generate_error(), vec![0])
                    }
                    self.handle_response(size)
                },
                Err(_) => {
                    self.disconnect_with_error("Failure during network write.".to_string());
                    (self.generate_error(), vec![0])
                }
            }
        } else {
            (self.generate_error(), vec![0])
        }
    }

    fn handle_response(&mut self, result_size: usize) -> (CommandResponse, Vec<u8>) {
        let response = self.response_buffer[0];

        if response == CommandResponse::Busy as u8 {
            return (CommandResponse::Busy, vec![0]);
        }

        self.reset_buffer();

        if response == CommandResponse::Ok as u8 {
            return (CommandResponse::Ok, vec![0]);
        }

        if ((response == CommandResponse::Read as u8) ||
            (response == CommandResponse::Version as u8) ||
            (response == CommandResponse::Count as u8)) && result_size == 2 {
            return (CommandResponse::Ok, vec![self.response_buffer[1]]);
        }

        if response == CommandResponse::Info as u8 && result_size >= 2 {
            return (CommandResponse::Ok, self.response_buffer[2..result_size - 1].to_vec());
        }

        eprintln!("error: {}", str::from_utf8(&self.response_buffer[1..result_size]).unwrap());
        (CommandResponse::Error, vec![0])
    }

    fn reset_buffer(&mut self) {
        self.buffer_index = BUFFER_HEADER_SIZE;
        self.buffer_cycles = 0;
    }

    fn generate_error(&mut self) -> CommandResponse {
        self.reset_buffer();
        CommandResponse::Error
    }

    fn set_command(&mut self, command: Command, argument: u8, optional_arguments: Option<&[u8]>) {
        self.write_buffer[0] = command as u8;
        self.write_buffer[1] = argument;
        self.write_buffer[2] = 0;
        self.write_buffer[3] = 0;

        if let Command::TryWrite = command {
            return;
        }

        self.reset_buffer();

        if let Some(arguments) = optional_arguments {
            for &item in arguments.iter() {
                self.write_buffer[self.buffer_index] = item;
                self.buffer_index += 1;
            }
        }
    }
}
