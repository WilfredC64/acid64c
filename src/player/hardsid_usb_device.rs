// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::sid_device::{SidDevice, SidClock, SamplingMethod};
use super::hardsid_usb::{HardSidUsb, HSID_USB_STATE_ERROR, HSID_USB_STATE_BUSY, DEV_TYPE_HS_4U, DEV_TYPE_HS_UPLAY, DEV_TYPE_HS_UNO};

use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, thread, time};

const MAX_NUMBER_OF_SID_CHIPS: usize = 8;
const HSID_BUSY_WAIT_MS: u64 = 5;
const HSID_ABORT_PLAY_WAIT_MS: u64 = 20;

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

    fn test_connection(&mut self, _dev_nr: i32) {
        self.hs_device.test_connection();
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

    fn set_sid_position(&mut self, _dev_nr: i32, sid_position: i8) {
        self.hs_device.set_sid_position(sid_position);
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.hs_device.set_sid_model(dev_nr, sid_socket);
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.hs_device.set_sid_clock(sid_clock);
    }

    fn set_sampling_method(&mut self, _dev_nr: i32, sampling_method: SamplingMethod) {
        self.hs_device.set_sampling_method(sampling_method);
    }

    fn set_sid_header(&mut self, _dev_nr: i32, sid_header: Vec<u8>) {
        self.hs_device.set_sid_header(sid_header);
    }

    fn set_fade_in(&mut self, _dev_nr: i32, time_millis: u32) {
        self.hs_device.set_fade_in(time_millis);
    }

    fn set_fade_out(&mut self, _dev_nr: i32, time_millis: u32) {
        self.hs_device.set_fade_out(time_millis);
    }

    fn device_reset(&mut self, dev_nr: i32) {
        self.hs_device.device_reset(dev_nr);
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

    fn force_flush(&mut self, dev_nr: i32) {
        self.hs_device.force_flush(dev_nr);
    }
}

pub struct HardsidUsbDevice {
    sid_device: Option<HardSidUsb>,
    device_count: i32,
    number_of_sids: i32,
    sid_clock: SidClock,
    turbo_mode: bool,
    device_type: Vec<u8>,
    device_id: Vec<u8>,
    device_base_reg: Vec<u8>,
    aborted: Arc<AtomicBool>,
    last_error: Option<String>,
    device_mappings: [i32; MAX_NUMBER_OF_SID_CHIPS],
}

#[allow(dead_code)]
impl HardsidUsbDevice {
    pub fn new(aborted: Arc<AtomicBool>) -> HardsidUsbDevice {
        HardsidUsbDevice {
            sid_device: None,
            device_count: 0,
            number_of_sids: 0,
            sid_clock: SidClock::PAL,
            turbo_mode: false,
            device_type: vec![],
            device_id: vec![],
            device_base_reg: vec![],
            aborted,
            last_error: None,
            device_mappings: [0; MAX_NUMBER_OF_SID_CHIPS],
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.disconnect();
        self.last_error = None;

        let usb_device = HardSidUsb::new();
        let init_response = usb_device.init();
        if !init_response {
            Err(format!("Initializing HardSID USB device failed."))
        } else {
            let dev_count = usb_device.get_dev_count();

            if dev_count > 0 {
                for i in 0..dev_count {
                    let dev_type = usb_device.get_device_type(i);
                    let dev_sid_count = usb_device.get_sid_count(i);
                    for j in 0..dev_sid_count {
                        self.device_type.push(dev_type);
                        self.device_id.push(i);
                        self.device_base_reg.push(j * 0x20);
                        self.device_mappings[j as usize] = j as i32;
                    }
                }

                self.device_count = self.device_id.len() as i32;

                self.sid_device = Some(usb_device);
                Ok(())
            } else {
                Err(format!("No HardSID USB device found."))
            }
        }
    }

    pub fn disconnect(&mut self) {
        if self.sid_device.is_some() {
            self.sid_device.as_ref().unwrap().close();
            self.sid_device = None;
        }
        self.init_to_default();
    }

    #[inline]
    fn init_to_default(&mut self) {
        self.device_count = 0;
        self.number_of_sids = 0;
        self.sid_clock = SidClock::PAL;
    }

    pub fn disconnect_with_error(&mut self, error_message: String) {
        self.last_error = Some(error_message);
        self.disconnect();
    }

    pub fn get_last_error(&mut self) -> Option<String> {
        self.last_error.clone()
    }

    pub fn is_connected(&mut self) -> bool {
        self.sid_device.is_some()
    }

    pub fn test_connection(&mut self) {
        if self.is_connected() {
            let dev_count = self.sid_device.as_ref().unwrap().get_dev_count();
            if dev_count as i32 != self.device_count {
                self.disconnect_with_error("Number of devices is changed.".to_string());
            }
        }
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        dev1 != dev2 &&
            self.device_id[dev1 as usize] == self.device_id[dev2 as usize] &&
            self.device_type[dev1 as usize] == DEV_TYPE_HS_4U
    }

    pub fn get_device_count(&mut self) -> i32 {
        self.device_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> String {
        let dev_name = match self.device_type[dev_nr as usize] {
            DEV_TYPE_HS_4U => "HardSID 4U ",
            DEV_TYPE_HS_UPLAY => "HS UPlay ",
            DEV_TYPE_HS_UNO => "HardSID Uno ",
            _ => "Unknown HS "
        };
        dev_name.to_string() + &*(dev_nr + 1).to_string()
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;
    }

    pub fn set_sid_position(&mut self, _sid_position: i8) {
        // not supported
    }

    pub fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.device_mappings[sid_socket as usize] = dev_nr;
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
    }

    pub fn set_sampling_method(&mut self, _sampling_method: SamplingMethod) {
        // not supported
    }

    pub fn set_sid_header(&mut self, _sid_header: Vec<u8>) {
        // not supported
    }

    pub fn set_fade_in(&mut self, _time_millis: u32) {
        // not supported
    }

    pub fn set_fade_out(&mut self, _time_millis: u32) {
        // not supported
    }

    pub fn device_reset(&mut self, dev_nr: i32) {
        self.reset_all_buffers(dev_nr);
        self.reset_sid(dev_nr);
    }

    pub fn reset_all_sids(&mut self) {
        for i in 0..self.device_id.len() {
            self.reset_sid(i as i32);
        }
    }

    pub fn reset_sid(&mut self, dev_nr: i32) {
        if self.number_of_sids > 0 {
            let reg_base = self.device_base_reg[dev_nr as usize];

            self.write(dev_nr, 8, reg_base + 0x04, 0);
            self.write(dev_nr, 8, reg_base + 0x0b, 0);
            self.write(dev_nr, 8, reg_base + 0x12, 0);

            self.write(dev_nr, 8, reg_base + 0x00, 0);
            self.write(dev_nr, 8, reg_base + 0x01, 0);
            self.write(dev_nr, 8, reg_base + 0x07, 0);
            self.write(dev_nr, 8, reg_base + 0x08, 0);
            self.write(dev_nr, 8, reg_base + 0x0e, 0);
            self.write(dev_nr, 8, reg_base + 0x0f, 0);

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
        self.write(dev_nr, 8, reg, 0xff);
        self.write(dev_nr, 8, reg, 0x08);
        self.dummy_write(dev_nr, 50);
        self.write(dev_nr, 8, reg, 0x00);
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        if self.is_connected() {
            self.sid_device.as_ref().unwrap().abort_play(dev_nr as u8);

            // wait a few milliseconds to get it processed
            thread::sleep(time::Duration::from_millis(HSID_ABORT_PLAY_WAIT_MS));
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

    pub fn write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) {
        let reg = self.map_to_supported_device(dev_nr, reg);

        let cycles = if cycles_input > 0xffff {
            self.delay(dev_nr, cycles_input, 0x100)
        } else {
            cycles_input
        };

        self.delay(dev_nr, cycles, 0);
        self.try_write(dev_nr, reg, data);
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
    fn try_write(&mut self, dev_nr: i32, reg: u8, data: u8) -> bool {
        if self.is_connected() {
            let physical_dev_nr = self.device_id[dev_nr as usize];

            let (dev_nr, reg) = self.convert_device_info(reg);
            let base_reg = self.device_base_reg[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().write(physical_dev_nr as u8, reg | base_reg, data);

                if state == HSID_USB_STATE_ERROR {
                    self.disconnect_with_error("Failure occurred during write to device.".to_string());
                    return false;
                }

                if state != HSID_USB_STATE_BUSY || self.is_aborted() {
                    return true;
                }

                thread::sleep(time::Duration::from_millis(HSID_BUSY_WAIT_MS));
            }
        }
        false
    }

    #[inline]
    fn try_flush(&mut self, dev_nr: i32) -> bool {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().flush(dev_nr as u8);

                if state == HSID_USB_STATE_ERROR {
                    self.disconnect_with_error("Failure occurred during write to device.".to_string());
                    return false;
                }

                if state != HSID_USB_STATE_BUSY || self.is_aborted() {
                    return true;
                }

                thread::sleep(time::Duration::from_millis(HSID_BUSY_WAIT_MS));
            }
        }
        false
    }

    #[inline]
    fn delay(&mut self, dev_nr: i32, cycles: u32, minimum_cycles_to_remain: u32) -> u32 {
        let mut cycles = cycles - minimum_cycles_to_remain;
        while cycles > 0xffff {
            self.try_delay(dev_nr, 0xffff);
            cycles -= 0xffff;
        }

        if cycles > 0 {
            self.try_delay(dev_nr, cycles as u16);
            cycles = 0;
        }

        minimum_cycles_to_remain + cycles
    }

    #[inline]
    fn try_delay(&mut self, dev_nr: i32, cycles: u16) -> bool {
        if self.is_connected() {
            let dev_nr = self.device_id[dev_nr as usize];

            loop {
                let state = self.sid_device.as_ref().unwrap().delay(dev_nr as u8, cycles);

                if state == HSID_USB_STATE_ERROR {
                    self.disconnect_with_error("Failure occurred during write to device.".to_string());
                    return false;
                }

                if state != HSID_USB_STATE_BUSY || self.is_aborted() {
                    return true;
                }

                thread::sleep(time::Duration::from_millis(HSID_BUSY_WAIT_MS));
            }
        }
        false
    }

    #[inline]
    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }
}
