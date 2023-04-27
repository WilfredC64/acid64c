// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::clock_adjust::ClockAdjust;
use super::sid_device::{SidDevice, SidClock, SamplingMethod, DeviceResponse, DeviceId};
use super::sidblaster_scheduler::{SidBlasterScheduler, SID_WRITES_BUFFER_SIZE};
use super::{ABORT_NO, MIN_CYCLE_SID_WRITE};

use std::collections::VecDeque;
use std::sync::atomic::{Ordering, AtomicI32, AtomicU32, AtomicBool};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use atomicring::AtomicRingBuffer;
use crate::player::ABORTED;
use crate::player::sidblaster_scheduler::{MAX_CYCLES_IN_BUFFER, SidWrite};
use crate::utils::sidblaster;

const DUMMY_REG: u8 = 0x1e;
const ERROR_MSG_DEVICE_FAILURE: &str = "Failure occurred during interaction with device.";
const ERROR_MSG_NO_SIDBLASTER_FOUND: &str = "No SIDBlaster USB device found.";
const SB_MIN_CYCLE_SID_WRITE: u32 = 4;
const ALLOWED_CYCLES_TO_BE_IN_BUFFER: u32 = 20_000;

pub struct SidBlasterUsbDeviceFacade {
    pub sb_device: SidBlasterUsbDevice
}

impl SidDevice for SidBlasterUsbDeviceFacade {
    fn get_device_id(&mut self, _dev_nr: i32) -> DeviceId { DeviceId::SidBlaster }

    fn disconnect(&mut self, _dev_nr: i32) {
        self.sb_device.disconnect();
    }

    fn is_connected(&mut self, _dev_nr: i32) -> bool {
        self.sb_device.is_connected()
    }

    fn get_last_error(&mut self, _dev_nr: i32) -> Option<String> {
        self.sb_device.get_last_error()
    }

    fn test_connection(&mut self, dev_nr: i32) {
        self.sb_device.test_connection(dev_nr);
    }

    fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        self.sb_device.can_pair_devices(dev1, dev2)
    }

    fn get_device_count(&mut self, _dev_nr: i32) -> i32 {
        self.sb_device.get_device_count()
    }

    fn get_device_info(&mut self, dev_nr: i32) -> String {
        self.sb_device.get_device_info(dev_nr)
    }

    fn set_sid_count(&mut self, _dev_nr: i32, sid_count: i32) {
        self.sb_device.set_sid_count(sid_count);
    }

    fn set_sid_position(&mut self, _dev_nr: i32, _sid_position: i8) {
        // not supported
    }

    fn set_sid_model(&mut self, dev_nr: i32, sid_socket: i32) {
        self.sb_device.set_sid_model(dev_nr, sid_socket);
    }

    fn set_sid_clock(&mut self, _dev_nr: i32, sid_clock: SidClock) {
        self.sb_device.set_sid_clock(sid_clock);
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

    fn silent_all_sids(&mut self, _dev_nr: i32, _write_volume: bool) {
        self.sb_device.silent_all_sids();
    }

    fn silent_active_sids(&mut self, _dev_nr: i32, _write_volume: bool) {
        self.sb_device.silent_all_sids();
    }

    fn reset_all_sids(&mut self, dev_nr: i32) {
        self.sb_device.reset_all_sids(dev_nr);
    }

    fn reset_active_sids(&mut self, dev_nr: i32) {
        self.sb_device.reset_active_sids(dev_nr);
    }

    fn reset_all_buffers(&mut self, _dev_nr: i32) {
        self.sb_device.reset_all_buffers();
    }

    fn enable_turbo_mode(&mut self, _dev_nr: i32) {
        self.sb_device.enable_turbo_mode();
    }

    fn disable_turbo_mode(&mut self, _dev_nr: i32) {
        self.sb_device.disable_turbo_mode();
    }

    fn dummy_write(&mut self, dev_nr: i32, cycles: u32) {
        self.sb_device.dummy_write(dev_nr, cycles);
    }

    fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.sb_device.write(dev_nr, cycles, reg, data)
    }

    fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        self.sb_device.try_write(dev_nr, cycles, reg, data)
    }

    fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        self.sb_device.retry_write(dev_nr)
    }

    fn force_flush(&mut self, dev_nr: i32) {
        self.sb_device.force_flush(dev_nr);
    }

    fn set_native_device_clock(&mut self, enabled: bool) {
        self.sb_device.set_native_device_clock(enabled);
    }

    fn get_device_clock(&mut self, _dev_nr: i32) -> SidClock {
        self.sb_device.get_device_clock()
    }

    fn has_remote_sidplayer(&mut self, _dev_nr: i32) -> bool {
        false
    }

    fn send_sid(&mut self, _dev_nr: i32, _filename: &str, _song_number: i32, _sid_data: &[u8], _ssl_data: &[u8]) {
        // not supported
    }

    fn stop_sid(&mut self, _dev_nr: i32) {
        // not supported
    }

    fn set_cycles_in_fifo(&mut self, _dev_nr: i32, _cycles: u32) {
        // not supported
    }
}

pub struct SidBlasterUsbDevice {
    device_names: Vec<String>,
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
    sid_blaster_scheduler: SidBlasterScheduler,
    queue: Arc<AtomicRingBuffer<SidWrite>>,
    cycles_in_buffer: Arc<AtomicU32>,
    queue_started: Arc<AtomicBool>,
    last_cycles: u32,
    last_reg: u8,
    last_data: u8,
    aborted: Arc<AtomicBool>
}

#[allow(dead_code)]
impl SidBlasterUsbDevice {
    pub fn new(abort_type: Arc<AtomicI32>) -> SidBlasterUsbDevice {
        let cycles_in_buffer = Arc::new(AtomicU32::new(0));
        let buf = Arc::new(AtomicRingBuffer::<SidWrite>::with_capacity(SID_WRITES_BUFFER_SIZE));
        let queue_started = Arc::new(AtomicBool::new(false));
        let aborted = Arc::new(AtomicBool::new(false));

        let sid_blaster_scheduler = SidBlasterScheduler::new(
            buf.clone(),
            queue_started.clone(),
            aborted.clone(),
            cycles_in_buffer.clone()
        );

        SidBlasterUsbDevice {
            device_names: vec![],
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
            sid_blaster_scheduler,
            queue: buf,
            cycles_in_buffer,
            queue_started,
            last_cycles: 0,
            last_reg: 0,
            last_data: 0,
            aborted
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        self.disconnect();

        self.abort_type.store(ABORT_NO, Ordering::Relaxed);
        self.last_error = None;

        self.device_names = sidblaster::detect_devices()?;
        self.sid_count = self.device_names.len() as i32;

        if self.sid_count > 0 {
            self.sid_blaster_scheduler.start()
        } else {
            Err(ERROR_MSG_NO_SIDBLASTER_FOUND.to_string())
        }
    }

    pub fn disconnect(&mut self) {
        self.device_names.clear();
        self.abort_type.store(ABORTED, Ordering::Relaxed);

        self.init_device_settings();
    }

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

        self.queue.clear();
        self.cycles_in_buffer.store(0, Ordering::SeqCst);

        self.init_write_state();
    }

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
        !self.device_names.is_empty()
    }

    pub fn test_connection(&mut self, _dev_nr: i32) {
    }

    pub fn can_pair_devices(&mut self, dev1: i32, dev2: i32) -> bool {
        dev1 != dev2
    }

    pub fn get_device_count(&self) -> i32 {
        self.sid_count
    }

    pub fn get_device_info(&mut self, dev_nr: i32) -> String {
        self.device_names[dev_nr as usize].clone()
    }

    pub fn set_sid_count(&mut self, sid_count: i32) {
        self.number_of_sids = sid_count;

        let _ = self.queue.try_push(SidWrite {
            reg: 0,
            data: 0,
            cycles: 0,
            clock: self.get_device_clock() as u8,
            stop_draining: true
        });
    }

    pub fn set_sid_model(&mut self, _dev_nr: i32, _sid_socket: i32) {
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
        self.clock_adjust.init(sid_clock);
    }

    pub fn silent_all_sids(&mut self) {
        if self.is_connected() {
            if self.cycles_in_buffer.load(Ordering::SeqCst) > ALLOWED_CYCLES_TO_BE_IN_BUFFER {
                self.queue.clear();
                self.cycles_in_buffer.store(0, Ordering::SeqCst);
            }

            let _ = self.queue.try_push(SidWrite {
                reg: 0,
                data: 0,
                cycles: 0,
                clock: self.get_device_clock() as u8,
                stop_draining: true
            });
        }
    }

    fn silent_sid(&mut self, dev_nr: i32, base_reg: u8) {
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

            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, base_reg + 0x18, 0);
        }
    }

    pub fn reset_all_sids(&mut self, dev_nr: i32) {
        if self.is_connected() {
            for i in 0..self.sid_count {
                let base_reg = i << 5;
                self.reset_sid(dev_nr, base_reg as u8);
            }
        }
    }

    pub fn reset_active_sids(&mut self, dev_nr: i32) {
        if self.is_connected() {
            self.reset_sid(dev_nr, 0);
        }
    }

    fn reset_sid(&mut self, dev_nr: i32, base_reg: u8) {
        if self.number_of_sids > 0 && self.is_connected() {
            self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, 0x18, 0x00);

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

    fn reset_sid_register(&mut self, dev_nr: i32, reg: u8) {
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0xff);
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x08);
        let base_reg = reg & 0xe0;
        self.write_direct(dev_nr, 50, base_reg + DUMMY_REG, 0);
        self.write_direct(dev_nr, MIN_CYCLE_SID_WRITE, reg, 0x00);
    }

    pub fn reset_all_buffers(&mut self) {
        if self.cycles_in_buffer.load(Ordering::SeqCst) > ALLOWED_CYCLES_TO_BE_IN_BUFFER {
            self.queue.clear();
            self.cycles_in_buffer.store(0, Ordering::SeqCst);
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
            self.write(dev_nr, cycles, DUMMY_REG, 0);
        }
    }

    fn are_multiple_sid_chips_supported(&mut self, reg: u8) -> bool {
        (reg >> 5) < self.sid_count as u8
    }

    pub fn force_flush(&mut self, _dev_nr: i32) {
        self.start_draining();
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

    fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    fn filter_reg_for_unsupported_writes(&mut self, reg: u8) -> u8 {
        if self.number_of_sids > 1 && !self.are_multiple_sid_chips_supported(reg) {
            // ignore second/third SID chip for devices that don't support accessing multiple SID chip simultaneously
            DUMMY_REG
        } else {
            reg
        }
    }

    pub fn write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        let reg = self.filter_reg_for_unsupported_writes(reg);
        let dev_nr = (dev_nr + ((reg & 0xe0) >> 5) as i32) % self.sid_count;

        let mut cycles = cycles;
        if self.cycles_to_compensate > 0 {
            cycles -= self.cycles_to_compensate;
            self.cycles_to_compensate = 0;
        }

        if self.is_connected() {
            if !self.use_native_device_clock {
                self.adjust_frequency(dev_nr, cycles, reg, data);
            } else {
                self.write_to_queue(dev_nr, cycles, reg, data);
            }
        }

        if self.is_aborted() {
            self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
            return DeviceResponse::Error;
        }

        DeviceResponse::Ok
    }

    fn write_to_queue(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        let _ = self.queue.try_push(SidWrite {
            reg: (reg & 0x1f) | (dev_nr << 5) as u8,
            data,
            cycles,
            clock: self.get_device_clock() as u8,
            stop_draining: false
        });

        self.cycles_in_buffer.fetch_add(cycles, Ordering::SeqCst);
    }

    fn adjust_frequency(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        let reg_offset = reg & 0x1f;

        if reg_offset < 0x10 {
            let voice_offset = reg_offset % 7;

            match voice_offset {
                0x00 | 0x01 => {
                    let voice_nr = reg_offset / 7;
                    let base_reg = reg & 0xe0;
                    self.adjust_frequency_for_voice(dev_nr, voice_nr, cycles, base_reg, voice_offset, data)
                },
                _ => self.write_to_queue(dev_nr, cycles, reg, data)
            }
        } else {
            self.write_to_queue(dev_nr, cycles, reg, data)
        }
    }

    fn adjust_frequency_for_voice(&mut self, dev_nr: i32, voice_nr: u8, cycles: u32, base_reg: u8, reg: u8, data: u8) {
        if reg <= 1 {
            let voice_index = voice_nr + (base_reg >> 5) * 3;

            self.clock_adjust.update_frequency(voice_index, reg, data);
            let last_freq = self.clock_adjust.get_last_scaled_freq(voice_index);
            let scaled_freq = self.clock_adjust.scale_frequency(voice_index);
            let update_hi_freq = last_freq & 0xff00 != scaled_freq & 0xff00;

            let mut cycles = cycles;
            let voice_base = voice_nr * 7;

            if update_hi_freq {
                self.write_to_queue(dev_nr, cycles, 1 + voice_base + base_reg, (scaled_freq >> 8) as u8);
                cycles = SB_MIN_CYCLE_SID_WRITE;
                self.cycles_to_compensate += SB_MIN_CYCLE_SID_WRITE;
            }
            self.write_to_queue(dev_nr, cycles, voice_base + base_reg, (scaled_freq & 0xff) as u8);
        }
    }

    pub fn has_max_data_in_buffer(&mut self) -> bool {
        let cycles = self.cycles_in_buffer.load(Ordering::SeqCst);

        let enough_data = self.queue.len() > SID_WRITES_BUFFER_SIZE / 2 || cycles > MAX_CYCLES_IN_BUFFER;
        if enough_data {
            self.start_draining();

            if !self.turbo_mode {
                thread::sleep(Duration::from_millis(5));
            } else {
                thread::yield_now();
            }
        }
        enough_data
    }

    pub fn start_draining(&mut self) {
        self.queue_started.store(true, Ordering::SeqCst);
    }

    fn write_direct(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) {
        self.write(dev_nr, cycles, reg, data);
    }

    pub fn retry_write(&mut self, dev_nr: i32) -> DeviceResponse {
        self.try_write(dev_nr, self.last_cycles, self.last_reg, self.last_data)
    }

    pub fn try_write(&mut self, dev_nr: i32, cycles: u32, reg: u8, data: u8) -> DeviceResponse {
        if self.is_aborted() {
            self.disconnect_with_error(ERROR_MSG_DEVICE_FAILURE.to_string());
            return DeviceResponse::Error
        }

        if self.has_max_data_in_buffer() {
            self.last_cycles = cycles;
            self.last_reg = reg;
            self.last_data = data;
            return DeviceResponse::Busy;
        }

        self.write(dev_nr, cycles, reg, data)
    }
}
