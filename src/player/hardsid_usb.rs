// Copyright (C) 2020 - 2021 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

#![allow(dead_code)]
use libloading::{Library, Symbol};

use std::ffi::CStr;

pub const HSID_USB_STATE_OK: HsidUsbState = 1;
pub const HSID_USB_STATE_BUSY: HsidUsbState = 2;
pub const HSID_USB_STATE_ERROR: HsidUsbState = 3;
pub type HsidUsbState = u8;

pub const DEV_TYPE_HS_4U: HsidDevType = 1;     // HardSID 4U device
pub const DEV_TYPE_HS_UPLAY: HsidDevType = 2;  // HardSID UPlay device
pub const DEV_TYPE_HS_UNO: HsidDevType = 3;    // HardSID Uno device
pub type HsidDevType = u8;

pub const SYS_MODE_SIDPLAY: HsidSysMode = 1;
pub const SYS_MODE_VST: HsidSysMode = 2;
pub type HsidSysMode = u16;

pub struct HardSidUsb {
    hs_usb_lib: Library
}

impl HardSidUsb {
    fn new(hs_usb_lib: Library) -> HardSidUsb {
        HardSidUsb {
            hs_usb_lib
        }
    }

    /// loads the library
    pub fn load() -> Result<HardSidUsb, String> {
        let hs_usb_lib = unsafe { Library::new("hardsid_usb") };
        if hs_usb_lib.is_err() {
            return Err("hardsid_usb library could not be loaded.".to_string());
        }
        Ok(HardSidUsb::new(hs_usb_lib.unwrap()))
    }

    /// initializes the library
    pub fn init(&self, sync: bool, sys_mode: HsidSysMode) -> bool {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_init").unwrap() as Symbol<unsafe extern "C" fn(i32, u16) -> bool>)(sync as i32, sys_mode)
        }
    }

    /// initializes the library in sync mode and Sidplay mode
    pub fn init_sidplay_mode(&self) -> bool {
        unsafe {
            const SYNC_MODE: bool = true;
            (self.hs_usb_lib.get(b"hardsid_usb_init").unwrap() as Symbol<unsafe extern "C" fn(i32, u16) -> bool>)(SYNC_MODE as i32, SYS_MODE_SIDPLAY)
        }
    }

    /// closes the library
    pub fn close(&self) {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_close").unwrap() as Symbol<unsafe extern "C" fn()>)()
        }
    }

    /// returns the number of active USB HardSID devices
    pub fn get_dev_count(&self) -> u8 {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_getdevcount").unwrap() as Symbol<unsafe extern "C" fn() -> u8>)()
        }
    }

    /// returns the device type of the given device
    pub fn get_device_type(&self, dev_id: u8) -> HsidDevType {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_getdevicetype").unwrap() as Symbol<unsafe extern "C" fn(u8) -> HsidDevType>)(dev_id)
        }
    }

    /// returns the number of detected SID chips on the given device
    pub fn get_sid_count(&self, dev_id: u8) -> u8 {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_getsidcount").unwrap() as Symbol<unsafe extern "C" fn(u8) -> u8>)(dev_id)
        }
    }

    /// schedules a write command
    pub fn write(&self, dev_id: u8, reg: u8, data: u8) -> HsidUsbState {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_write").unwrap() as Symbol<unsafe extern "C" fn(u8, u8, u8) -> HsidUsbState>)(dev_id, reg, data)
        }
    }

    /// flushes the software buffer to the hardware
    pub fn flush(&self, dev_id: u8) -> HsidUsbState {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_flush").unwrap() as Symbol<unsafe extern "C" fn(u8) -> HsidUsbState>)(dev_id)
        }
    }

    /// schedules a delay command
    pub fn delay(&self, dev_id: u8, cycles: u16) -> HsidUsbState {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_delay").unwrap() as Symbol<unsafe extern "C" fn(u8, u16) -> HsidUsbState>)(dev_id, cycles)
        }
    }

    /// aborts the playback ASAP, only use in sync mode
    pub fn abort_play(&self, dev_id: u8) {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_usb_abortplay").unwrap() as Symbol<unsafe extern "C" fn(u8)>)(dev_id)
        }
    }

    /// passes a buffer to the kernel driver - async mode only
    pub fn write_buffer(&self, buffer: &[u8]) -> HsidUsbState {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_write_buff").unwrap() as Symbol<unsafe extern "C" fn(*const u8, i32) -> HsidUsbState>)(buffer.as_ptr(), buffer.len() as i32)
        }
    }

    /// queries driver state variables (such as errorpacketcount)
    pub fn query_status(&self, dev_id: u8) -> u32 {
        unsafe {
            (self.hs_usb_lib.get(b"hardsid_query_status").unwrap() as Symbol<unsafe extern "C" fn(u8) -> u32>)(dev_id)
        }
    }

    /// gets the last error which can be used when init fails
    pub fn get_last_error(&self) -> Option<String> {
        unsafe {
            let error_msg = (self.hs_usb_lib.get(b"hardsid_usb_getlasterror").unwrap() as Symbol<unsafe extern "C" fn() -> *const i8>)();
            Self::convert_pchar_to_ansi_string(error_msg)
        }
    }

    unsafe fn convert_pchar_to_ansi_string(text: *const i8) -> Option<String> {
        if text.is_null() {
            None
        } else {
            Some(CStr::from_ptr(text).to_string_lossy().to_string())
        }
    }
}
