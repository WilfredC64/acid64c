// Copyright (C) 2020 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse};
use super::hardsid_usb_device::{HardsidUsbDevice, HardsidUsbDeviceFacade};
use super::network_sid_device::{NetworkSidDevice, NetworkSidDeviceFacade};

use std::sync::atomic::AtomicI32;
use std::sync::Arc;

pub struct SidDevicesFacade {
    pub devices: SidDevices
}

impl SidDevice for SidDevicesFacade {
    fn disconnect(&mut self, dev_nr: i32) {
        self.devices.disconnect(dev_nr);
    }

    fn is_connected(&mut self, dev_nr: i32) -> bool {
        self.devices.is_connected(dev_nr)
    }

    fn get_last_error(&mut self, dev_nr: i32) -> Option<String> {
        self.devices.get_last_error(dev_nr)
    }

    fn test_connection(&mut self, dev_nr: i32) {
        self.devices.test_connection(dev_nr);
    }

    fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        self.devices.can_pair_devices(dev1, dev2)
    }

    fn get_device_count(&mut self, dev_nr: i32) -> i32 {
        self.devices.get_device_count(dev_nr)
    }

    fn get_device_info(&mut self, dev_nr: i32) -> String {
        self.devices.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, dev_nr: i32, sid_count: i32) {
        self.devices.set_sid_count(dev_nr, sid_count);
    }

    fn set_sid_position(&mut self, dev_nr: i32, sid_position: i8) {
        self.devices.set_sid_position(dev_nr, sid_position);
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.devices.set_sid_model(dev_nr, sid_socket);
    }

    fn set_sid_clock(&mut self, dev_nr: i32, sid_clock: SidClock) {
        self.devices.set_sid_clock(dev_nr, sid_clock);
    }

    fn set_sampling_method(&mut self, dev_nr: i32, sampling_method: SamplingMethod) {
        self.devices.set_sampling_method(dev_nr, sampling_method);
    }

    fn set_sid_header(&mut self, dev_nr: i32, sid_header: Vec<u8>) {
        self.devices.set_sid_header(dev_nr, sid_header);
    }

    fn set_fade_in(&mut self, dev_nr: i32, time_millis: u32) {
        self.devices.set_fade_in(dev_nr, time_millis);
    }

    fn set_fade_out(&mut self, dev_nr: i32, time_millis: u32) {
        self.devices.set_fade_out(dev_nr, time_millis);
    }

    fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool) {
        self.devices.silent_all_sids(dev_nr, write_volume);
    }

    fn silent_sid(&mut self, dev_nr: i32, write_volume: bool) {
        self.devices.silent_sid(dev_nr, write_volume);
    }

    fn device_reset(&mut self, dev_nr: i32) {
        self.devices.device_reset(dev_nr);
    }

    fn reset_all_sids(&mut self, dev_nr: i32) {
        self.devices.reset_all_sids(dev_nr);
    }

    fn reset_sid(&mut self, dev_nr: i32) {
        self.devices.reset_sid(dev_nr);
    }

    fn reset_all_buffers(&mut self, dev_nr: i32) {
        self.devices.reset_all_buffers(dev_nr);
    }

    fn enable_turbo_mode(&mut self, dev_nr: i32) {
        self.devices.enable_turbo_mode(dev_nr);
    }

    fn disable_turbo_mode(&mut self, dev_nr: i32) {
        self.devices.disable_turbo_mode(dev_nr);
    }

    fn dummy_write(&mut self, dev_nr: i32, cycles_input: u32) {
        self.devices.dummy_write(dev_nr, cycles_input);
    }

    fn write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) {
        self.devices.write(dev_nr, cycles_input, reg, data);
    }

    fn try_write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) -> DeviceResponse {
        self.devices.try_write(dev_nr, cycles_input, reg, data)
    }

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        self.devices.retry_write(dev_nr)
    }

    fn force_flush(&mut self, dev_nr: i32) {
        self.devices.force_flush(dev_nr);
    }

    fn set_native_device_clock(&mut self, enabled: bool) {
        self.devices.set_native_device_clock(enabled);
    }

    fn get_device_clock(&mut self, dev_nr: i32) -> SidClock {
        self.devices.get_device_clock(dev_nr)
    }
}

pub struct SidDevices {
    sid_devices: Vec<Box<dyn SidDevice + Send>>,
    device_count: i32,
    device_name: Vec<String>,
    device_mapping_id: Vec<u8>,
    device_sid_count: Vec<u8>,
    device_offset: Vec<u8>,
    abort_type: Arc<AtomicI32>,
    use_native_device_clock: bool,
}

#[allow(dead_code)]
impl SidDevices {
    pub fn new(abort_type: Arc<AtomicI32>) -> SidDevices {
        SidDevices {
            sid_devices: vec![],
            device_count: 0,
            device_name: vec![],
            device_mapping_id: vec![],
            device_sid_count: vec![],
            device_offset: vec![],
            abort_type,
            use_native_device_clock: true,
        }
    }

    pub fn connect(&mut self, ip_address: &str, port: &str) -> Result<(), String> {
        let hs_connect_result = self.try_connect_hardsid_device();
        let ns_connect_result = self.try_connect_network_device(ip_address, port);

        if self.sid_devices.len() == 0 {
            Err(hs_connect_result.err().unwrap_or("".to_string()) + " | "
                + &ns_connect_result.err().unwrap_or("".to_string()))
        } else {
            self.set_native_device_clock(self.use_native_device_clock);
            Ok(())
        }
    }

    fn try_connect_hardsid_device(&mut self) -> Result<(), String> {
        let mut hs_device = HardsidUsbDevice::new(Arc::clone(&self.abort_type));
        let hs_connect_result = hs_device.connect();
        if hs_connect_result.is_ok() {
            let sid_count = hs_device.get_device_count();
            let hs_facade = HardsidUsbDeviceFacade { hs_device };
            self.sid_devices.push(Box::new(hs_facade));
            self.device_sid_count.push(sid_count as u8);

            self.retrieve_device_info(self.sid_devices.len() - 1);
            Ok(())
        } else {
            Err(hs_connect_result.err().unwrap())
        }
    }

    fn try_connect_network_device(&mut self, ip_address: &str, port: &str) -> Result<(), String> {
        let mut ns_device = NetworkSidDevice::new(Arc::clone(&self.abort_type));
        let ns_connect_result = ns_device.connect(ip_address, port);
        if ns_connect_result.is_ok() {
            let sid_count = ns_device.get_device_count();
            let ns_facade = NetworkSidDeviceFacade { ns_device };
            self.sid_devices.push(Box::new(ns_facade));
            self.device_sid_count.push(sid_count as u8);

            self.retrieve_device_info(self.sid_devices.len() - 1);
            Ok(())
        } else {
            Err(ns_connect_result.err().unwrap())
        }
    }

    fn retrieve_device_info(&mut self, dev_nr: usize) {
        let device_count = self.sid_devices[dev_nr].get_device_count(0);

        for i in 0..device_count {
            self.device_name.push(self.sid_devices[dev_nr].get_device_info(i));
            self.device_mapping_id.push(dev_nr as u8);
            self.device_offset.push(i as u8);
        }

        self.device_count += device_count;
    }

    #[inline]
    fn map_device(&mut self, dev_nr: i32) -> u8 {
        self.device_mapping_id[dev_nr as usize]
    }

    #[inline]
    fn map_sid_offset(&mut self, dev_nr: i32) -> u8 {
        self.device_offset[dev_nr as usize]
    }

    pub fn disconnect(&mut self, dev_nr: i32) {
        if dev_nr < self.device_count {
            let mapped_dev_nr = self.map_device(dev_nr) as usize;
            self.disconnect_device(mapped_dev_nr);
        }
    }

    fn disconnect_device(&mut self, dev_nr: usize) {
        let device_count = self.device_sid_count[dev_nr];
        self.sid_devices[dev_nr].disconnect(0);
        self.sid_devices.remove(dev_nr);
        self.device_sid_count.remove(dev_nr);

        for (i, &device_id) in self.device_mapping_id.iter().enumerate().rev() {
            if device_id == dev_nr as u8 {
                self.device_name.remove(i);
                self.device_offset.remove(i);
            }
        }

        self.device_mapping_id = self.device_mapping_id.iter()
            .filter(|&&index| index != dev_nr as u8 )
            .map(|&index| {
                if index > dev_nr as u8 {
                    index - 1
                } else {
                    index
                }
            }).collect();

        self.device_count -= device_count as i32;
    }

    pub fn is_connected(&mut self, dev_nr: i32) -> bool {
        if dev_nr == -1 {
            for i in 0..self.sid_devices.len() {
                let connected = self.sid_devices[i].is_connected(0);
                if !connected {
                    return false;
                }
            }
            return true;
        }

        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].is_connected(mapped_sid_nr as i32)
    }

    pub fn get_last_error(&mut self, dev_nr: i32) -> Option<String> {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].get_last_error(mapped_sid_nr as i32)
    }

    pub fn test_connection(&mut self, dev_nr: i32) {
        if dev_nr == -1 {
            for i in (0..self.sid_devices.len()).rev() {
                self.sid_devices[i].test_connection(0);
                if !self.sid_devices[i].is_connected(0) {
                    self.disconnect_device(i);
                }
            }
        } else {
            let mapped_dev_nr = self.map_device(dev_nr);
            let mapped_sid_nr = self.map_sid_offset(dev_nr);
            self.sid_devices[mapped_dev_nr as usize].test_connection(mapped_sid_nr as i32);
            if !self.sid_devices[mapped_dev_nr as usize].is_connected(0) {
                self.disconnect_device(mapped_dev_nr as usize);
            }
        }
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        let mapped_dev1 = self.map_device(dev1);
        let mapped_dev2 = self.map_device(dev2);

        if mapped_dev1 != mapped_dev2 {
            false
        } else {
            let mapped_sid_nr1 = self.map_sid_offset(dev1);
            let mapped_sid_nr2 = self.map_sid_offset(dev2);
            self.sid_devices[mapped_dev1 as usize].can_pair_devices(mapped_sid_nr1 as i32, mapped_sid_nr2 as i32)
        }
    }

    pub fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.device_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> String {
        self.device_name[dev_nr as usize].clone()
    }

    pub fn set_sid_count(&mut self, dev_nr: i32, sid_count: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sid_count(mapped_sid_nr as i32, sid_count);
    }

    pub fn set_sid_position(&mut self, dev_nr: i32, sid_position: i8) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sid_position(mapped_sid_nr as i32, sid_position);
    }

    pub fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sid_model(mapped_sid_nr as i32, sid_socket);
    }

    pub fn set_sid_clock(&mut self, dev_nr: i32, sid_clock: SidClock) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sid_clock(mapped_sid_nr as i32, sid_clock);
    }

    pub fn set_sampling_method(&mut self, dev_nr: i32, sampling_method: SamplingMethod) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sampling_method(mapped_sid_nr as i32, sampling_method);
    }

    pub fn set_sid_header(&mut self, dev_nr: i32, sid_header: Vec<u8>) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_sid_header(mapped_sid_nr as i32, sid_header);
    }

    pub fn set_fade_in(&mut self, dev_nr: i32, time_millis: u32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_fade_in(mapped_sid_nr as i32, time_millis);
    }

    pub fn set_fade_out(&mut self, dev_nr: i32, time_millis: u32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].set_fade_out(mapped_sid_nr as i32, time_millis);
    }

    pub fn silent_all_sids(&mut self, dev_nr: i32, write_volume: bool) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].silent_all_sids(mapped_sid_nr as i32, write_volume);
    }

    pub fn silent_sid(&mut self, dev_nr: i32, write_volume: bool) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].silent_sid(mapped_sid_nr as i32, write_volume);
    }

    pub fn device_reset(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].device_reset(mapped_sid_nr as i32);
    }

    pub fn reset_all_sids(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].reset_all_sids(mapped_sid_nr as i32);
    }

    pub fn reset_sid(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].reset_sid(mapped_sid_nr as i32);
    }

    pub fn reset_all_buffers(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].reset_all_buffers(mapped_sid_nr as i32);
    }

    pub fn enable_turbo_mode(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].enable_turbo_mode(mapped_sid_nr as i32);
    }

    pub fn disable_turbo_mode(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].disable_turbo_mode(mapped_sid_nr as i32);
    }

    pub fn dummy_write(&mut self, dev_nr: i32, cycles_input: u32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].dummy_write(mapped_sid_nr as i32, cycles_input);
    }

    pub fn write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].write(mapped_sid_nr as i32, cycles_input, reg, data);
    }

    fn try_write(&mut self, dev_nr: i32, cycles_input: u32, reg: u8, data: u8) -> DeviceResponse {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].try_write(mapped_sid_nr as i32, cycles_input, reg, data)
    }

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].retry_write(mapped_sid_nr as i32)
    }

    pub fn force_flush(&mut self, dev_nr: i32) {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].force_flush(mapped_sid_nr as i32);
    }

    pub fn set_native_device_clock(&mut self, enabled: bool) {
        self.use_native_device_clock = enabled;
        for i in 0..self.sid_devices.len() {
            self.sid_devices[i].set_native_device_clock(enabled);
        }
    }

    pub fn get_device_clock(&mut self, dev_nr: i32) -> SidClock {
        let mapped_dev_nr = self.map_device(dev_nr);
        let mapped_sid_nr = self.map_sid_offset(dev_nr);
        self.sid_devices[mapped_dev_nr as usize].get_device_clock(mapped_sid_nr as i32)
    }
}
