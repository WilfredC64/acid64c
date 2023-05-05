// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::{thread, time::Duration};
use std::time::Instant;

use atomicring::AtomicRingBuffer;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use crate::utils::sidblaster;

pub const SID_WRITES_BUFFER_SIZE: usize = 65_536;
pub const MAX_CYCLES_IN_BUFFER: u32 = 63*312 * 50 * 2; // ~2 seconds

const PAL_CYCLES_PER_MICRO: f64 = 17_734_475.0 / 18.0 / 1_000_000.0;
const NTSC_CYCLES_PER_MICRO: f64 = 14_318_180.0 / 14.0 / 1_000_000.0;
const ONE_MHZ_CYCLES_PER_MICRO: f64 = 1.0;

const MAX_DEVICE_BUFFER_SIZE: usize = 50;
const MAX_DEVICE_BUFFER_CYCLES: u32 = 1000;
const ALLOW_DOUBLE_REG_WRITES_WITHIN_CYCLES: u32 = 20;
const THRESHOLD_TO_FLUSH_BUFFER_IN_CYCLES: u32 = 500;
const THRESHOLD_TO_SLEEP_THREAD_IN_MICROS: u128 = 1500;

pub enum SidClock {
    Pal = 0,
    Ntsc = 1,
    OneMhz = 2
}

impl SidClock {
    pub fn from_u8(value: u8) -> SidClock {
        match value {
            0 => SidClock::Pal,
            1 => SidClock::Ntsc,
            _ => SidClock::OneMhz,
        }
    }
}

pub struct SidWrite {
    pub cycles: u32,
    pub reg: u8,
    pub data: u8,
    pub clock: u8,
    pub stop_draining: bool
}

pub struct SidBlasterScheduler {
    queue: Arc<AtomicRingBuffer<SidWrite>>,
    queue_started: Arc<AtomicBool>,
    cycles_in_buffer: Arc<AtomicU32>,
    sid_writer_thread: Option<thread::JoinHandle<()>>,
    aborted: Arc<AtomicBool>,
}

impl Drop for SidBlasterScheduler {
    fn drop(&mut self) {
        self.stop_sid_writer_thread();
    }
}

impl SidBlasterScheduler {
    pub fn new(
        queue: Arc<AtomicRingBuffer<SidWrite>>,
        queue_started: Arc<AtomicBool>,
        aborted: Arc<AtomicBool>,
        cycles_in_buffer: Arc<AtomicU32>
    ) -> SidBlasterScheduler {

        SidBlasterScheduler {
            queue,
            queue_started,
            cycles_in_buffer,
            sid_writer_thread: None,
            aborted
        }
    }

    fn stop_sid_writer_thread(&mut self) {
        self.aborted.store(true, Ordering::SeqCst);

        if self.sid_writer_thread.is_some() {
            let _ = self.sid_writer_thread.take().unwrap().join().ok();
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.stop_sid_writer_thread();

        self.aborted.store(false, Ordering::SeqCst);

        let mut sid_devices = sidblaster::get_devices()?;
        if sid_devices.is_empty() {
            return Err(sidblaster::ERROR_MSG_NO_SIDBLASTER_FOUND.to_string());
        }

        let queue = self.queue.clone();
        let cycles_in_buffer = self.cycles_in_buffer.clone();
        let queue_started = self.queue_started.clone();

        let aborted = self.aborted.clone();

        let mut last_write = None;

        let mut last_dev_nr = 0;

        self.sid_writer_thread = Some(thread::spawn(move || {
            let _ = set_current_thread_priority(ThreadPriority::Max);

            let mut cycles_processed = 0_u32;

            let mut buffer = vec![];
            let mut sid_write_usage = [false; 256];

            let mut next_write: Option<SidWrite> = None;

            let mut cycles_in_temp_buffer = 0;

            sidblaster::silent_sids(&mut sid_devices);

            loop {
                if Self::is_aborted(&aborted) {
                    sidblaster::silent_sids(&mut sid_devices);
                    break;
                }

                if !queue_started.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }

                let sid_write = if next_write.is_none() {
                    queue.try_pop()
                } else {
                    next_write.take()
                };

                if let Some(sid_write) = &sid_write {
                    let cycles_per_micro = match SidClock::from_u8(sid_write.clock) {
                        SidClock::Pal => PAL_CYCLES_PER_MICRO,
                        SidClock::Ntsc => NTSC_CYCLES_PER_MICRO,
                        SidClock::OneMhz => ONE_MHZ_CYCLES_PER_MICRO
                    };

                    let cycles = sid_write.cycles;
                    let reg = sid_write.reg;

                    let dev_nr = reg >> 5;
                    let device_change = !buffer.is_empty() && dev_nr != last_dev_nr;
                    last_dev_nr = dev_nr;

                    cycles_processed += cycles;
                    cycles_in_temp_buffer += cycles;

                    if !sid_write.stop_draining {
                        buffer.push(reg & 0x1f | 0xe0);
                        buffer.push(sid_write.data);
                    }

                    sid_write_usage[reg as usize] = true;

                    next_write = queue.try_pop();

                    let mut should_flush = device_change || sid_write.stop_draining;
                    if let Some(next) = &next_write {
                        if next.cycles > THRESHOLD_TO_FLUSH_BUFFER_IN_CYCLES || next.reg >> 5 != dev_nr || (sid_write_usage[next.reg as usize] && next.cycles > ALLOW_DOUBLE_REG_WRITES_WITHIN_CYCLES) {
                            should_flush |= true;
                        }
                    }

                    if !buffer.is_empty() && (should_flush || buffer.len() > MAX_DEVICE_BUFFER_SIZE || cycles_in_temp_buffer > MAX_DEVICE_BUFFER_CYCLES || sid_write.cycles > THRESHOLD_TO_FLUSH_BUFFER_IN_CYCLES) {
                        if last_write.is_none() {
                            last_write = Some(Instant::now());
                        }

                        Self::wait(cycles_processed, &last_write.unwrap(), cycles_per_micro);

                        if sidblaster::write(&mut sid_devices[dev_nr as usize], &buffer).is_err() {
                            aborted.store(true, Ordering::SeqCst);
                        }

                        buffer.clear();

                        cycles_in_temp_buffer = 0;
                        sid_write_usage = [false; 256];
                    }

                    if cycles_in_buffer.load(Ordering::SeqCst) >= cycles {
                        cycles_in_buffer.fetch_sub(cycles, Ordering::SeqCst);
                    } else {
                        cycles_in_buffer.store(0, Ordering::SeqCst);
                    }

                    if sid_write.stop_draining {
                        last_write = None;
                        cycles_processed = 0;
                        queue_started.store(false, Ordering::SeqCst);
                    }
                } else {
                    last_write = None;
                    cycles_processed = 0;
                    thread::sleep(Duration::from_millis(5));
                }
            }
        }));

        Ok(())
    }

    fn wait(cycles: u32, start_time: &Instant, cycles_per_micro: f64) {
        let next_time_in_micros = (cycles as f64 / cycles_per_micro) as u128;
        let elapsed = start_time.elapsed().as_micros();

        if elapsed < next_time_in_micros {
            let time_to_wait = next_time_in_micros - elapsed;
            if time_to_wait > THRESHOLD_TO_SLEEP_THREAD_IN_MICROS {
                thread::sleep(Duration::from_millis(time_to_wait as u64 / 1000 - 1));
            }
            while start_time.elapsed().as_micros() < next_time_in_micros {}
        }
    }

    fn is_aborted(aborted: &Arc<AtomicBool>) -> bool {
        aborted.load(Ordering::SeqCst)
    }
}
