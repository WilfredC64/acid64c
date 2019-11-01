// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::cmp::{min, max};
use std::io::prelude::*;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, str, thread, time};

const WRITE_BUFFER_SIZE: usize = 1024;      // 1 KB maximum to avoid network overhead
const RESPONSE_BUFFER_SIZE: usize = 260;
const BUFFER_SINGLE_WRITE_SIZE: usize = 4;  // cycles 2 bytes, register 1 byte and data 1 byte
const MAX_SID_WRITES: usize = WRITE_BUFFER_SIZE - BUFFER_SINGLE_WRITE_SIZE;
const WRITE_CYCLES_THRESHOLD: u32 = 63 * 312 * 5 / 2;
const CLIENT_WAIT_CYCLES_THRESHOLD: u32 = 4000;
const MIN_CYCLES_FOR_DELAY: u32 = 63 * 312 * 50;
const MIN_WAIT_TIME_BUSY_MS: u64 = 15;
const BUFFER_HEADER_SIZE: usize = 4;
const DEFAULT_DEVICE_COUNT_INTERFACE_V1: i32 = 2;

#[derive(Copy, Clone)]
pub enum SidClock {
    PAL = 0,
    NTSC = 1
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum SamplingMethod {
    BEST = 0,
    FAST = 1
}

#[derive(Copy, Clone)]
enum CommandResponse {
    Ok = 0,
    Busy,
    Error,
    Read,
    Version,
    Count,
    Info
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
    SetPsidHeader
}

pub struct NetworkSidDevice {
    sid_device: TcpStream,
    interface_version: i32,
    write_buffer: [u8; WRITE_BUFFER_SIZE],
    response_buffer: [u8; RESPONSE_BUFFER_SIZE],
    buffer_index: usize,
    buffer_cycles: u32,
    device_count: i32,
    number_of_sids: i32,
    sid_clock: SidClock,
    sampling_method: SamplingMethod,
    aborted: Arc<AtomicBool>
}

#[allow(dead_code)]
impl NetworkSidDevice {
    pub fn new(ip_address: &str, port: &str, aborted: Arc<AtomicBool>) -> NetworkSidDevice {
        let mut nsd_props = NetworkSidDevice {
            sid_device: TcpStream::connect([ip_address, port].join(":")).unwrap(),
            interface_version: 0,
            write_buffer: [0; WRITE_BUFFER_SIZE],
            response_buffer: [0; RESPONSE_BUFFER_SIZE],
            buffer_index: BUFFER_HEADER_SIZE,
            buffer_cycles: 0,
            device_count: 0,
            number_of_sids: 0,
            sid_clock: SidClock::PAL,
            sampling_method: SamplingMethod::BEST,
            aborted
        };

        nsd_props.interface_version = nsd_props.get_version() as i32;

        if nsd_props.interface_version >= 2 {
            nsd_props.device_count = nsd_props.get_config_count() as i32;
        } else {
            nsd_props.device_count = DEFAULT_DEVICE_COUNT_INTERFACE_V1;
        }
        nsd_props
    }

    #[inline]
    fn get_version(&mut self) -> i32 {
        self.try_flush_buffer(Command::GetVersion, 0, None)[0] as i32
    }

    #[inline]
    fn get_config_count(&mut self) -> i32 {
        self.try_flush_buffer(Command::GetConfigCount, 0, None)[0] as i32
    }

    pub fn get_device_count(&mut self) -> i32 {
        self.device_count
    }

    pub fn get_device_info(&mut self, device_number: i32) -> String {
        if self.interface_version >= 2 {
            let device = self.try_flush_buffer(Command::GetConfigInfo, device_number, None);

            if device.len() > 0 {
                return String::from_utf8(device).unwrap()
                    .replace("JSidDevice10_", "Default")
                    .replace("(", " - ")
                    .replace(")", "")
                    .replace("_", " - ")
                    .replace("6581", " 6581")
                    .replace("8580", " 8580")
                    .replace("  ", " ")
            }

            "Unknown".to_string()
        } else {
            if device_number == 0 {
                "Default 6581".to_string()
            } else {
                "Default 8580".to_string()
            }
        }
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
                sid_position
            } else {
                0
            };

            panning = min(panning, 100);
            panning = max(panning, -100);

            for sid_number in 0..self.number_of_sids {
                self.try_flush_buffer(Command::SetSidPosition, sid_number, Some(&[panning as u8]));
                panning = -panning;
            }
        }
    }

    pub fn set_sid_model(&mut self, device_number: i32, sid_model: i32) {
        if self.interface_version >= 2 {
            if sid_model < self.device_count {
                self.try_flush_buffer(Command::TrySetSidModel, device_number, Some(&[sid_model as u8]));
            }
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

    pub fn reset_sid(&mut self, device_number: i32) {
        let volume = 0x0f;
        self.try_flush_buffer(Command::TryReset, device_number, Some(&[volume]));

        for voice_number in 0..3 {
            let unmute = 0;
            self.try_flush_buffer(Command::Mute, device_number, Some(&[voice_number, unmute]));
        }
    }

    pub fn flush_buffers(&mut self, device_number: i32) {
        self.try_flush_buffer(Command::Flush, device_number, None);
    }

    pub fn dummy_write(&mut self, device_number: i32, cycles_input: u32) {
        self.write(device_number, cycles_input, 0x1e, 0);
    }

    pub fn write(&mut self, device_number: i32, cycles_input: u32, reg: u8, data: u8) {
        let cycles = if cycles_input > 0xffff {
            let device_number = self.convert_device_number(device_number);
            self.delay(device_number, cycles_input, 0x100)
        } else {
            cycles_input
        };

        self.add_to_buffer(reg, data, cycles);

        if (self.buffer_index >= MAX_SID_WRITES) || (self.buffer_cycles >= WRITE_CYCLES_THRESHOLD) {
            let device_number = self.convert_device_number(device_number);
            self.try_flush_buffer(Command::TryWrite, device_number, None);
        }
    }

    #[inline]
    fn convert_device_number(&mut self, device_number: i32) -> i32 {
        if self.interface_version == 1 {
            return device_number | (self.sid_clock as i32) << 1 | (self.sampling_method as i32) << 2;
        }
        device_number
    }

    fn delay(&mut self, device_number: i32, cycles: u32, minimum_cycles_to_remain: u32) -> u32 {
        self.flush_pending_writes(device_number);

        let mut cycles = cycles - minimum_cycles_to_remain;
        while cycles > 0xffff {
            self.flush_delay(device_number, 0xffff);
            cycles -= 0xffff;
        }

        if cycles > MIN_CYCLES_FOR_DELAY {
            self.flush_delay(device_number, cycles as u16);
            cycles = 0;
        }

        minimum_cycles_to_remain + cycles
    }

    #[inline]
    fn flush_delay(&mut self, device_number: i32, cycles: u16) {
        self.try_flush_buffer(Command::TryDelay, device_number, Some(&[(cycles >> 8) as u8, (cycles & 0xff) as u8]));
    }

    #[inline]
    fn flush_pending_writes(&mut self, device_number: i32) {
        if self.buffer_index > BUFFER_HEADER_SIZE {
            self.try_flush_buffer(Command::TryWrite, device_number, None);
        }
    }

    #[inline]
    fn are_multiple_sid_chips_supported(&mut self) -> bool {
        self.interface_version > 1
    }

    #[inline]
    fn add_to_buffer(&mut self, reg: u8, data: u8, cycles: u32) {
        let sid_reg = if !self.are_multiple_sid_chips_supported() && reg >= 0x20 && self.number_of_sids > 1 {
            // version 1 doesn't support stereo mixing, so ignore second SID chip
            0x1e
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
        self.buffer_cycles += cycles & 0xffff;
    }

    fn try_flush_buffer(&mut self, command: Command, device_number: i32, arguments: Option<&[u8]>) -> Vec<u8> {
        self.set_command(command, device_number as u8, arguments);

        let cycles_sent_to_server = self.buffer_cycles;
        let mut idle_time = MIN_WAIT_TIME_BUSY_MS;

        loop {
            let (device_state, result) = self.flush_buffer();

            if let CommandResponse::Busy = device_state {
                if self.aborted.load(Ordering::SeqCst) {
                    return vec![0];
                }

                thread::sleep(time::Duration::from_millis(idle_time));
                idle_time = 1;
                continue;
            } else {
                if let Command::TryWrite = command {
                    if cycles_sent_to_server > CLIENT_WAIT_CYCLES_THRESHOLD {
                        thread::sleep(time::Duration::from_millis(1));
                    }
                }

                return result;
            }
        }
    }

    fn flush_buffer(&mut self) -> (CommandResponse, Vec<u8>) {
        self.set_data_length(self.buffer_index);

        let response = self.send_data();

        if let CommandResponse::Error = response {
            return (CommandResponse::Error, vec![0]);
        }

        let (response, data) = self.read_data();

        if let CommandResponse::Error = response {
            self.read_error_message();
        }

        (response, data)
    }

    #[inline]
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

    #[inline]
    fn send_data(&mut self) -> CommandResponse {
        let result = self.sid_device.write(&self.write_buffer[0..self.buffer_index]);
        match result {
            Ok(size) => {
                if size != self.buffer_index {
                    return self.generate_error()
                }
            },
            Err(_) => {
                return self.generate_error();
            }
        }

        CommandResponse::Ok
    }

    #[inline]
    fn read_data(&mut self) -> (CommandResponse, Vec<u8>) {
        let result = self.sid_device.read(&mut self.response_buffer);

        match result {
            Ok(size) => {
                if size <= 0 {
                    return (self.generate_error(), vec![0])
                }
            },
            Err(_) => {
                return (self.generate_error(), vec![0]);
            }
        }

        let result_size = result.unwrap();

        self.handle_response(result_size)
    }

    #[inline]
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
            return (CommandResponse::Ok, self.response_buffer[2..result_size].to_vec());
        }

        (CommandResponse::Error, vec![0])
    }

    fn read_error_message(&mut self) -> (CommandResponse, u8) {
        let result = self.sid_device.read(&mut self.write_buffer[0..MAX_SID_WRITES]);

        match result {
            Ok(size) => {
                panic!("{}", str::from_utf8(&self.write_buffer[0..size]).unwrap());
            },
            Err(_) => {
                return (self.generate_error(), 0);
            }
        }
    }

    #[inline]
    fn reset_buffer(&mut self) {
        self.buffer_index = BUFFER_HEADER_SIZE;
        self.buffer_cycles = 0;
    }

    #[inline]
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
            for item in arguments.iter() {
                self.write_buffer[self.buffer_index] = *item;
                self.buffer_index += 1;
            }
        }
    }
}