// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crate::player::sid_device::SidModel;

const MIN_CYCLE_SID_WRITE: u32 = 8;

pub struct ArmSidWrite {
    pub cycles: u32,
    pub reg: u8,
    pub data: u8,
}

pub struct SidFilter {
    pub filter_strength_6581: u8,
    pub filter_lowest_freq_6581: u8,
    pub filter_central_freq_8580: u8,
    pub filter_lowest_freq_8580: u8
}

pub fn configure_armsid(sid_model: &SidModel, sid_filter: &SidFilter) -> Vec<ArmSidWrite> {
    let mut sid_writes = vec![];
    set_sid_model(sid_model, &mut sid_writes);
    config_filter(sid_model, sid_filter, &mut sid_writes);
    disable_config(&mut sid_writes);
    sid_writes
}

fn set_sid_model(sid_model: &SidModel, sid_writes: &mut Vec<ArmSidWrite>) {
    enable_config(sid_writes);

    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1d, data: b'S'});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});

    match sid_model {
        SidModel::Mos6581 => sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: b'6'}),
        SidModel::Mos8580 => sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: b'8'})
    }
}

fn config_filter(sid_model: &SidModel, sid_filter: &SidFilter, sid_writes: &mut Vec<ArmSidWrite>) {
    enable_config(sid_writes);

    let filter_strength_6581 = (sid_filter.filter_strength_6581 + 0x09) & 0x0f;
    let filter_lowest_freq_6581 = (sid_filter.filter_lowest_freq_6581 + 0x0f) & 0x0f;
    let filter_central_freq_8580 = (sid_filter.filter_central_freq_8580 + 0x0d) & 0x0f;
    let filter_lowest_freq_8580 = (sid_filter.filter_lowest_freq_8580 + 0x0d) & 0x0f;

    match sid_model {
        SidModel::Mos6581 => {
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: filter_strength_6581 | 0x80});
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});
            sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});

            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: filter_lowest_freq_6581 | 0x90});
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});
            sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});
        },
        SidModel::Mos8580 => {
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: filter_central_freq_8580 | 0xa0});
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});
            sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});

            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: filter_lowest_freq_8580 | 0xb0});
            sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});
            sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});
        }
    }

    save_to_ram(sid_writes);
}

fn enable_config(sid_writes: &mut Vec<ArmSidWrite>) {
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1d, data: b'S'});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'I'});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: b'D'});
    sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});
}

fn disable_config(sid_writes: &mut Vec<ArmSidWrite>) {
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1d, data: 0});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: 0});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: 0});
    sid_writes.push(ArmSidWrite{ cycles: 20_000, reg: 0x1e, data: 0});
}

fn save_to_ram(sid_writes: &mut Vec<ArmSidWrite>) {
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: 0xc0});
    sid_writes.push(ArmSidWrite{ cycles: MIN_CYCLE_SID_WRITE, reg: 0x1e, data: b'E'});
    sid_writes.push(ArmSidWrite{ cycles: 1_000, reg: 0x1e, data: 0});
}
