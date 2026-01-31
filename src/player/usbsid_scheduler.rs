// Copyright (C) 2025 - 2026 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::{cmp, thread, time::Duration};
use crossbeam_channel::{Receiver};
use ringbuf::{SharedRb, CachingCons};
use ringbuf::storage::Heap;
use ringbuf::traits::Consumer;
use rusb::{Device, EndpointDescriptor, GlobalContext, Error};
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::player::sid_device::{DeviceInfo, SidModel, SidWrite};
use crate::utils::{armsid, armsid::SidFilter, fpgasid, mossid};

pub const USBSID_DEVICE_NAME: &str = "USBSID-Pico";
pub const ERROR_NO_USBSID_FOUND: &str = "No USBSID device found.";
const ERROR_CONNECTING_DEVICE: &str = "Error connecting to USBSID Device";
const ERROR_STARTING_SCHEDULER: &str = "Error starting USBSID Scheduler.";

const USBSID_VENDOR: u16 = 0xCAFE;
const USBSID_PRODUCT_ID: u16 = 0x4011;
const BUFFER_EMPTY_DELAY_IN_MILLIS: u64 = 5;

const EP_OUT_ADDR: u8 = 0x02;
const EP_IN_ADDR: u8 = 0x82;
const ACM_CTRL_DTR: u16 = 0x01;
const ACM_CTRL_RTS: u16 = 0x02;
const ENCODING: [u8; 7] = [0x40, 0x54, 0x89, 0x00, 0x00, 0x00, 0x08];

const USB_BUFFER_SIZE: usize = 64;
const MAX_SID_WRITES: usize = (USB_BUFFER_SIZE - 1) / 4;
const MAX_BULK_WRITE_SIZE: usize = MAX_SID_WRITES * 4 + 1;

const CYCLED_WRITE: u8 = 0x02;
const COMMAND: u8 = 0x03;

const CONFIG: u8 = 0x12;
const CMD_GET_NUM_SIDS: u8 = 0x39;
const CMD_GET_PCB_VERSION: u8 = 0x81;
const CMD_SET_STEREO: u8 = 0x89;
const CMD_SET_CLOCK: u8 = 0x50;

pub enum UsbSidCommand {
    Abort,
    ClearBuffer,
    SetDevice,
    SetClock,
    SetModel,
    MuteAll,
    Reset,
    ResetAll,
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

pub struct UsbSidConfig {
    pub devices: Vec<DeviceInfo>,
}

pub struct UsbSidScheduler {
    queue: Option<CachingCons<Arc<SharedRb<Heap<SidWrite>>>>>,
    sid_writer_thread: Option<thread::JoinHandle<()>>,
    aborted: Arc<AtomicBool>,

    dev_handles: Option<Vec<rusb::DeviceHandle<GlobalContext>>>,
    in_endpoint: Vec<u8>,
    out_endpoint: Vec<u8>,
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
            in_endpoint: vec![],
            out_endpoint: vec![],
            cycles_in_buffer
        }
    }

    fn stop_sid_writer_thread(&mut self) {
        self.aborted.store(true, Ordering::SeqCst);

        if self.sid_writer_thread.is_some() {
            let _ = self.sid_writer_thread.take().unwrap().join().ok();
        }
    }

    pub fn start(&mut self, cmd_receiver: Receiver<(UsbSidCommand, i32)>) -> Result<UsbSidConfig, String> {
        self.stop_sid_writer_thread();
        self.aborted.store(false, Ordering::SeqCst);

        let usbsid_config = self.detect_devices().map_err(|error| format!("{}: {error}", ERROR_CONNECTING_DEVICE))?;
        if usbsid_config.devices.is_empty() {
            return Err(ERROR_NO_USBSID_FOUND.to_string());
        }

        let mut queue = self.queue.take().ok_or(ERROR_STARTING_SCHEDULER.to_string())?;
        let mut handles = self.dev_handles.take().ok_or(ERROR_STARTING_SCHEDULER.to_string())?;

        let mut write_buffer = [SidWrite::default(); MAX_SID_WRITES];
        let cycles_in_buffer = self.cycles_in_buffer.clone();
        let devices = usbsid_config.devices.clone();
        let aborted = self.aborted.clone();

        self.sid_writer_thread = Some(thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut device_index = 0;
            if Self::config_sids(&mut handles[device_index], devices[device_index].socket_count).is_err() {
                aborted.store(true, Ordering::SeqCst);
                return;
            }

            loop {
                if Self::is_aborted(&aborted) {
                    break;
                }

                let recv_result = cmd_receiver.try_recv();
                if let Ok((command, device)) = recv_result {
                    match command {
                        UsbSidCommand::Abort => {
                            if Self::mute_sids(&mut handles[device_index], devices[device_index].socket_count).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                            }
                            break;
                        }
                        UsbSidCommand::ClearBuffer => {
                            cycles_in_buffer.store(0, Ordering::Relaxed);
                            queue.clear();
                        }
                        UsbSidCommand::MuteAll => {
                            if Self::mute_sids(&mut handles[device_index], devices[device_index].socket_count).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        UsbSidCommand::SetDevice => {
                            device_index = device as usize;
                            if Self::config_sids(&mut handles[device_index], devices[device_index].socket_count).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        UsbSidCommand::SetClock => {
                            let clock_type = if device == 0 { SidClock::Pal } else { SidClock::Ntsc };
                            if Self::usb_set_clock(&mut handles[device_index], clock_type).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        UsbSidCommand::SetModel => {
                            let sid_model = if device == 0 { SidModel::Mos6581 } else { SidModel::Mos8580 };
                            if Self::set_sid_model_for_all_sids(&mut handles[device_index], devices[device_index].socket_count, &sid_model).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        UsbSidCommand::Reset => {
                            if Self::reset_active_sids(&mut handles[device_index], device as u8).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        UsbSidCommand::ResetAll => {
                            if Self::reset_all_sids(&mut handles[device_index], devices[device_index].socket_count).is_err() {
                                aborted.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                }

                let count = queue.pop_slice(&mut write_buffer);

                if count == 0 {
                    thread::sleep(Duration::from_millis(BUFFER_EMPTY_DELAY_IN_MILLIS));
                    continue;
                }

                let mut byte_buffer = [0u8; MAX_SID_WRITES * 4];

                let mut total_cycles: u32 = 0;
                for (chunk, sid_write) in byte_buffer.chunks_exact_mut(4).zip(&write_buffer[..count]) {
                    let cycles = sid_write.cycles.saturating_sub(1);
                    chunk[0] = sid_write.reg;
                    chunk[1] = sid_write.data;
                    chunk[2] = (cycles >> 8) as u8;
                    chunk[3] = (cycles & 0xFF) as u8;

                    total_cycles += sid_write.cycles as u32;
                }

                if cycles_in_buffer.load(Ordering::Relaxed) >= total_cycles {
                    cycles_in_buffer.fetch_sub(total_cycles, Ordering::Relaxed);
                } else {
                    cycles_in_buffer.store(0, Ordering::Relaxed);
                }

                if Self::usbsid_buffer_multi_write(&mut handles[device_index], &byte_buffer[..(count * 4)]).is_err() {
                    aborted.store(true, Ordering::SeqCst);
                    break;
                }
            }

            cycles_in_buffer.store(0, Ordering::SeqCst);
            queue.clear();
            aborted.store(true, Ordering::SeqCst);
        }));

        Ok(usbsid_config)
    }

    fn is_aborted(aborted: &Arc<AtomicBool>) -> bool {
        aborted.load(Ordering::SeqCst)
    }

    fn detect_devices(&mut self) -> Result<UsbSidConfig, Error> {
        let mut usbsid_config = UsbSidConfig {
            devices: vec![],
        };

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
        let config = device.config_descriptor(0)?;

        let interface = config.interfaces()
            .find(|interface| {
                interface.descriptors().any(|desc| {
                    desc.endpoint_descriptors().any(|ep| {
                        ep.transfer_type() == rusb::TransferType::Bulk
                    })
                })
            })
            .ok_or(Error::Other)?;

        let in_endpoint_filter = |endpoint: &EndpointDescriptor| {
            endpoint.direction() == rusb::Direction::In &&
                endpoint.address() == EP_IN_ADDR

        };
        let out_endpoint_filter = |endpoint: &EndpointDescriptor| {
            endpoint.direction() == rusb::Direction::Out &&
                endpoint.address() == EP_OUT_ADDR
        };

        let interface_desc = interface.descriptors().next().unwrap();

        let mut in_endpoint: Option<EndpointDescriptor> = None;
        let mut out_endpoint: Option<EndpointDescriptor> = None;

        for ep in interface_desc.endpoint_descriptors() {
            if in_endpoint.is_none() && in_endpoint_filter(&ep) {
                in_endpoint = Some(ep);
                if out_endpoint.is_some() { break; }
            } else if out_endpoint.is_none() && out_endpoint_filter(&ep) {
                out_endpoint = Some(ep);
                if in_endpoint.is_some() { break; }
            }
        }

        let in_endpoint = in_endpoint.ok_or(Error::Other)?;
        let out_endpoint = out_endpoint.ok_or(Error::Other)?;

        let interface_desc = interface.descriptors().next().ok_or(Error::Other)?;
        let interface_number = interface_desc.interface_number();

        let mut handle = device.open()?;

        if handle.kernel_driver_active(interface_number).unwrap_or(false) {
            handle.detach_kernel_driver(interface_number)?;
        }
        handle.claim_interface(interface_number)?;

        let timeout = Duration::from_secs(0);

        handle.write_control(0x21, 0x22, ACM_CTRL_DTR | ACM_CTRL_RTS, 0, &[], timeout)?;
        let rc = handle.write_control(0x21, 0x20, 0, 0, &ENCODING, timeout)?;
        if rc != ENCODING.len() {
            let _ = handle.release_interface(interface_number);
            let _ = handle.attach_kernel_driver(interface_number);
            return Err(Error::Other);
        }

        self.in_endpoint.push(in_endpoint.address());
        self.out_endpoint.push(out_endpoint.address());

        let socket_count = Self::usb_get_num_sids(&mut handle)?;

        let id = (index + 1).to_string();
        usbsid_config.devices.push(DeviceInfo {
            name: format!("{}-{}", device_name, id),
            id,
            socket_count: socket_count as i32
        });

        if let Some(ref mut handles) = self.dev_handles {
            handles.push(handle);
        }

        Ok(())
    }

    fn usb_set_clock(handle: &mut rusb::DeviceHandle<GlobalContext>, clock_type: SidClock) -> rusb::Result<usize> {
        let write_buffer = [
            COMMAND << 6 | CONFIG,
            CMD_SET_CLOCK,
            clock_type as u8,
            0,
            0,
            0,
        ];

        Self::usbsid_buffer_write(handle, &write_buffer)
    }

    fn usb_get_pcb_version(handle: &mut rusb::DeviceHandle<GlobalContext>) -> rusb::Result<u8> {
        let write_buffer = [
            COMMAND << 6 | CONFIG,
            CMD_GET_PCB_VERSION,
            0x01,
            0,
            0,
            0,
        ];

        let timeout = Duration::from_millis(0);
        handle.write_bulk(EP_OUT_ADDR, &write_buffer, timeout)?;

        let mut read_buffer = [0u8; 1];
        let size = handle.read_bulk(EP_IN_ADDR, &mut read_buffer, timeout)?;

        if size == 1 {
            Ok(read_buffer[0])
        } else {
            Err(Error::Other)
        }
    }

    fn usb_get_num_sids(handle: &mut rusb::DeviceHandle<GlobalContext>) -> rusb::Result<u8> {
        let write_buffer = [
            COMMAND << 6 | CONFIG,
            CMD_GET_NUM_SIDS,
            0,
            0,
            0,
            0,
        ];

        let timeout = Duration::from_millis(0);
        handle.write_bulk(EP_OUT_ADDR, &write_buffer, timeout)?;

        let mut read_buffer = [0u8; 1];
        let size = handle.read_bulk(EP_IN_ADDR, &mut read_buffer, timeout)?;

        if size == 1 {
            Ok(read_buffer[0])
        } else {
            Err(Error::Other)
        }
    }

    fn set_stereo_config(handle: &mut rusb::DeviceHandle<GlobalContext>, output_mode: UsbSidOutput) -> rusb::Result<usize> {
        let write_buffer = [
            COMMAND << 6 | CONFIG,
            CMD_SET_STEREO,
            output_mode as u8,
            0,
            0,
            0,
        ];

        Self::usbsid_buffer_write(handle, &write_buffer)
    }

    fn usbsid_buffer_multi_write(handle: &mut rusb::DeviceHandle<GlobalContext>, buff: &[u8]) -> rusb::Result<usize> {
        let timeout = Duration::from_millis(0);
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

    fn usbsid_buffer_write(handle: &mut rusb::DeviceHandle<GlobalContext>, buff: &[u8]) -> rusb::Result<usize> {
        let timeout = Duration::from_millis(0);
        handle.write_bulk(EP_OUT_ADDR, &buff[0..cmp::min(MAX_BULK_WRITE_SIZE, buff.len())], timeout)
    }

    fn config_sids(handle: &mut rusb::DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let pcb_version = Self::usb_get_pcb_version(handle)?;
        if (pcb_version) >= 13 {
            Self::set_stereo_config(handle, UsbSidOutput::Mono)?;
        }

        Self::usb_set_clock(handle, SidClock::Pal)?;
        Self::mute_sids(handle, socket_count)
    }

    fn mute_sids(handle: &mut rusb::DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let mut sid_writes: Vec<SidWrite> = vec![];
        for sid_index in 0..socket_count {
            let writes = mossid::silent_sid_sequence((sid_index * 0x20) as u8, false);
            sid_writes.extend_from_slice(&writes);
        }

        let mut buffer = vec![];
        for sid_write in sid_writes {
            buffer.push(sid_write.reg);
            buffer.push(sid_write.data);
            buffer.push((sid_write.cycles >> 8) as u8);
            buffer.push(sid_write.cycles as u8);
        }

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }

    fn set_sid_model_for_all_sids(handle: &mut rusb::DeviceHandle<GlobalContext>, socket_count: i32, sid_model: &SidModel) -> rusb::Result<usize> {
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

        for sid_write in arm_writes.into_iter().chain(fpga_writes) {
            buffer.push(base_reg + sid_write.reg);
            buffer.push(sid_write.data);
            buffer.push((sid_write.cycles >> 8) as u8);
            buffer.push(sid_write.cycles as u8);
        }
    }

    fn reset_all_sids(handle: &mut rusb::DeviceHandle<GlobalContext>, socket_count: i32) -> rusb::Result<usize> {
        let sid_writes = mossid::reset_all_sids_sequence(socket_count, true);

        let mut buffer = vec![];
        for sid_write in sid_writes {
            buffer.push(sid_write.reg);
            buffer.push(sid_write.data);
            buffer.push((sid_write.cycles >> 8) as u8);
            buffer.push(sid_write.cycles as u8);
        }

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }

    fn reset_active_sids(handle: &mut rusb::DeviceHandle<GlobalContext>, base_reg: u8) -> rusb::Result<usize> {
        let mut buffer = vec![];

        let sid_writes = mossid::reset_sid_sequence(base_reg, true);
        for sid_write in sid_writes {
            let cycles = sid_write.cycles;
            buffer.extend_from_slice(&[sid_write.reg, sid_write.data, (cycles >> 8) as u8, (cycles & 0xFF) as u8]);
        }

        Self::usbsid_buffer_multi_write(handle, &buffer)
    }
}
