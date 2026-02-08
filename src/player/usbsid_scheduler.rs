// Copyright (C) 2025 - 2026 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::{cmp, thread, time::Duration};
use crossbeam_channel::{Receiver};
use ringbuf::{SharedRb, CachingCons};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Observer};
use rusb::{Device, Direction, Error, GlobalContext, Recipient, RequestType, request_type, TransferType, DeviceHandle};
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::player::sid_device::{DeviceInfo, SidModel, SidWrite};
use crate::utils::{armsid, armsid::SidFilter, fpgasid, mossid};

pub const USBSID_DEVICE_NAME: &str = "USBSID-Pico";
const ERROR_CONNECTING_DEVICE: &str = "Error connecting to USBSID Device.";
const ERROR_STARTING_SCHEDULER: &str = "Error starting USBSID Scheduler.";

const USBSID_VENDOR: u16 = 0xCAFE;
const USBSID_PRODUCT_ID: u16 = 0x4011;
const TIMEOUT_IN_MILLIS: u64 = 500;
const BUFFER_EMPTY_DELAY_IN_MILLIS: u64 = 5;

const EP_OUT_ADDR: u8 = 0x02;
const EP_IN_ADDR: u8 = 0x82;

const ACM_CTRL_DTR: u16 = 0x01;
const ACM_CTRL_RTS: u16 = 0x02;
const ACM_SET_LINE_CODING: u8 = 0x20;
const ACM_SET_CONTROL_LINE_STATE: u8 = 0x22;

const ENCODING: [u8; 7] = [0x40, 0x54, 0x89, 0x00, 0x00, 0x00, 0x08];

const USB_BUFFER_SIZE: usize = 64;
const MAX_SID_WRITES: usize = (USB_BUFFER_SIZE - 1) / 4;
const MAX_BULK_WRITE_SIZE: usize = MAX_SID_WRITES * 4 + 1;

const COMMAND: u8 = 0x03;
const CONFIG: u8 = 0x12;
const CYCLED_WRITE: u8 = 0x02;

const CMD_GET_NUM_SIDS: u8 = 0x39;
const CMD_GET_PCB_VERSION: u8 = 0x81;
const CMD_SET_CLOCK: u8 = 0x50;
const CMD_SET_STEREO: u8 = 0x89;

pub enum UsbSidCommand {
    ClearBuffer,
    MuteAll,
    Reset,
    ResetAll,
    SetClock,
    SetDevice,
    SetModel,
}

#[allow(dead_code)]
pub enum UsbSidOutput {
    Mono = 0,
    Stereo = 1,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SidClock {
    Pal = 1,
    Ntsc = 2,
}

#[derive(Default)]
pub struct UsbSidConfig {
    pub devices: Vec<DeviceInfo>,
}

pub struct UsbSidScheduler {
    queue: Option<CachingCons<Arc<SharedRb<Heap<SidWrite>>>>>,
    sid_writer_thread: Option<thread::JoinHandle<()>>,
    aborted: Arc<AtomicBool>,

    dev_handles: Option<Vec<DeviceHandle<GlobalContext>>>,
    cycles_in_buffer: Arc<AtomicU32>,
}

impl Drop for UsbSidScheduler {
    fn drop(&mut self) {
        self.stop_sid_writer_thread();
    }
}

impl UsbSidScheduler {
    pub fn new(
        queue: Option<CachingCons<Arc<SharedRb<Heap<SidWrite>>>>>,
        aborted: Arc<AtomicBool>,
        cycles_in_buffer: Arc<AtomicU32>
    ) -> Self {
        Self {
            queue,
            sid_writer_thread: None,
            aborted,
            dev_handles: Some(vec![]),
            cycles_in_buffer
        }
    }

    fn stop_sid_writer_thread(&mut self) {
        self.aborted.store(true, Ordering::SeqCst);

        if self.sid_writer_thread.is_some() {
            let _ = self.sid_writer_thread.take().unwrap().join();
        }
    }

    pub fn start(&mut self, cmd_receiver: Receiver<(UsbSidCommand, i32)>) -> Result<UsbSidConfig, String> {
        self.stop_sid_writer_thread();
        self.aborted.store(false, Ordering::SeqCst);

        let usbsid_config = self.detect_devices().map_err(|error| format!("{}: {error}", ERROR_CONNECTING_DEVICE))?;
        if usbsid_config.devices.is_empty() {
            return Ok(usbsid_config);
        }

        let mut queue = self.queue.take().ok_or(ERROR_STARTING_SCHEDULER.to_string())?;
        let handles = self.dev_handles.take().ok_or(ERROR_STARTING_SCHEDULER.to_string())?;

        let cycles_in_buffer = self.cycles_in_buffer.clone();
        let devices = usbsid_config.devices.clone();
        let aborted = self.aborted.clone();

        self.sid_writer_thread = Some(thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            if Self::run_usbsid_writer_loop(&handles, &devices, &mut queue, &cmd_receiver, &cycles_in_buffer, &aborted).is_err() {
                aborted.store(true, Ordering::SeqCst);
            }

            for _ in cmd_receiver.try_iter() {}; // drain all pending requests

            cycles_in_buffer.store(0, Ordering::Relaxed);
            queue.clear();
            Self::cleanup_handles(&handles);
        }));

        Ok(usbsid_config)
    }

    fn run_usbsid_writer_loop(
        handles: &[DeviceHandle<GlobalContext>],
        devices: &[DeviceInfo],
        queue: &mut CachingCons<Arc<SharedRb<Heap<SidWrite>>>>,
        cmd_receiver: &Receiver<(UsbSidCommand, i32)>,
        cycles_in_buffer: &Arc<AtomicU32>,
        aborted: &Arc<AtomicBool>
    ) -> Result<(), Error> {
        let mut device_index = 0;
        Self::config_sids(&handles[device_index], devices[device_index].socket_count)?;

        let mut write_buffer = [SidWrite::default(); MAX_SID_WRITES];
        let mut byte_buffer = [0u8; MAX_SID_WRITES * 4];

        while !Self::is_aborted(aborted) {
            while let Ok((command, param)) = cmd_receiver.try_recv() {
                match command {
                    UsbSidCommand::ClearBuffer => {
                        cycles_in_buffer.store(0, Ordering::Relaxed);
                        queue.clear();
                    }
                    UsbSidCommand::MuteAll => {
                        Self::mute_sids(&handles[device_index], devices[device_index].socket_count)?;
                    }
                    UsbSidCommand::SetDevice => {
                        device_index = param as usize;
                        Self::config_sids(&handles[device_index], devices[device_index].socket_count)?;
                    }
                    UsbSidCommand::SetClock => {
                        let clock_type = if param == 0 { SidClock::Pal } else { SidClock::Ntsc };
                        Self::set_clock(&handles[device_index], clock_type)?;
                    }
                    UsbSidCommand::SetModel => {
                        let sid_model = if param == 0 { SidModel::Mos6581 } else { SidModel::Mos8580 };
                        Self::set_sid_model_for_all_sids(&handles[device_index], devices[device_index].socket_count, &sid_model)?;
                    }
                    UsbSidCommand::Reset => {
                        Self::reset_active_sids(&handles[device_index], param as u8)?;
                    }
                    UsbSidCommand::ResetAll => {
                        Self::reset_all_sids(&handles[device_index], devices[device_index].socket_count)?;
                    }
                }
                thread::yield_now();
            }

            if queue.is_empty() {
                thread::sleep(Duration::from_millis(BUFFER_EMPTY_DELAY_IN_MILLIS));
                continue;
            }

            let count = queue.pop_slice(&mut write_buffer);

            let mut total_cycles: u32 = 0;
            for (chunk, sid_write) in byte_buffer.chunks_exact_mut(4).zip(&write_buffer[..count]) {
                // subtract 1 from cycles since the hardware adds 1 extra cycle to sync with the internal clock
                let cycles = sid_write.cycles.saturating_sub(1);
                chunk[0] = sid_write.reg;
                chunk[1] = sid_write.data;
                chunk[2] = (cycles >> 8) as u8;
                chunk[3] = cycles as u8;

                total_cycles += sid_write.cycles as u32;
            }

            if cycles_in_buffer.load(Ordering::Relaxed) >= total_cycles {
                cycles_in_buffer.fetch_sub(total_cycles, Ordering::Relaxed);
            } else {
                cycles_in_buffer.store(0, Ordering::Relaxed);
            }

            Self::usbsid_buffer_multi_write(&handles[device_index], &byte_buffer[..(count * 4)])?;
        }

        Self::mute_sids(&handles[device_index], devices[device_index].socket_count)?;
        Ok(())
    }

    fn is_aborted(aborted: &Arc<AtomicBool>) -> bool {
        aborted.load(Ordering::SeqCst)
    }

    fn detect_devices(&mut self) -> Result<UsbSidConfig, Error> {
        let mut usbsid_config = UsbSidConfig::default();

        let mut index = 0;
        for device in rusb::devices()?.iter() {
            let device_desc = device.device_descriptor()?;
            if device_desc.vendor_id() == USBSID_VENDOR && device_desc.product_id() == USBSID_PRODUCT_ID {
                self.configure_device(&device, index, &mut usbsid_config, USBSID_DEVICE_NAME)?;
                index += 1;
            }
        }

        Ok(usbsid_config)
    }

    fn configure_device(&mut self, device: &Device<GlobalContext>, index: usize, usbsid_config: &mut UsbSidConfig, device_name: &str) -> Result<(), Error> {
        let interface_number = Self::get_interface_number(device)?;
        let handle = self.open_and_store_device(device)?;

        if handle.kernel_driver_active(interface_number).unwrap_or(false) {
            handle.detach_kernel_driver(interface_number)?;
        }

        handle.claim_interface(interface_number)?;
        Self::setup_usb_serial(handle)?;

        let socket_count = Self::get_num_sids(handle)?;

        let id = (index + 1).to_string();
        usbsid_config.devices.push(DeviceInfo {
            name: format!("{}-{}", device_name, id),
            id,
            socket_count
        });

        Ok(())
    }

    fn open_and_store_device(&mut self, device: &Device<GlobalContext>) -> Result<&DeviceHandle<GlobalContext>, Error> {
        let handles = self.dev_handles.as_mut().ok_or(Error::Other)?;
        handles.push(device.open()?);
        Ok(handles.last().unwrap())
    }

    fn setup_usb_serial(handle: &DeviceHandle<GlobalContext>) -> Result<(), Error> {
        let timeout = Duration::from_millis(TIMEOUT_IN_MILLIS);
        let req_type = request_type(Direction::Out, RequestType::Class, Recipient::Interface);

        handle.write_control(req_type, ACM_SET_CONTROL_LINE_STATE, ACM_CTRL_DTR | ACM_CTRL_RTS, 0, &[], timeout)?;
        let count = handle.write_control(req_type, ACM_SET_LINE_CODING, 0, 0, &ENCODING, timeout)?;
        if count != ENCODING.len() {
            return Err(Error::Other);
        }
        Ok(())
    }

    fn get_interface_number(device: &Device<GlobalContext>) -> Result<u8, Error> {
        let config = device.config_descriptor(0)?;
        config.interfaces()
            .flat_map(|i| i.descriptors())
            .find_map(|d| {
                d.endpoint_descriptors().find(|ep|
                    ep.transfer_type() == TransferType::Bulk &&
                        ep.direction() == Direction::In &&
                        ep.address() == EP_IN_ADDR
                )?;

                d.endpoint_descriptors().find(|ep|
                    ep.transfer_type() == TransferType::Bulk &&
                        ep.direction() == Direction::Out &&
                        ep.address() == EP_OUT_ADDR
                )?;

                Some(d.interface_number())
            })
            .ok_or(Error::Other)
    }

    fn cleanup_handles(handles: &[DeviceHandle<GlobalContext>]) {
        for handle in handles {
            let device = handle.device();
            if let Ok(interface_number) = Self::get_interface_number(&device) {
                let _ = handle.release_interface(interface_number);
                let _ = handle.attach_kernel_driver(interface_number);
            }
        }
    }

    fn set_clock(handle: &DeviceHandle<GlobalContext>, clock_type: SidClock) -> rusb::Result<usize> {
        let write_buffer = [COMMAND << 6 | CONFIG, CMD_SET_CLOCK, clock_type as u8, 0, 0, 0];
        Self::usbsid_buffer_write(handle, &write_buffer)
    }

    fn get_pcb_version(handle: &DeviceHandle<GlobalContext>) -> rusb::Result<u8> {
        let write_buffer = [COMMAND << 6 | CONFIG, CMD_GET_PCB_VERSION, 0x01, 0, 0, 0];

        let timeout = Duration::from_millis(TIMEOUT_IN_MILLIS);
        handle.write_bulk(EP_OUT_ADDR, &write_buffer, timeout)?;

        let mut read_buffer = [0u8; 1];
        let size = handle.read_bulk(EP_IN_ADDR, &mut read_buffer, timeout)?;

        if size == 1 {
            Ok(read_buffer[0])
        } else {
            Err(Error::Other)
        }
    }

    fn get_num_sids(handle: &DeviceHandle<GlobalContext>) -> rusb::Result<i32> {
        let write_buffer = [COMMAND << 6 | CONFIG, CMD_GET_NUM_SIDS, 0, 0, 0, 0];

        let timeout = Duration::from_millis(TIMEOUT_IN_MILLIS);
        handle.write_bulk(EP_OUT_ADDR, &write_buffer, timeout)?;

        let mut read_buffer = [0u8; 1];
        let size = handle.read_bulk(EP_IN_ADDR, &mut read_buffer, timeout)?;

        if size == 1 {
            Ok(read_buffer[0] as i32)
        } else {
            Err(Error::Other)
        }
    }

    fn set_stereo_config(handle: &DeviceHandle<GlobalContext>, output_mode: UsbSidOutput) -> rusb::Result<usize> {
        let write_buffer = [COMMAND << 6 | CONFIG, CMD_SET_STEREO, output_mode as u8, 0, 0, 0];
        Self::usbsid_buffer_write(handle, &write_buffer)
    }

    fn usbsid_buffer_multi_write(handle: &DeviceHandle<GlobalContext>, buff: &[u8]) -> rusb::Result<usize> {
        let timeout = Duration::from_millis(TIMEOUT_IN_MILLIS);
        let mut buffer = [0u8; MAX_BULK_WRITE_SIZE];
        let mut total_written = 0;

        for chunk in buff.chunks(MAX_SID_WRITES * 4) {
            let len = chunk.len();
            buffer[0] = CYCLED_WRITE << 6 | (len as u8).saturating_sub(1);
            buffer[1..=len].copy_from_slice(chunk);
            total_written += handle.write_bulk(EP_OUT_ADDR, &buffer[..=len], timeout)?;
        }

        Ok(total_written)
    }

    fn usbsid_buffer_write(handle: &DeviceHandle<GlobalContext>, buff: &[u8]) -> rusb::Result<usize> {
        let timeout = Duration::from_millis(TIMEOUT_IN_MILLIS);
        handle.write_bulk(EP_OUT_ADDR, &buff[0..cmp::min(MAX_BULK_WRITE_SIZE, buff.len())], timeout)
    }

    fn config_sids(handle: &DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let pcb_version = Self::get_pcb_version(handle)?;
        if (pcb_version) >= 13 {
            Self::set_stereo_config(handle, UsbSidOutput::Mono)?;
        }

        Self::set_clock(handle, SidClock::Pal)?;
        Self::mute_sids(handle, socket_count)
    }

    fn push_sid_writes(buffer: &mut Vec<u8>, sid_writes: &Vec<SidWrite>) {
        for sid_write in sid_writes {
            Self::push_sid_write(buffer, sid_write);
        }
    }

    fn push_sid_write(buffer: &mut Vec<u8>, sid_write: &SidWrite) {
        buffer.push(sid_write.reg);
        buffer.push(sid_write.data);
        buffer.push((sid_write.cycles >> 8) as u8);
        buffer.push(sid_write.cycles as u8);
    }

    fn mute_sids(handle: &DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let mut buffer = vec![];

        for sid_index in 0..socket_count {
            let sid_writes = mossid::silent_sid_sequence((sid_index * 0x20) as u8, false);
            Self::push_sid_writes(&mut buffer, &sid_writes);
        }

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }

    fn set_sid_model_for_all_sids(handle: &DeviceHandle<GlobalContext>, socket_count: i32, sid_model: &SidModel) -> rusb::Result<usize> {
        let mut buffer = vec![];
        for sid_index in 0..socket_count {
            Self::configure_sid_replacement((sid_index * 0x20) as u8, &mut buffer, sid_model);
        }
        Self::usbsid_buffer_multi_write(handle, &buffer)
    }

    fn configure_sid_replacement(base_reg: u8, buffer: &mut Vec<u8>, sid_model: &SidModel) {
        let sid_filter = SidFilter {
            filter_strength_6581: 1,
            filter_lowest_freq_6581: 3,
            filter_central_freq_8580: 3,
            filter_lowest_freq_8580: 0
        };

        let arm_writes = armsid::configure_armsid(sid_model, &sid_filter);
        let fpga_writes = fpgasid::configure_fpgasid(sid_model);

        buffer.reserve((arm_writes.len() + fpga_writes.len()) * 4);

        for mut sid_write in arm_writes.into_iter().chain(fpga_writes) {
            sid_write.reg += base_reg;
            Self::push_sid_write(buffer, &sid_write);
        }
    }

    fn reset_all_sids(handle: &DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let mut buffer = vec![];

        let sid_writes = mossid::reset_all_sids_sequence(socket_count, true);
        Self::push_sid_writes(&mut buffer, &sid_writes);

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }

    fn reset_active_sids(handle: &DeviceHandle<GlobalContext>, base_reg: u8) -> rusb::Result<usize> {
        let mut buffer = vec![];

        let sid_writes = mossid::reset_sid_sequence(base_reg, true);
        Self::push_sid_writes(&mut buffer, &sid_writes);

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }
}
