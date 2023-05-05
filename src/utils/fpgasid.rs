// Copyright (C) 2023 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crate::player::sid_device::SidModel;

const MIN_CYCLE_SID_WRITE: u32 = 8;

pub struct FpgaSidWrite {
    pub cycles: u32,
    pub reg: u8,
    pub data: u8,
}

pub fn configure_fpgasid(sid_model: &SidModel) -> Vec<FpgaSidWrite> {
    let mut sid_writes: Vec<FpgaSidWrite> = vec![];
    enable_config_mode(&mut sid_writes);
    set_sid_model(sid_model, &mut sid_writes);
    disable_config_mode(&mut sid_writes);
    sid_writes
}

fn set_sid_model(sid_model: &SidModel, sid_writes: &mut Vec<FpgaSidWrite>) {
    match sid_model {
        SidModel::Mos6581 => sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: 0x40 }),
        SidModel::Mos8580 => sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x1f, data: 0xff })
    }
}

fn enable_config_mode(sid_writes: &mut Vec<FpgaSidWrite>) {
    sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x19, data: 0x80 });
    sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x1a, data: 0x65 });
}

fn disable_config_mode(sid_writes: &mut Vec<FpgaSidWrite>) {
    sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x19, data: 0 });
    sid_writes.push(FpgaSidWrite { cycles: MIN_CYCLE_SID_WRITE, reg: 0x1a, data: 0 });
}
