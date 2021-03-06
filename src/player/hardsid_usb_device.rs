// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::clock_adjust::ClockAdjust;
use super::hardsid_usb::{HardSidUsb, HSID_USB_STATE_OK, HSID_USB_STATE_ERROR, HSID_USB_STATE_BUSY, DEV_TYPE_HS_4U, DEV_TYPE_HS_UPLAY, DEV_TYPE_HS_UNO};
use super::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse};
use super::{ABORT_NO, ABORTING, MIN_CYCLE_SID_WRITE};

use std::collections::VecDeque;
use std::sync::atomic::{Ordering, AtomicI32};
use std::{sync::Arc, thread, time};

const HSID_BUSY_WAIT_MS: u64 = 5;
const ERROR_MSG_DEVICE_FAILURE: &str = "Failure occurred during interaction with device.";
const ERROR_MSG_INIT_DEVICE: &str = "Initializing HardSID USB device failed.";
const ERROR_MSG_NO_HARDSID_FOUND: &str = "No HardSID USB device found.";
const ERROR_MSG_DEVICE_COUNT_CHANGED: &str = "Number of devices is changed.";

pub struct HardsidUsbDeviceFacade {
    pub hs_device: HardsidUsbDevice
}

impl SidDevice for HardsidUsbDeviceFacade {
    fn disconnect(&mut self, _dev_nr: i32) {
        self.hs_device.disconnect();
    }

    fn is_connected(&mut self, _dev_nr: i32) -> bool {
        self.hs_device.is_connected()
    }

    fn get_last_error(&mut self, _dev_nr: i32) -> Option<String> {
        self.hs_device.get_last_error()
    }

    fn test_connection(&mut self, dev_nr: i32) {
        self.hs_device.test_connection(dev_nr);
    }

    fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        self.hs_device.can_pair_devices(dev1, dev2)
    }

    fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.hs_device.get_device_count()
    }

    fn get_device_info(&mut self, dev_nr: i32) -> String {
        self.hs_device.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, _dev_nr: i32, sid_count: i32) {
        self.hs_device.set_sid_count(sid_count);
    }

    fn set_sid_position(&mut self, _dev_nr: i32, _sid_position: i8) {
        // not supported
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.hs_device.set_sid_model(dev_nr, sid_socket);
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.hs_device.set_sid_clock(sid_clock);
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

    fn silent_all_sids(&mut self, _dev_nr: i32, write_volume: bool) {
        self.hs_device.silent_all_sids(write_volume);
    }

    fn silent_sid(&mut self, dev_nr: i32, write_volume: bool) {
        self.hs_device.silent_sid(dev_nr, write_volume);
    }

    fn device_reset(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn reset_all_sids(&mut self, _dev_nr: i32) {
        self.hs_device.reset_all_sids();
    }

    fn reset_sid(&mut self, dev_nr: i32) {
        self.hs_device.reset_sid(dev_nr);
    }

    fn reset_all_buffers(&mut self, dev_nr: i32) {
        self.hs_device.reset_all_buffers(dev_nr);
    }

    fn enable_turbo_mode(&mut self, _dev_nr: i32) {
        self.hs_device.enable_turbo_mode();
    }

    fn disable_turbo_mode(&mut self, _dev_nr: i32) {
        self.hs_device.disable_turbo_mode();
    }

    fn dummy_write(&mut self, dev_nr: i32, cycles_input: u32) {
        self.hs_device.dummy_write(dev_nr, cycles_input);
    }

    fn write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) {
        self.hs_device.write(dev_nr, cycles_input, reg, data);
    }

    fn try_write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) -> DeviceResponse {
        self.hs_device.try_write(dev_nr, cycles_input, reg, data)
    }

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        self.hs_device.retry_write(dev_nr)
    }

    fn force_flush(&mut self, dev_nr: i32) {
        self.hs_device.force_flush(dev_nr);
    }

    fn set_native_device_clock(&mut self, enabled: bool) {
        self.hs_device.set_native_device_clock(enabled);
    }

    fn get_device_clock(&mut self, _dev_nr: i32) -> SidClock {
        self.hs_device.get_device_clock()
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq)]
pub enum DeviceCommand {
    Write = 0,
    Delay = 1
}

#[derive(Copy, Clone)]
pub struct SidWrite {
    pub command: DeviceCommand,
    pub reg: u8,
    pub data: u8,
    pub cycles: u32
}

impl SidWrite {
    pub fn new(command: DeviceCommand, reg: u8, data: u8, cycles: u32) -> SidWrite {
        SidWrite {
            command,
            reg,
            data,
            cycles,
        }
    }
}

pub struct HardsidUsbDevice {
    sid_device: Option<HardSidUsb>,
    device_count: i32,
    sid_count: i32,
    number_of_sids: i32,
    sid_clock: SidClock,
    turbo_mode: bool,
    device_type: Vec<u8>,
    device_id: Vec<u8>,
    device_base_reg: Vec<u8>,
    abort_type: Arc<AtomicI32>,
    last_error: Option<String>,
    device_mappings: Vec<i32>,
    sid_write_fifo: VecDeque<SidWrite>,
    use_native_device_clock: bool,
    clock_adjust: ClockAdjust,
    cycles_to_compensate: u32,
    device_init_done: Vec<bool>
}

#[allow(dead_code)]
impl HardsidUsbDevice {
    pub fn new(abort_type: Arc<AtomicI32>) -> HardsidUsbDevice {
        HardsidUsbDevice {
            sid_device: None,
            device_count: 0,
            sid_count: 0,
            number_of_sids: 0,
            sid_clock: SidClock::Pal,
            turbo_mode: false,
            device_type: vec![],
            device_id: vec![],
            device_base_reg: vec![],
            abort_type,
            last_error: None,
            device_mappings: vec![],
            sid_write_fifo: VecDeque::new(),
            use_native_device_clock: true,
            clock_adjust: ClockAdjust::new(),
            cycles_to_compensate: 0,
            device_init_done: vec![]
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.disconnect();
        self.last_error = None;

        let usb_device = HardSidUsb::new();
        let init_response = usb_device.init();
        if !init_response {
            Err(ERROR_MSG_INIT_DEVICE.to_string())
        } else {
            let dev_count = usb_device.get_dev_count();
            self.device_count = dev_count as i32;

            if dev_count > 0 {
                for i in 0..dev_count {
                    let dev_type = usb_device.get_device_type(i);
                    let dev_sid_count = usb_device.get_sid_count(i);
                    for j in 0..dev_sid_count {
                        self.device_type.push(dev_type);
                        self.device_id.push(i);
                        self.device_base_reg.push(j * 0x20);
                        self.device_mappings.push(j as i32);
                        self.device_init_done.push(false);
                    }
                }

                self.sid_count = self.device_id.len() as i32;

                self.sid_device = Some(usb_device);
                Ok(())
            } else {
                Err(ERROR_MSG_NO_HARDSID_FOUND.to_string())
            }
        }
    }

    pub fn disconnect(&mut self) {
        if self.sid_device.is_some() {
            self.sid_device.as_ref().unwrap().close();
            self.sid_device = None;
        }

        self.init_device_settings();
    }

    #[inline]
    fn init_device_settings(&mut self) {
        self.device_count = 0;
        self.sid_count = 0;
        self.number_of_sids = 0;
        self.sid_clock = SidClock::Pal;

        self.device_type = vec![];
        self.device_id = vec![];
        self.device_base_reg = vec![];
        self.device_mappings = vec![];
        self.device_init_done = vec![];

        self.init_write_state();
    }

    #[inline]
    fn init_write_state(&mut self) {
        self.sid_write_fifo.clear();
        self.cycles_to_compensate = 0;
        self.clock_adjust.init(self.sid_clock);
    }

    pub fn disconnect_with_error(&mut self, error_message: String) {
        self.last_error = Some(error_message);
        self.disconnect();
    }

    pub fn get_last_error(&mut self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.sid_device.is_some()
    }

    pub fn test_connection(&mut self, dev_nr: i32) {
        if self.is_connected() {
            let dev_count = self.sid_device.as_ref().unwrap().get_dev_count();

            if dev_count as i32 != self.device_count {
                self.disconnect_with_error(ERROR_MSG_DEVICE_COUNT_CHANGED.to_string());
            }

            self.dummy_write(dev_nr, MIN_CYCLE_SID_WRITE);
            self.force_flush(dev_nr);
        }
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        dev1 != dev2 &&
            self.device_id[dev1 as usize] == self.device_id[dev2 as usize] &&
            self.device_type[dev1 as usize] == DEV_TYPE_HS_4U
    }

    pub fn get_device_count(&mut self) -> i32 {
        self.sid_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> String {
        let dev_name = match self.device_type[dev_nr as usize] {
            DEV_TYPE_HS_4U => "HardSID 4U ",
            DEV_TYPE_HS_UPLAY => "HS UPlay ",
            DEV_TYPE_HS_UNO => "HardSID Uno ",
            _ => "Unknown HS "
        };
        dev_name.to_string() + &(dev_nr + 1).to_string()
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;
    }

    pub fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        if self.is_connected() {
            let current_val = self.device_mappings[sid_socket as usize];

            for i in 0..self.device_mappings.len() {
                if self.device_mappings[i] == dev_nr {
                    self.device_mappings[i] = current_val;
                    break;
                }
            }

            self.wait_for_uplay_activation(dev_nr, sid_socket);

            self.device_mappings[sid_socket as usize] = dev_nr;
        }
    }

    #[inline]
    fn wait_for_uplay_activation(&mut self, dev_nr: i32, sid_socket: i32) {
        if self.device_type[dev_nr as usize] == DEV_TYPE_HS_UPLAY
            && (self.device_mappings[sid_socket as usize] != dev_nr || !self.device_init_done[dev_nr as usize]) {
            self.device_init_done[dev_nr as usize] = true;
            // trigger SID selection by performing a dummy write to new device number
            self.dummy_write(dev_nr, MIN_CYCLE_SID_WRITE);

            // wait a while to finish the switching of the relay of the UPlay device
            thread::sleep(time::Duration::from_millis(400));
        }
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
        self.clock_adjust.init(sid_clock);
    }

    pub fn silent_all_sids(&mut self, write_volume: bool) {
        for i in 0..self.number_of_sids {
            self.silent_sid(i, write_volume);
        }
    }

    pub fn silent_sid(&mut self, dev_nr: i32, write_volume: bool) {
        if self.number_of_sids > 0 && self.is_connected() {
            let reg_base = self.device_base_reg[dev_nr as usize];
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x01, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x00, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x08, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x07, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0f, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0e, 0);

            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x04, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x05, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x06, 0);

            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0b, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0c, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0d, 0);

            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x12, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x13, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x14, 0);

            if write_volume {
                self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x18, 0);
            }

            self.force_flush(dev_nr);
        }
    }

    pub fn reset_all_sids(&mut self) {
        for i in 0..self.number_of_sids {
            self.reset_sid(i);
        }
    }

    pub fn reset_sid(&mut self, dev_nr: i32) {
        if self.number_of_sids > 0 && self.is_connected() {
            let reg_base = self.device_base_reg[dev_nr as usize];
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x04, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0b, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x12, 0);

            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x00, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x01, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x07, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x08, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0e, 0);
            self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg_base + 0x0f, 0);

            self.reset_sid_register(dev_nr, reg_base + 0x02);
            self.reset_sid_register(dev_nr, reg_base + 0x03);
            self.reset_sid_register(dev_nr, reg_base + 0x04);
            self.reset_sid_register(dev_nr, reg_base + 0x05);
            self.reset_sid_register(dev_nr, reg_base + 0x06);

            self.reset_sid_register(dev_nr, reg_base + 0x09);
            self.reset_sid_register(dev_nr, reg_base + 0x0a);
            self.reset_sid_register(dev_nr, reg_base + 0x0b);
            self.reset_sid_register(dev_nr, reg_base + 0x0c);
            self.reset_sid_register(dev_nr, reg_base + 0x0d);

            self.reset_sid_register(dev_nr, reg_base + 0x10);
            self.reset_sid_register(dev_nr, reg_base + 0x11);
            self.reset_sid_register(dev_nr, reg_base + 0x12);
            self.reset_sid_register(dev_nr, reg_base + 0x13);
            self.reset_sid_register(dev_nr, reg_base + 0x14);

            self.reset_sid_register(dev_nr, reg_base + 0x15);
            self.reset_sid_register(dev_nr, reg_base + 0x16);
            self.reset_sid_register(dev_nr, reg_base + 0x17);
            self.reset_sid_register(dev_nr, reg_base + 0x19);

            self.dummy_write(dev_nr, 40000);
            self.force_flush(dev_nr);
        }
    }

    #[inline]
    fn reset_sid_register(&mut self, dev_nr: i32, reg: u8) {
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0xff);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x08);
        self.dummy_write(dev_nr, 50);
        self.write(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x00);
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        if self.is_connected() {
            self.sid_device.as_ref().unwrap().abort_play(dev_nr as u8);
        }
    }

    pub fn enable_turbo_mode(&mut self) {
        self.turbo_mode = true;
    }

    pub fn disable_turbo_mode(&mut self) {
        self.turbo_mode = false;
    }

    pub fn dummy_write(&mut self, dev_nr: i32, cycles_input: u32) {
        let reg_base = self.device_base_reg[dev_nr as usize];
        self.write(dev_nr, cycles_input, reg_base + 0x1e, 0);
    }

    #[inline]
    fn are_multiple_sid_chips_supported(&mut self, dev_nr: i32) -> bool {
        self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U
    }

    #[inline]
    fn map_to_supported_device(&mut self, dev_nr: i32, reg: u8) -> u8 {
        if !self.are_multiple_sid_chips_supported(dev_nr) && reg >= 0x20 && self.number_of_sids > 1 {
            // ignore second SID chip for devices that don't support accessing multiple SID chip simultaneously
            0x1e
        } else {
            reg
        }
    }

    pub fn force_flush(&mut self, dev_nr: i32) {
        self.try_flush(dev_nr);
    }

    pub fn set_native_device_clock(&mut self, enabled: bool) {
        self.use_native_device_clock = enabled;
    }

    pub fn get_device_clock(&mut self) -> SidClock {
        if self.use_native_device_clock {
            SidClock::OneMhz
        } else {
            self.sid_clock
        }
    }

    pub fn write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) {
        if self.is_connected() {
            let reg = self.map_to_supported_device(dev_nr, reg);

            self.create_delay(cycles_input);
            self.create_write(reg, data);

            while !self.sid_write_fifo.is_empty() {
                let sid_write = self.sid_write_fifo.pop_front().unwrap();
                match sid_write.command {
                    DeviceCommand::Delay => self.try_delay_sync(dev_nr, sid_write.cycles as u16),
                    DeviceCommand::Write => self.try_write_sync(dev_nr, sid_write.reg, sid_write.data)
                }
            }
        }
    }

    #[inline]
    fn push_write(&mut self, command: DeviceCommand, reg: u8, data: u8, cycles: u32) {
        let sid_write = SidWrite::new(command, reg, data, cycles);
        self.sid_write_fifo.push_back(sid_write);
    }

    #[inline]
    fn create_write(&mut self, reg: u8, data: u8) {
        if !self.use_native_device_clock {
            self.adjust_frequency(reg, data);
        } else {
            self.push_write(DeviceCommand::Write, reg, data, 0);
        }
    }

    pub fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        if !self.sid_write_fifo.is_empty() {
            self.process_write_fifo(dev_nr)
        } else {
            DeviceResponse::Ok
        }
    }

    pub fn try_write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) -> DeviceResponse {
        if !self.sid_write_fifo.is_empty() {
            self.process_write_fifo(dev_nr)
        } else {
            let reg = self.map_to_supported_device(dev_nr, reg);

            self.create_delay(cycles_input);
            self.create_write(reg, data);

            self.process_write_fifo(dev_nr)
        }
    }

    #[inline]
    fn process_write_fifo(&mut self, dev_nr: i32) -> DeviceResponse {
        while !self.sid_write_fifo.is_empty() {
            let sid_write = self.sid_write_fifo.pop_front().unwrap();

            let device_state = match sid_write.command {
                DeviceCommand::Delay => self.try_delay_async(dev_nr, sid_write.cycles as u16),
                DeviceCommand::Write => self.try_write_async(dev_nr, sid_write.reg, sid_write.data)
            };

            match device_state {
                HSID_USB_STATE_BUSY => {
                    self.sid_write_fifo.push_front(sid_write);
                    thread::sleep(time::Duration::from_millis(HSID_BUSY_WAIT_MS));
                    return DeviceResponse::Busy
                },
                HSID_USB_STATE_ERROR => {
                    self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
                    return DeviceResponse::Error
                },
                _ => ()
            };

            if self.is_aborted() {
                break;
            }

            thread::sleep(time::Duration::from_millis(0));
        }

        DeviceResponse::Ok
    }

    #[inline]
    fn adjust_frequency(&mut self, reg: u8, data: u8) {
        let reg_offset = reg & 0x1f;

        if reg_offset < 0x10 {
            let voice_nr = reg_offset / 7;
            let reg_base = reg & 0xe0;
            let reg_offset = reg_offset % 7;

            match reg_offset {
                0x00 | 0x01 => self.adjust_frequency_for_voice(voice_nr, reg_base, reg_offset, data),
                _ => self.push_write(DeviceCommand::Write, reg, data, 0)
            }
        } else {
            self.push_write(DeviceCommand::Write, reg, data, 0);
        }
    }

    #[inline]
    fn adjust_frequency_for_voice(&mut self, voice_nr: u8, reg_base: u8, reg: u8, data: u8) {
        if reg <= 1 {
            let voice_index = voice_nr + (reg_base >> 5) * 3;

            self.clock_adjust.update_frequency(voice_index, reg, data);
            let last_freq = self.clock_adjust.get_last_scaled_freq(voice_index);
            let scaled_freq = self.clock_adjust.scale_frequency(voice_index);

            let voice_base = voice_nr * 7;

            let update_hi_freq = last_freq & 0xff00 != scaled_freq & 0xff00;

            if update_hi_freq {
                self.push_write(DeviceCommand::Write, 1 + voice_base + reg_base, (scaled_freq >> 8) as u8, 0);
                self.create_delay(MIN_CYCLE_SID_WRITE);
                self.cycles_to_compensate += MIN_CYCLE_SID_WRITE;
            }
            self.push_write(DeviceCommand::Write, 0 + voice_base + reg_base, (scaled_freq & 0xff) as u8, 0);
        }
    }

    #[inline]
    fn convert_device_info(&mut self, reg: u8) -> (i32, u8) {
        let sid_nr = reg >> 5;
        if sid_nr < self.number_of_sids as u8 {
            let dev_nr = self.device_mappings[sid_nr as usize];
            (dev_nr, reg & 0x1f)
        } else {
            (0, 0x1e)
        }
    }

    #[inline]
    fn try_write_sync(&mut self, dev_nr: i32, reg: u8, data: u8) {
        if self.is_connected() {
            let physical_dev_nr = self.device_id[dev_nr as usize];
            let (dev_nr, reg) = self.convert_device_info(reg);
            let base_reg = self.device_base_reg[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().write(physical_dev_nr as u8, reg | base_reg, data);

                if self.process_response(state) {
                    break;
                }
            }
        }
    }

    #[inline]
    fn try_write_async(&mut self, dev_nr: i32, reg: u8, data: u8) -> u8 {
        if self.is_connected() {
            let physical_dev_nr = self.device_id[dev_nr as usize];

            let (dev_nr, reg) = self.convert_device_info(reg);
            let base_reg = self.device_base_reg[dev_nr as usize];

            self.sid_device.as_ref().unwrap().write(physical_dev_nr as u8, reg | base_reg, data)
        } else {
            HSID_USB_STATE_OK
        }
    }

    #[inline]
    fn try_flush(&mut self, dev_nr: i32) {
        self.sid_write_fifo.clear();

        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().flush(dev_nr as u8);

                if self.process_response(state) {
                    break;
                }
            }
        }
    }

    #[inline]
    fn create_delay(&mut self, cycles: u32) {
        const MINIMUM_CYCLES: u32 = 100;

        let mut cycles = if !self.use_native_device_clock {
            self.clock_adjust.adjust_cycles(cycles)
        } else {
            cycles
        };

        if cycles > MIN_CYCLE_SID_WRITE {
            if cycles - MIN_CYCLE_SID_WRITE > self.cycles_to_compensate {
                cycles -= self.cycles_to_compensate;
                self.cycles_to_compensate = 0;
            } else {
                self.cycles_to_compensate -= cycles - MIN_CYCLE_SID_WRITE;
                cycles = MIN_CYCLE_SID_WRITE;
            }
        }

        if cycles > 0xffff {
            if cycles % 0xffff < MINIMUM_CYCLES {
                self.push_write(DeviceCommand::Delay, 0, 0, MINIMUM_CYCLES);
                cycles -= MINIMUM_CYCLES
            }

            while cycles > 0xffff {
                self.push_write(DeviceCommand::Delay, 0, 0, 0xffff);
                cycles -= 0xffff;
            }
        }

        if cycles >= MIN_CYCLE_SID_WRITE {
            self.push_write(DeviceCommand::Delay, 0, 0, cycles);
        } else {
            self.push_write(DeviceCommand::Delay, 0, 0, MIN_CYCLE_SID_WRITE);
            self.cycles_to_compensate += MIN_CYCLE_SID_WRITE - cycles;
        }
    }

    #[inline]
    fn try_delay_sync(&mut self, dev_nr: i32, cycles: u16) {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().delay(dev_nr as u8, cycles);

                if self.process_response(state) {
                    break;
                }
            }
        }
    }

    #[inline]
    fn try_delay_async(&mut self, dev_nr: i32, cycles: u16) -> u8 {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            self.sid_device.as_ref().unwrap().delay(dev_nr as u8, cycles)
        } else {
            HSID_USB_STATE_OK
        }
    }

    #[inline]
    fn process_response(&mut self, state: u8) -> bool {
        if state == HSID_USB_STATE_ERROR {
            self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
            return true;
        }

        if state != HSID_USB_STATE_BUSY || self.is_aborted() {
            return true;
        }

        if !self.turbo_mode {
            thread::sleep(time::Duration::from_millis(HSID_BUSY_WAIT_MS));
        } else {
            thread::sleep(time::Duration::from_millis(0));
        }

        false
    }

    #[inline]
    fn is_aborted(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type != ABORT_NO && abort_type != ABORTING
    }
}
