// Copyright (C) 2020 - 2022 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::clock_adjust::ClockAdjust;
use super::hardsid_usb::{HardSidUsb, HSID_USB_STATE_OK, HSID_USB_STATE_ERROR, HSID_USB_STATE_BUSY, DEV_TYPE_HS_4U, DEV_TYPE_HS_UPLAY, DEV_TYPE_HS_UNO};
use super::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse, DeviceId};
use super::{ABORT_NO, ABORTING, MIN_CYCLE_SID_WRITE};

use std::collections::VecDeque;
use std::sync::atomic::{Ordering, AtomicI32};
use std::{sync::Arc, thread, time};

const BUSY_WAIT_MILLIS: u64 = 1;
const ERROR_MSG_DEVICE_FAILURE: &str = "Failure occurred during interaction with device.";
const ERROR_MSG_INIT_DEVICE: &str = "Initializing HardSID USB device failed with error:";
const ERROR_MSG_NO_HARDSID_FOUND: &str = "No HardSID USB device found.";
const ERROR_MSG_DEVICE_COUNT_CHANGED: &str = "Number of devices is changed.";

const HS_MIN_CYCLE_SID_WRITE: u32 = 4;

const DUMMY_REG: u8 = 0x1e;

pub struct HardsidUsbDeviceFacade {
    pub hs_device: HardsidUsbDevice
}

impl SidDevice for HardsidUsbDeviceFacade {
    fn get_device_id(&mut self, _dev_nr: i32) -> DeviceId { DeviceId::HardsidUsb }

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

    fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool) {
        self.hs_device.silent_all_sids(dev_nr, write_volume);
    }

    fn silent_active_sids(&mut self, dev_nr: i32, write_volume: bool) {
        self.hs_device.silent_active_sids(dev_nr, write_volume);
    }

    fn reset_all_sids(&mut self, dev_nr: i32) {
        self.hs_device.reset_all_sids(dev_nr);
    }

    fn reset_active_sids(&mut self, dev_nr: i32) {
        self.hs_device.reset_active_sids(dev_nr);
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

    fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        self.hs_device.dummy_write(dev_nr, cycles);
    }

    fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        self.hs_device.write(dev_nr, cycles, reg, data);
    }

    fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.hs_device.try_write(dev_nr, cycles, reg, data)
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
pub enum DeviceCommand {
    Write = 0,
    Delay = 1
}

pub struct SidWrite {
    pub command: DeviceCommand,
    pub reg: u8,
    pub data: u8,
    pub cycles: u16
}

impl SidWrite {
    pub fn new(command: DeviceCommand, reg: u8, data: u8, cycles: u16) -> SidWrite {
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
    device_index: Vec<u8>,
    abort_type: Arc<AtomicI32>,
    last_error: Option<String>,
    device_mappings: Vec<i32>,
    sid_write_fifo: VecDeque<SidWrite>,
    use_native_device_clock: bool,
    clock_adjust: ClockAdjust,
    cycles_to_compensate: u32,
    device_init_done: Vec<bool>,
    prev_uplay_dev_nr: i32
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
            device_index: vec![],
            abort_type,
            last_error: None,
            device_mappings: vec![],
            sid_write_fifo: VecDeque::new(),
            use_native_device_clock: true,
            clock_adjust: ClockAdjust::new(),
            cycles_to_compensate: 0,
            device_init_done: vec![],
            prev_uplay_dev_nr: 0
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.disconnect();
        self.last_error = None;

        let hardsid_usb = HardSidUsb::load();
        if hardsid_usb.is_err() {
            return Err("hardsid_usb library could not be loaded.".to_string())
        }

        let usb_device = hardsid_usb.unwrap();

        if !usb_device.init_sidplay_mode() {
            let unknown_device = "unknown".to_string();
            let error = usb_device.get_last_error().unwrap_or(unknown_device);
            Err(format!("{} {}.", ERROR_MSG_INIT_DEVICE, error))
        } else {
            let dev_count = usb_device.get_dev_count();
            self.device_count = dev_count as i32;

            if dev_count > 0 {
                let mut dev_type_count = [0u8; 4];

                for i in 0..dev_count {
                    let dev_type = usb_device.get_device_type(i);
                    let dev_sid_count = usb_device.get_sid_count(i);

                    for j in 0..dev_sid_count {
                        self.device_type.push(dev_type);
                        self.device_id.push(i);
                        self.device_index.push(dev_type_count[dev_type as usize]);
                        self.device_base_reg.push(j * 0x20);
                        self.device_mappings.push(j as i32);
                        self.device_init_done.push(false);
                        dev_type_count[dev_type as usize] += 1;
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
            self.sid_device.as_mut().unwrap().close();
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
        self.device_index = vec![];
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

    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.sid_device.is_some()
    }

    pub fn test_connection(&mut self, dev_nr: i32) {
        if self.is_connected() {
            let dev_count = self.sid_device.as_mut().unwrap().get_dev_count();

            if dev_count as i32 != self.device_count {
                self.disconnect_with_error(ERROR_MSG_DEVICE_COUNT_CHANGED.to_string());
            } else if dev_nr >= 0 && dev_nr < self.device_base_reg.len() as i32 {
                let base_reg = self.device_base_reg[dev_nr as usize];

                self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + DUMMY_REG, 0);
                self.force_flush(dev_nr);
            }
        }
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        dev1 != dev2 &&
            self.device_id[dev1 as usize] == self.device_id[dev2 as usize] &&
            self.device_type[dev1 as usize] == DEV_TYPE_HS_4U
    }

    pub fn get_device_count(&self) -> i32 {
        self.sid_count
    }

    pub fn get_device_info(&self, dev_nr: i32) -> String {
        let dev_name = match self.device_type[dev_nr as usize] {
            DEV_TYPE_HS_4U => "HardSID 4U ",
            DEV_TYPE_HS_UPLAY => "HS UPlay ",
            DEV_TYPE_HS_UNO => "HardSID Uno ",
            _ => "Unknown HS "
        };
        let dev_index = self.device_index[dev_nr as usize];
        dev_name.to_string() + &(dev_index + 1).to_string()
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;
    }

    pub fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        if self.is_connected() {
            if sid_socket >= self.sid_count {
                return;
            }

            let mut new_dev_nr = dev_nr + sid_socket;
            let physical_dev_nr = self.device_id[dev_nr as usize];

            let mut dev_iter = self.device_id.iter();
            let physical_dev_nr_start = dev_iter.position(|&v| v == physical_dev_nr).unwrap_or(0) as i32;
            let mut dev_iter = self.device_id.iter();
            let physical_dev_nr_end = dev_iter.rposition(|&v| v == physical_dev_nr).unwrap_or(0) as i32;

            if new_dev_nr > physical_dev_nr_end {
                new_dev_nr = new_dev_nr - physical_dev_nr_end + physical_dev_nr_start - 1;
            }

            let old_dev_nr = self.device_mappings[sid_socket as usize];

            self.device_mappings[sid_socket as usize] = new_dev_nr;
            if old_dev_nr != new_dev_nr || !self.device_init_done[new_dev_nr as usize] {
                self.wait_for_uplay_activation(new_dev_nr);
            }
        }
    }

    #[inline]
    fn wait_for_uplay_activation(&mut self, dev_nr: i32) {
        if self.device_type[dev_nr as usize] == DEV_TYPE_HS_UPLAY {
            if self.device_init_done[dev_nr as usize] && self.prev_uplay_dev_nr == dev_nr {
                return;
            }

            self.prev_uplay_dev_nr = dev_nr;
            self.device_init_done[dev_nr as usize] = true;

            // trigger SID selection by performing a dummy write to new device number without flushing it
            self.dummy_write(dev_nr, MIN_CYCLE_SID_WRITE);

            // wait a while to finish the switching of the relay of the UPlay device
            thread::sleep(time::Duration::from_millis(400));
        }
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
        self.clock_adjust.init(sid_clock);
    }

    pub fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool) {
        if self.is_connected() {
            if self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U {
                let physical_dev_nr = self.device_id[dev_nr as usize];
                for i in 0..self.device_id.len() {
                    if self.device_id[i] == physical_dev_nr {
                        let base_reg = self.device_base_reg[i];
                        self.silent_sid(i as i32, base_reg, write_volume);
                        if !self.is_connected() {
                            break;
                        }
                    }
                }
            } else {
                let base_reg = self.device_base_reg[dev_nr as usize];
                self.silent_sid(dev_nr, base_reg, write_volume);
            }

            self.force_flush(dev_nr);
        }
    }

    pub fn silent_active_sids(&mut self, dev_nr: i32, write_volume: bool) {
        if self.is_connected() {
            if self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U {
                for sid_nr in 0..self.number_of_sids as u8 {
                    let mapped_dev_nr = self.device_mappings[sid_nr as usize];
                    let base_reg = self.device_base_reg[mapped_dev_nr as usize];
                    self.silent_sid(dev_nr, base_reg, write_volume);
                    if !self.is_connected() {
                        break;
                    }
                }
            } else {
                let base_reg = self.device_base_reg[dev_nr as usize];
                self.silent_sid(dev_nr, base_reg, write_volume);
            }

            self.force_flush(dev_nr);
        }
    }

    fn silent_sid(&mut self, dev_nr: i32, base_reg: u8, write_volume: bool) {
        if self.number_of_sids > 0 && self.is_connected() {
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x01, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x08, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x07, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0f, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0e, 0);

            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x04, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x05, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x06, 0);

            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0b, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0c, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0d, 0);

            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x12, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x13, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x14, 0);

            if write_volume {
                self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x18, 0);
            }
        }
    }

    pub fn reset_all_sids(&mut self, dev_nr: i32) {
        if self.is_connected() {
            let base_reg = self.device_base_reg[dev_nr as usize];

            if self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U {
                let physical_dev_nr = self.device_id[dev_nr as usize];
                for i in 0..self.device_id.len() {
                    if self.device_id[i] == physical_dev_nr {
                        let base_reg = self.device_base_reg[i];
                        self.reset_sid(i as i32, base_reg);
                        if !self.is_connected() {
                            break;
                        }
                    }
                }
            } else {
                self.reset_sid(dev_nr, base_reg);
            }

            self.write_direct(dev_nr, 40000, base_reg + DUMMY_REG, 0);
            self.force_flush(dev_nr);
        }
    }

    pub fn reset_active_sids(&mut self, dev_nr: i32) {
        if self.is_connected() {
            let base_reg = self.device_base_reg[dev_nr as usize];

            if self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U {
                for sid_nr in 0..self.number_of_sids as u8 {
                    let mapped_dev_nr = self.device_mappings[sid_nr as usize];
                    let base_reg = self.device_base_reg[mapped_dev_nr as usize];

                    self.reset_sid(dev_nr, base_reg);
                    if !self.is_connected() {
                        break;
                    }
                }
            } else {
                self.reset_sid(dev_nr, base_reg);
            }

            self.write_direct(dev_nr, 40000, base_reg + DUMMY_REG, 0);
            self.force_flush(dev_nr);
        }
    }

    fn reset_sid(&mut self, dev_nr: i32, base_reg: u8) {
        if self.number_of_sids > 0 && self.is_connected() {
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x01, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x07, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x08, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0e, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0f, 0);

            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x04, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x0b, 0);
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x12, 0);

            self.reset_sid_register(dev_nr, base_reg + 0x02);
            self.reset_sid_register(dev_nr, base_reg + 0x03);
            self.reset_sid_register(dev_nr, base_reg + 0x04);
            self.reset_sid_register(dev_nr, base_reg + 0x05);
            self.reset_sid_register(dev_nr, base_reg + 0x06);

            self.reset_sid_register(dev_nr, base_reg + 0x09);
            self.reset_sid_register(dev_nr, base_reg + 0x0a);
            self.reset_sid_register(dev_nr, base_reg + 0x0b);
            self.reset_sid_register(dev_nr, base_reg + 0x0c);
            self.reset_sid_register(dev_nr, base_reg + 0x0d);

            self.reset_sid_register(dev_nr, base_reg + 0x10);
            self.reset_sid_register(dev_nr, base_reg + 0x11);
            self.reset_sid_register(dev_nr, base_reg + 0x12);
            self.reset_sid_register(dev_nr, base_reg + 0x13);
            self.reset_sid_register(dev_nr, base_reg + 0x14);

            self.reset_sid_register(dev_nr, base_reg + 0x15);
            self.reset_sid_register(dev_nr, base_reg + 0x16);
            self.reset_sid_register(dev_nr, base_reg + 0x17);
            self.reset_sid_register(dev_nr, base_reg + 0x19);
        }
    }

    #[inline]
    fn reset_sid_register(&mut self, dev_nr: i32, reg: u8) {
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0xff);
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x08);
        let base_reg = reg & 0xe0;
        self.write_direct(dev_nr, 50, base_reg + DUMMY_REG, 0);
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x00);
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];
            self.sid_device.as_mut().unwrap().abort_play(dev_nr as u8);
        }
    }

    pub fn enable_turbo_mode(&mut self) {
        self.turbo_mode = true;
    }

    pub fn disable_turbo_mode(&mut self) {
        self.turbo_mode = false;
    }

    pub fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        if self.is_connected() {
            let base_reg = self.device_base_reg[dev_nr as usize];
            self.write(dev_nr, cycles, base_reg + DUMMY_REG, 0);
        }
    }

    #[inline]
    fn are_multiple_sid_chips_supported(&mut self, dev_nr: i32) -> bool {
        self.device_type[dev_nr as usize] == DEV_TYPE_HS_4U
    }

    pub fn force_flush(&mut self, dev_nr: i32) {
        self.try_flush(dev_nr);
    }

    pub fn set_native_device_clock(&mut self, enabled: bool) {
        self.use_native_device_clock = enabled;
    }

    pub fn get_device_clock(&self) -> SidClock {
        if self.use_native_device_clock {
            SidClock::OneMhz
        } else {
            self.sid_clock
        }
    }

    pub fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        if self.is_connected() {
            let reg = self.map_device_to_reg(dev_nr, reg);
            self.write_direct(dev_nr, cycles, reg, data);
        }
    }

    #[inline]
    fn map_device_to_reg(&mut self, dev_nr: i32, reg: u8) -> u8 {
        let reg = self.filter_reg_for_unsupported_writes(dev_nr, reg);
        let mapped_dev_nr = self.map_reg_to_device(reg);
        let base_reg = self.device_base_reg[mapped_dev_nr as usize];
        (reg & 0x1f) | base_reg
    }

    #[inline]
    fn filter_reg_for_unsupported_writes(&mut self, dev_nr: i32, reg: u8) -> u8 {
        if self.number_of_sids > 1 && !self.are_multiple_sid_chips_supported(dev_nr) && reg >= 0x20 {
            // ignore second SID chip for devices that don't support accessing multiple SID chip simultaneously
            DUMMY_REG
        } else {
            reg
        }
    }

    #[inline]
    fn map_reg_to_device(&mut self, reg: u8) -> i32 {
        let sid_nr = reg >> 5;
        if self.number_of_sids > 1 && sid_nr < self.sid_count as u8 {
            self.device_mappings[sid_nr as usize]
        } else {
            self.device_mappings[0]
        }
    }

    fn write_direct(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        self.create_delay(cycles);
        self.create_write(reg, data);

        while !self.sid_write_fifo.is_empty() {
            let sid_write = self.sid_write_fifo.pop_front().unwrap();
            match sid_write.command {
                DeviceCommand::Delay => self.try_delay_sync(dev_nr, sid_write.cycles),
                DeviceCommand::Write => self.try_write_sync(dev_nr, sid_write.reg, sid_write.data)
            }
        }
    }

    #[inline]
    fn push_write(&mut self, command: DeviceCommand, reg: u8, data: u8, cycles: u16) {
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

    pub fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        if self.is_connected() && self.sid_write_fifo.is_empty() {
            let reg = self.map_device_to_reg(dev_nr, reg);
            self.create_delay(cycles);
            self.create_write(reg, data);
        }
        self.process_write_fifo(dev_nr)
    }

    #[inline]
    fn process_write_fifo(&mut self, dev_nr: i32) -> DeviceResponse {
        while !self.sid_write_fifo.is_empty() {
            let sid_write = self.sid_write_fifo.pop_front().unwrap();

            let device_state = match sid_write.command {
                DeviceCommand::Delay => self.try_delay_async(dev_nr, sid_write.cycles),
                DeviceCommand::Write => self.try_write_async(dev_nr, sid_write.reg, sid_write.data)
            };

            match device_state {
                HSID_USB_STATE_BUSY => {
                    self.sid_write_fifo.push_front(sid_write);
                    thread::yield_now();
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

            thread::yield_now();
        }

        DeviceResponse::Ok
    }

    #[inline]
    fn adjust_frequency(&mut self, reg: u8, data: u8) {
        let reg_offset = reg & 0x1f;

        if reg_offset < 0x10 {
            let voice_nr = reg_offset / 7;
            let base_reg = reg & 0xe0;
            let reg_offset = reg_offset % 7;

            match reg_offset {
                0x00 | 0x01 => self.adjust_frequency_for_voice(voice_nr, base_reg, reg_offset, data),
                _ => self.push_write(DeviceCommand::Write, reg, data, 0)
            }
        } else {
            self.push_write(DeviceCommand::Write, reg, data, 0);
        }
    }

    #[inline]
    fn adjust_frequency_for_voice(&mut self, voice_nr: u8, base_reg: u8, reg: u8, data: u8) {
        if reg <= 1 {
            let voice_index = voice_nr + (base_reg >> 5) * 3;

            self.clock_adjust.update_frequency(voice_index, reg, data);
            let last_freq = self.clock_adjust.get_last_scaled_freq(voice_index);
            let scaled_freq = self.clock_adjust.scale_frequency(voice_index);

            let voice_base = voice_nr * 7;

            let update_hi_freq = last_freq & 0xff00 != scaled_freq & 0xff00;

            if update_hi_freq {
                self.push_write(DeviceCommand::Write, 1 + voice_base + base_reg, (scaled_freq >> 8) as u8, 0);
                self.create_delay(HS_MIN_CYCLE_SID_WRITE);
                self.cycles_to_compensate += HS_MIN_CYCLE_SID_WRITE;
            }
            self.push_write(DeviceCommand::Write, voice_base + base_reg, (scaled_freq & 0xff) as u8, 0);
        }
    }

    #[inline]
    fn try_write_sync(&mut self, dev_nr: i32, reg: u8, data: u8) {
        if self.is_connected() {
            let physical_dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_mut().unwrap().write(physical_dev_nr, reg, data);

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
            self.sid_device.as_mut().unwrap().write(physical_dev_nr, reg, data)
        } else {
            HSID_USB_STATE_OK
        }
    }

    #[inline]
    fn try_flush(&mut self, dev_nr: i32) {
        self.sid_write_fifo.clear();

        if self.is_connected() {
            let physical_dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_mut().unwrap().flush(physical_dev_nr);

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

        if cycles > HS_MIN_CYCLE_SID_WRITE {
            if cycles > self.cycles_to_compensate + HS_MIN_CYCLE_SID_WRITE {
                cycles -= self.cycles_to_compensate;
                self.cycles_to_compensate = 0;
            } else {
                self.cycles_to_compensate -= cycles - HS_MIN_CYCLE_SID_WRITE;
                cycles = HS_MIN_CYCLE_SID_WRITE;
            }
        }

        if cycles > 0xffff {
            if cycles % 0xffff < MINIMUM_CYCLES {
                self.push_write(DeviceCommand::Delay, 0, 0, MINIMUM_CYCLES as u16);
                cycles -= MINIMUM_CYCLES
            }

            while cycles > 0xffff {
                self.push_write(DeviceCommand::Delay, 0, 0, 0xffff);
                cycles -= 0xffff;
            }
        }

        if cycles >= HS_MIN_CYCLE_SID_WRITE {
            self.push_write(DeviceCommand::Delay, 0, 0, cycles as u16);
        } else {
            self.push_write(DeviceCommand::Delay, 0, 0, HS_MIN_CYCLE_SID_WRITE as u16);
            self.cycles_to_compensate += HS_MIN_CYCLE_SID_WRITE - cycles;
        }
    }

    #[inline]
    fn try_delay_sync(&mut self, dev_nr: i32, cycles: u16) {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_mut().unwrap().delay(dev_nr as u8, cycles);

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

            self.sid_device.as_mut().unwrap().delay(dev_nr as u8, cycles)
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
            thread::sleep(time::Duration::from_millis(BUSY_WAIT_MILLIS));
        } else {
            thread::yield_now();
        }

        false
    }

    #[inline]
    fn is_aborted(&self) -> bool {
        let abort_type = self.abort_type.load(Ordering::SeqCst);
        abort_type != ABORT_NO && abort_type != ABORTING
    }
}
