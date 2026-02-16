// Copyright (C) 2019 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use super::sid_device::SidClock;
use std::collections::VecDeque;
use std::time::Instant;

const PAL_CYCLES_PER_SECOND: f64 = 17_734_475.0 / 18.0;    // = 0985248,611 = ~ 312 * 63 * 50;
const NTSC_CYCLES_PER_SECOND: f64 = 14_318_180.0 / 14.0;   // = 1022727,143 = ~ 263 * 65 * 60;
const ONE_MHZ_CYCLES_PER_SECOND: f64 = 1_000_000.0;

#[derive(Copy, Clone)]
pub struct SidWrite {
    pub reg: u8,
    pub data: u8,
    pub cycles: u32,
    pub cycles_real: u32
}

impl SidWrite {
    pub fn new(reg: u8, data: u8, cycles: u32, cycles_real: u32) -> SidWrite {
        SidWrite {
            reg,
            data,
            cycles,
            cycles_real
        }
    }
}

pub struct SidDataProcessor {
    time_in_cycles: u32,            // current time of the tune played in cycles
    time_elapsed_in_cycles: u32,    // time in cycles elapsed from last start/pause
    last_sid_write: [u8; 256],
    second_last_sid_write: [u8; 256],
    last_sid_write_times: [u32; 256],
    sid_clock: SidClock,
    sid_write_fifo: VecDeque<SidWrite>,
    cycles_in_fifo: u32,
    current_sid_write: Option<SidWrite>,
    current_time: Option<Instant>,
    cycles_per_second: f64,
    next_time_in_micros: u128
}

impl SidDataProcessor {
    pub fn new() -> SidDataProcessor {
        SidDataProcessor {
            time_in_cycles: 0,
            time_elapsed_in_cycles: 0,
            last_sid_write: [0; 256],
            second_last_sid_write: [0; 256],
            last_sid_write_times: [0; 256],
            sid_clock: SidClock::Pal,
            sid_write_fifo: VecDeque::with_capacity(0x1ffff),
            cycles_in_fifo: 0,
            current_sid_write: None,
            current_time: None,
            cycles_per_second: PAL_CYCLES_PER_SECOND,
            next_time_in_micros: 0
        }
    }

    pub fn init(&mut self, current_time_in_cycles: u32) {
        self.time_in_cycles = current_time_in_cycles;

        self.sync_time();
        self.current_time = None;

        self.next_time_in_micros = 0;
        self.sid_write_fifo = VecDeque::with_capacity(0x1ffff);
        self.cycles_in_fifo = 0;

        if current_time_in_cycles == 0 {
            self.last_sid_write = [0; 256];
            self.second_last_sid_write = [0; 256];
            self.last_sid_write_times = [0; 256];
        }
    }

    // cycles can be different than cycles_real in case of fast forward, where cycles param is set to minimum cycle value
    pub fn write(&mut self, cycles: u32, reg: u8, data: u8, cycles_real: u32) {
        if self.current_time.is_none() {
            self.current_time = Some(Instant::now());
        }

        self.cycles_in_fifo += cycles;
        self.sid_write_fifo.push_back(SidWrite::new(reg, data, cycles, cycles_real));
    }

    pub fn get_buffer_copy(&self) -> Vec<SidWrite> {
        let mut sid_writes = Vec::new();

        sid_writes.extend(self.sid_write_fifo.iter());
        sid_writes
    }

    pub fn clear_buffer(&mut self) {
        self.init(self.time_in_cycles);
    }

    pub fn get_cycles_in_fifo(&mut self) -> u32 {
        self.cycles_in_fifo
    }

    pub fn get_time_in_millis(&self) -> u32 {
        (self.time_in_cycles as f64 / (self.cycles_per_second / 1000.0)).round() as u32
    }

    fn process_write(&mut self, reg: u8, data: u8, cycles: u32, cycles_real: u32) {
        self.time_in_cycles += cycles_real;
        self.time_elapsed_in_cycles += cycles;

        if data != self.last_sid_write[reg as usize] {
            self.second_last_sid_write[reg as usize] = self.last_sid_write[reg as usize];

            if data != self.last_sid_write[reg as usize] {
                self.last_sid_write_times[reg as usize] = self.time_in_cycles;
            }
            self.last_sid_write[reg as usize] = data;
        }
    }

    pub fn get_sid_write(&self) -> Option<SidWrite> {
        let front = self.sid_write_fifo.front();
        front.copied()
    }

    pub fn process_sid_write_fifo(&mut self) {
        while !self.sid_write_fifo.is_empty() {
            if self.current_sid_write.is_none() {
                self.current_sid_write = self.get_sid_write();
                if let Some(current_sid_write) = self.current_sid_write {
                    let cycles = self.time_elapsed_in_cycles + current_sid_write.cycles;
                    self.next_time_in_micros = (cycles as f64 / (self.cycles_per_second / 1000000.0)) as u128;
                }
            }
            self.process_next_data();

            if self.current_sid_write.is_some() {
                break;
            }
        }
    }

    pub fn get_next_event_in_millis(&mut self) -> u64 {
        if let Some(current_time) = self.current_time && self.next_time_in_micros > 0 {
            let now = current_time.elapsed().as_micros();
            if self.next_time_in_micros > now {
                return ((self.next_time_in_micros - now) / 1000) as u64;
            }
        }
        0
    }

    fn process_next_data(&mut self) {
        if let Some(sid_write) = self.current_sid_write {
            let elapsed =  self.current_time.unwrap().elapsed().as_micros();
            if elapsed >= self.next_time_in_micros {
                self.sid_write_fifo.pop_front();
                self.cycles_in_fifo -= sid_write.cycles;
                self.process_write(sid_write.reg, sid_write.data, sid_write.cycles, sid_write.cycles_real);
                self.current_sid_write = None;
            }
        }
    }

    fn sync_time(&mut self) {
        self.current_time = Some(Instant::now());
        self.time_elapsed_in_cycles = 0;
        self.current_sid_write = None;
    }

    pub fn set_sid_clock(&mut self, sid_clock: SidClock) {
        self.sid_clock = sid_clock;
        self.cycles_per_second = Self::get_cycles_per_second(sid_clock);
    }

    pub fn is_note_finished(&mut self, reg_base: u8) -> bool {
        static ENV_DECAY_RELEASE_IN_CYCLES: [u32; 16] = [
            (0x0009 * 3) << 8, // ~6ms
            (0x0020 * 3) << 8, // ~24ms
            (0x003f * 3) << 8, // ~48ms
            (0x005f * 3) << 8, // ~72ms
            (0x0095 * 3) << 8, // ~114ms
            (0x00dc * 3) << 8, // ~168ms
            (0x010b * 3) << 8, // ~204ms
            (0x0139 * 3) << 8, // ~240ms
            (0x0188 * 3) << 8, // ~300ms
            (0x03d1 * 3) << 8, // ~750ms
            (0x07a2 * 3) << 8, // ~1.5s
            (0x0c36 * 3) << 8, // ~2.4s
            (0x0f43 * 3) << 8, // ~3s
            (0x2dc8 * 3) << 8, // ~9s
            (0x4c4c * 3) << 8, // ~15s
            (0x7a13 * 3) << 8  // ~24s
        ];

        let last_write_time = self.get_last_sid_write_times(0x04 + reg_base);
        let last_write_time_diff = self.time_in_cycles.saturating_sub(last_write_time);

        let gate_cleared = self.last_sid_write[0x04 + reg_base as usize] & 1 == 0;
        let last_release = self.last_sid_write[0x06 + reg_base as usize] & 0x0f;

        gate_cleared && ENV_DECAY_RELEASE_IN_CYCLES[last_release as usize] < last_write_time_diff
    }

    pub fn get_last_sid_write(&self, reg: u8) -> u8 {
        self.last_sid_write[reg as usize]
    }

    pub fn get_last_sid_write_times(&self, reg: u8) -> u32 {
        self.last_sid_write_times[reg as usize]
    }

    pub fn get_cycles_per_second(sid_clock: SidClock) -> f64 {
        match sid_clock {
            SidClock::Pal => PAL_CYCLES_PER_SECOND,
            SidClock::Ntsc => NTSC_CYCLES_PER_SECOND,
            _ => ONE_MHZ_CYCLES_PER_SECOND
        }
    }
}
