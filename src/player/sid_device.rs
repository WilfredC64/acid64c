// Copyright (C) 2020 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SidClock {
    Pal = 0,
    Ntsc = 1,
    OneMhz = 2
}

pub enum SidModel {
    Mos6581 = 0,
    Mos8580 = 1
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum SamplingMethod {
    Best = 0,
    Fast = 1
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DeviceResponse {
    Ok = 0,
    Busy = 1,
    Error = 2
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DeviceId {
    HardsidUsb = 0,
    NetworkSidDevice = 1,
    UltimateDevice = 2,
    SidBlaster = 3
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String
}

pub const DUMMY_REG: u8 = 0x1e;

pub trait SidDevice {
    fn get_device_id(&mut self, dev_nr: i32) -> DeviceId;

    fn disconnect(&mut self, dev_nr: i32);

    fn is_connected(&mut self, dev_nr: i32) -> bool;

    fn get_last_error(&mut self, dev_nr: i32) -> Option<String>;

    fn test_connection(&mut self, dev_nr: i32);

    fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool;

    fn get_device_count(&mut self, dev_nr: i32) -> i32;

    fn get_device_info(&mut self, dev_nr: i32) -> DeviceInfo;

    fn set_sid_count(&mut self, dev_nr: i32, sid_count: i32);

    fn set_sid_position(&mut self, dev_nr: i32, sid_position: i8);

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32, sid_model: SidModel);

    fn set_sid_clock(&mut self, dev_nr: i32, sid_clock: SidClock);

    fn set_sampling_method(&mut self, dev_nr: i32, sampling_method: SamplingMethod);

    fn set_sid_header(&mut self, dev_nr: i32, sid_header: Vec<u8>);

    fn set_fade_in(&mut self, dev_nr: i32, time_millis: u32);

    fn set_fade_out(&mut self, dev_nr: i32, time_millis: u32);

    fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool);

    fn silent_active_sids(&mut self, dev_nr: i32, write_volume: bool);

    fn reset_all_sids(&mut self, dev_nr: i32);

    fn reset_active_sids(&mut self, dev_nr: i32);

    fn reset_all_buffers(&mut self, dev_nr: i32);

    fn enable_turbo_mode(&mut self, dev_nr: i32);

    fn disable_turbo_mode(&mut self, dev_nr: i32);

    fn dummy_write(&mut self, dev_nr: i32, cycles: u32);

    fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse;

    fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse;

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse;

    fn force_flush(&mut self, dev_nr: i32);

    fn set_native_device_clock(&mut self, enabled: bool);

    fn get_device_clock(&mut self, dev_nr: i32) -> SidClock;

    fn has_remote_sidplayer(&mut self, dev_nr: i32) -> bool;

    fn send_sid(&mut self, dev_nr: i32, filename: &str, song_number: i32, sid_data: &[u8], ssl_data: &[u8]);

    fn stop_sid(&mut self, dev_nr: i32);

    fn set_cycles_in_fifo(&mut self, dev_nr: i32, cycles: u32);
}
