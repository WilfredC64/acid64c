// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use crossterm_cursor::TerminalCursor;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

pub struct Clock {
    cursor: TerminalCursor,
    seconds_counter: Arc<AtomicIsize>,
    timer: timer::Timer,
    previous_count: isize,
    guard: Option<timer::Guard>,
    clock_length: u16,
    paused: Arc<AtomicBool>
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            cursor: TerminalCursor::new(),
            seconds_counter: Arc::new(AtomicIsize::new(0)),
            timer: timer::Timer::new(),
            previous_count: -1,
            guard: None,
            clock_length: 0,
            paused: Arc::new(AtomicBool::new(false))
        }
    }

    pub fn set_clock_display_length(&mut self, clock_length: usize) {
        self.clock_length = clock_length as u16;
    }

    pub fn start(&mut self) {
        self.pause(false);
        self.seconds_counter.store(0, Ordering::Relaxed);

        let counter = Arc::clone(&self.seconds_counter);
        let paused = Arc::clone(&self.paused);

        let guard = {
            self.timer.schedule_repeating(chrono::Duration::milliseconds(1000), move || {
                if !paused.load(Ordering::Relaxed) {
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            })
        };
        self.guard = Some(guard);

        let _ = self.cursor.hide();
        let _ = self.cursor.move_left(self.clock_length);
        let _ = self.cursor.save_position();
    }

    pub fn pause(&mut self, pause: bool) {
        self.paused.store(pause, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.guard = None;
        let _ = self.cursor.move_right(self.clock_length);
        let _ = self.cursor.show();
    }

    pub fn refresh_clock(&mut self) {
        let seconds = self.seconds_counter.load(Ordering::Relaxed);

        if self.previous_count != seconds {
            self.previous_count = seconds;

            let time = Clock::convert_seconds_to_time_string(seconds as u32, false);
            print!("{}", time);
            let _ = self.cursor.reset_position();
        }
    }

    pub fn convert_seconds_to_time_string(seconds_total: u32, display_hours: bool) -> String {
        let seconds = seconds_total % 60;
        let hours = seconds_total / 3600;
        let minutes = seconds_total / 60 - hours * 60;

        if !display_hours {
            format!("{:02}:{:02}", minutes, seconds)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        }
    }
}