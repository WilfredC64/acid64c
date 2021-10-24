// Copyright (C) 2019 - 2021 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::cmp::min;
use super::sid_device::SidClock;
use super::MIN_CYCLE_SID_WRITE;

const HS_CLOCK: f64 = 1000000.0;
const PAL_CLOCK: f64 = 17734475.0 / 18.0;
const NTSC_CLOCK: f64 = 14318180.0 / 14.0;

const PAL_CLOCK_SCALE: f64 = (HS_CLOCK - PAL_CLOCK) / HS_CLOCK;
const NTSC_CLOCK_SCALE: f64 = (NTSC_CLOCK - HS_CLOCK) / HS_CLOCK;

const PAL_FREQ_SCALE: u32 = ((HS_CLOCK - PAL_CLOCK) * 65536.0 / PAL_CLOCK) as u32;
const NTSC_FREQ_SCALE: u32 = ((NTSC_CLOCK - HS_CLOCK) * 65536.0 / NTSC_CLOCK) as u32;

pub struct ClockAdjust {
    total_cycles_to_stretch: f64,
    freq: [u32; 3*8],
    last_freq: [u32; 3*8],
    clock: SidClock
}

impl ClockAdjust {
    pub fn new() -> ClockAdjust {
        ClockAdjust {
            total_cycles_to_stretch: 0.0,
            freq: [0; 3*8],
            last_freq: [0; 3*8],
            clock: SidClock::Pal
        }
    }

    pub fn init(&mut self, clock: SidClock) {
        self.total_cycles_to_stretch = 0.0;
        self.freq = [0; 3 * 8];
        self.last_freq = [0; 3 * 8];
        self.clock = clock;
    }

    pub fn adjust_cycles(&mut self, cycles: u32) -> u32 {
        let cycles = cycles as f64;

        if self.clock == SidClock::Pal {
            let cycles_to_stretch = cycles * PAL_CLOCK_SCALE;
            self.total_cycles_to_stretch += cycles_to_stretch;

            if self.total_cycles_to_stretch >= 1.0 {
                let stretch_rounded = self.total_cycles_to_stretch.trunc();
                self.total_cycles_to_stretch -= stretch_rounded;
                return (cycles + stretch_rounded) as u32;
            }
        } else {
            let cycles_to_stretch = cycles * NTSC_CLOCK_SCALE;
            self.total_cycles_to_stretch += cycles_to_stretch;

            if self.total_cycles_to_stretch >= 1.0 {
                if cycles + 1.0 > self.total_cycles_to_stretch {
                    let stretch_rounded = self.total_cycles_to_stretch.trunc();
                    self.total_cycles_to_stretch -= stretch_rounded;
                    return (cycles - stretch_rounded) as u32;
                } else if cycles as u32 > MIN_CYCLE_SID_WRITE {
                    self.total_cycles_to_stretch -= cycles - MIN_CYCLE_SID_WRITE as f64;
                    return MIN_CYCLE_SID_WRITE;
                }
            }
        }
        cycles as u32
    }

    pub fn get_last_scaled_freq(&self, voice_index: u8) -> u32 {
        self.last_freq[voice_index as usize]
    }

    pub fn scale_frequency(&mut self, voice_index: u8) -> u32 {
        let freq = self.freq[voice_index as usize];
        let scaled_freq = if self.clock == SidClock::Ntsc {
            let freq = freq + ((freq * NTSC_FREQ_SCALE) >> 16);
            min(freq, 0xffff)
        } else {
            freq - ((freq * PAL_FREQ_SCALE) >> 16)
        };

        self.last_freq[voice_index as usize] = scaled_freq;
        scaled_freq
    }

    pub fn update_frequency(&mut self, voice_index: u8, reg: u8, data: u8) {
        let freq = self.freq[voice_index as usize];
        let freq = if reg == 0 {
            (freq & 0xff00) + data as u32
        } else {
            (freq & 0x00ff) + ((data as u32) << 8)
        };
        self.freq[voice_index as usize] = freq;
    }
}
