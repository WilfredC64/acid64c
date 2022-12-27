// Copyright (C) 2019 Wilfred Bos
// Licensed under the GNU GPL v3 license. See the LICENSE file for the terms and conditions.

use std::io::stdout;
use crossterm::cursor::{Hide, MoveLeft, MoveRight, SavePosition, RestorePosition, Show};
use crossterm::execute;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct Clock {
    counter: Arc<AtomicUsize>,
    timer: timer::Timer,
    previous_count: usize,
    guard: Option<timer::Guard>,
    clock_length: u16,
    paused: Arc<AtomicBool>
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            counter: Arc::new(AtomicUsize::new(0)),
            timer: timer::Timer::new(),
            previous_count: 0,
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
        self.counter.store(0, Ordering::Relaxed);

        let counter = Arc::clone(&self.counter);
        let paused = Arc::clone(&self.paused);

        let guard = {
            self.timer.schedule_repeating(chrono::Duration::milliseconds(20), move || {
                if !paused.load(Ordering::Relaxed) {
                    counter.fetch_add(20, Ordering::Relaxed);
                }
            })
        };
        self.guard = Some(guard);

        execute!(stdout(), Hide, MoveLeft(self.clock_length), SavePosition).unwrap();
    }

    pub fn set_clock(&mut self, millis: usize) {
        self.counter.store(millis, Ordering::Relaxed);
    }

    pub fn pause(&mut self, pause: bool) {
        self.paused.store(pause, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.guard = None;
        execute!(stdout(), MoveRight(self.clock_length), Show).unwrap();
    }

    pub fn refresh_clock(&mut self) {
        let millis = self.counter.load(Ordering::Relaxed);

        if self.previous_count != millis {
            self.previous_count = millis;

            let time = Clock::convert_seconds_to_time_string((millis / 1000) as u32, false);
            print!("{}", time);
            execute!(stdout(), RestorePosition).unwrap();
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
