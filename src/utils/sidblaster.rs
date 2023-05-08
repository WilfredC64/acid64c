// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::time::Duration;
use libftd2xx::{BitsPerWord, Ftdi, FtdiCommon, FtStatus, list_devices, Parity, StopBits};

const BAUD_RATE: u32 = 500_000;
const LATENCY_IN_MILLIS: u64 = 2;
const TIME_OUT_IN_MILLIS: u64 = 1000;
const ERROR_MSG_DEVICE_FAILURE: &str = "Failed to communicate with FTDI device.";
pub const ERROR_MSG_NO_SIDBLASTER_FOUND: &str = "No SIDBlaster USB device found.";

pub fn detect_devices() -> Result<Vec<(String, String)>, String> {
    let serials = get_serials()?;
    get_device_names(&serials)
}

pub fn get_devices() -> Result<Vec<Ftdi>, String> {
    get_serials()?.iter().map(|serial| {
        let mut usb_device = Ftdi::with_serial_number(serial).map_err(|_| ERROR_MSG_DEVICE_FAILURE.to_string())?;
        configure_device(&mut usb_device).map_err(|_| ERROR_MSG_DEVICE_FAILURE.to_string())?;
        Ok(usb_device)
    }).collect::<Result<Vec<_>, _>>()
}

pub fn silent_sids(sid_devices: &mut [Ftdi]) {
    for sid_device in &mut sid_devices.iter_mut() {
        let _ = &sid_device.write(&[0xf8, 0x00, 0xe1, 0x00, 0xe0, 0x00, 0xe8, 0x00, 0xe7, 0x00, 0xef, 0x00, 0xee, 0x00]);
    }
}

pub fn write(sid_device: &mut Ftdi, data: &[u8]) -> Result<usize, FtStatus> {
    sid_device.write(data)
}

fn get_serials() -> Result<Vec<String>, String> {
    let mut devices = list_devices().map_err(|_| ERROR_MSG_DEVICE_FAILURE.to_string())?;
    devices.sort_unstable_by_key(|device| device.description.to_owned() + &device.serial_number);

    Ok(devices.iter()
        .filter(|device| device.vendor_id == 0x0403 && device.description.starts_with("SIDBlaster/USB"))
        .map(|device| device.serial_number.clone())
        .collect::<Vec<_>>())
}

fn get_device_names(serials: &[String]) -> Result<Vec<(String, String)>, String> {
    serials.iter().map(|serial| {
        let mut usb_device = Ftdi::with_serial_number(serial).map_err(|_| ERROR_MSG_DEVICE_FAILURE.to_string())?;
        let device_info = usb_device.device_info().map_err(|_| ERROR_MSG_DEVICE_FAILURE.to_string())?;
        Ok((serial.to_owned(), device_info.description.replace("/USB", "").replace('/', " ").trim().to_string()))
    }).collect()
}

fn configure_device(usb_device: &mut Ftdi) -> Result<(), FtStatus> {
    usb_device.set_baud_rate(BAUD_RATE)?;
    usb_device.set_data_characteristics(BitsPerWord::Bits8, StopBits::Bits1, Parity::No)?;
    usb_device.set_break_off()?;
    usb_device.set_flow_control_xon_xoff(0, 0)?;
    usb_device.set_latency_timer(Duration::from_millis(LATENCY_IN_MILLIS))?;
    usb_device.set_timeouts(Duration::from_millis(TIME_OUT_IN_MILLIS), Duration::from_millis(TIME_OUT_IN_MILLIS))
}
