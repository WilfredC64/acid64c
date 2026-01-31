// Copyright (C) 2026 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crate::player::sid_device::{SidWrite, DUMMY_REG};

const MIN_CYCLE_SID_WRITE: u16 = 0x08;
const TIME_FOR_ADSR_TO_STABILIZE: u16 = 0x9c40;
const TIME_BETWEEN_TEST_BIT_FLIP: u16 = 0x32;

const RESET_REGS: &[u8] = &[
    0x02, 0x03, 0x04, 0x05, 0x06,
    0x09, 0x0a, 0x0b, 0x0c, 0x0d,
    0x10, 0x11, 0x12, 0x13, 0x14,
    0x15, 0x16, 0x17, 0x19,
];

pub fn silent_sid_sequence(base_reg: u8, write_volume: bool) -> Vec<SidWrite> {
    let mut writes = vec![];

    let target_regs = [
        0x01, 0x00, 0x08, 0x07, 0x0f, 0x0e,
        0x04, 0x05, 0x06,
        0x0b, 0x0c, 0x0d,
        0x12, 0x13, 0x14
    ];

    for reg in target_regs {
        push_write(&mut writes, base_reg + reg, 0);
    }

    if write_volume {
        push_write(&mut writes, base_reg + 0x18, 0);
    }
    writes
}

pub fn reset_all_sids_sequence(number_of_sids: i32, add_time_to_stabilize: bool) -> Vec<SidWrite> {
    let mut sid_writes = vec![];
    for sid_nr in 0..number_of_sids {
        sid_writes.append(&mut reset_sid_socket((sid_nr * 0x20) as u8));
    }
    if add_time_to_stabilize {
        sid_writes.push(SidWrite { cycles: TIME_FOR_ADSR_TO_STABILIZE, reg: DUMMY_REG, data: 0x00 });
    }
    sid_writes
}

pub fn reset_sid_sequence(base_reg: u8, add_time_to_stabilize: bool) -> Vec<SidWrite> {
    let mut sid_writes = reset_sid_socket(base_reg);
    if add_time_to_stabilize {
        sid_writes.push(SidWrite { cycles: TIME_FOR_ADSR_TO_STABILIZE, reg: DUMMY_REG, data: 0x00 });
    }
    sid_writes
}

fn reset_sid_socket(base_reg: u8) -> Vec<SidWrite> {
    let mut writes = silent_sid_sequence(base_reg, false);

    for &reg in RESET_REGS {
        push_write(&mut writes, base_reg + reg, 0xff);
        push_write(&mut writes, base_reg + reg, 0x08);
    }

    writes.push(SidWrite { cycles: TIME_BETWEEN_TEST_BIT_FLIP, reg: base_reg + DUMMY_REG, data: 0x00 });

     for &reg in RESET_REGS {
        push_write(&mut writes, base_reg + reg, 0x00);
    }

    writes
}

fn push_write(writes: &mut Vec<SidWrite>, reg: u8, data: u8) {
    writes.push(SidWrite {
        cycles: MIN_CYCLE_SID_WRITE,
        reg,
        data,
    });
}