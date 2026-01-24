// Copyright (C) 2019 - 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::cmp::min;
use super::sid_device::SidClock;
use super::MIN_CYCLE_SID_WRITE;

const ONE_MH_CLOCK: f64 = 1_000_000.0;
const PAL_CLOCK: f64 = 17_734_475.0 / 18.0;
const NTSC_CLOCK: f64 = 14_318_180.0 / 14.0;

const PAL_CLOCK_SCALE: f64 = (ONE_MH_CLOCK - PAL_CLOCK) / ONE_MH_CLOCK;
const NTSC_CLOCK_SCALE: f64 = (NTSC_CLOCK - ONE_MH_CLOCK) / ONE_MH_CLOCK;

const PAL_FREQ_SCALE: u32 = (((PAL_CLOCK - ONE_MH_CLOCK) * 65_536.0 / PAL_CLOCK) + 65_536.0) as u32;
const NTSC_FREQ_SCALE: u32 = (((NTSC_CLOCK - ONE_MH_CLOCK) * 65_536.0 / NTSC_CLOCK) + 65_536.0) as u32;

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
        let mut cycles = cycles as f64;

        if self.clock == SidClock::Pal {
            self.total_cycles_to_stretch += cycles * PAL_CLOCK_SCALE;

            if self.total_cycles_to_stretch >= 1.0 {
                let stretch_rounded = self.total_cycles_to_stretch.trunc();
                self.total_cycles_to_stretch -= stretch_rounded;
                cycles += stretch_rounded;
            }
        } else {
            self.total_cycles_to_stretch += cycles * NTSC_CLOCK_SCALE;

            if self.total_cycles_to_stretch >= 1.0 {
                if cycles > self.total_cycles_to_stretch {
                    let stretch_rounded = self.total_cycles_to_stretch.trunc();
                    self.total_cycles_to_stretch -= stretch_rounded;
                    cycles -= stretch_rounded;
                } else {
                    self.total_cycles_to_stretch -= cycles;
                    cycles = 0.0;
                }
            }
        }

        if (cycles as u32) < MIN_CYCLE_SID_WRITE {
            if self.clock == SidClock::Pal {
                self.total_cycles_to_stretch -= MIN_CYCLE_SID_WRITE as f64 - cycles;
            } else {
                self.total_cycles_to_stretch += MIN_CYCLE_SID_WRITE as f64 - cycles;
            }
            return MIN_CYCLE_SID_WRITE;
        }

        cycles as u32
    }

    pub fn get_last_scaled_freq(&self, voice_index: u8) -> u32 {
        self.last_freq[voice_index as usize]
    }

    pub fn scale_frequency(&mut self, voice_index: u8) -> u32 {
        let freq = self.freq[voice_index as usize];

        let scaled_freq = match self.clock {
            SidClock::Ntsc => min((freq.saturating_mul(NTSC_FREQ_SCALE)) >> 16, 0xffff),
            _ => (freq.saturating_mul(PAL_FREQ_SCALE)) >> 16
        };

        self.last_freq[voice_index as usize] = scaled_freq;
        scaled_freq
    }

    pub fn update_frequency(&mut self, voice_index: u8, reg: u8, data: u8) {
        let freq = &mut self.freq[voice_index as usize];
        *freq = match reg {
            0 => (*freq & 0xff00) | data as u32,
            _ => (*freq & 0x00ff) | ((data as u32) << 8),
        };
    }
}
