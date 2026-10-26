// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Per-invocation test receipts at the actual compression entry boundary.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

thread_local! {
    static CURRENT: RefCell<Option<Arc<Probe>>> = const { RefCell::new(None) };
}

#[derive(Default)]
pub(super) struct Stats {
    pub active: usize,
    pub peak: usize,
    pub threads: HashSet<ThreadId>,
    pub timed_out: bool,
}

pub(super) struct Probe {
    pub stats: Mutex<Stats>,
    entered: mpsc::Sender<String>,
    released: Mutex<bool>,
    release: Condvar,
}

pub(super) fn current() -> Option<Arc<Probe>> {
    CURRENT.with(|current| current.borrow().clone())
}

pub(super) struct Registration(Option<Arc<Probe>>);

impl Registration {
    pub fn install(probe: Arc<Probe>) -> Self {
        Self(CURRENT.with(|current| current.replace(Some(probe))))
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        CURRENT.with(|current| current.replace(self.0.take()));
    }
}

impl Probe {
    pub fn new() -> (Arc<Self>, mpsc::Receiver<String>) {
        let (entered, receiver) = mpsc::channel();
        (
            Arc::new(Self {
                stats: Mutex::new(Stats::default()),
                entered,
                released: Mutex::new(false),
                release: Condvar::new(),
            }),
            receiver,
        )
    }

    pub fn release_all(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }

    pub fn enter(self: &Arc<Self>, name: &str) -> Span {
        {
            let mut stats = self.stats.lock().unwrap();
            stats.active += 1;
            stats.peak = stats.peak.max(stats.active);
            stats.threads.insert(std::thread::current().id());
        }
        let _ = self.entered.send(name.to_owned());
        self.wait_for_release();
        Span(self.clone())
    }

    fn wait_for_release(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut released = self.released.lock().unwrap();
        while !*released {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.stats.lock().unwrap().timed_out = true;
                *released = true;
                self.release.notify_all();
                break;
            }
            released = self.release.wait_timeout(released, remaining).unwrap().0;
        }
    }
}

pub(super) struct Span(Arc<Probe>);

impl Drop for Span {
    fn drop(&mut self) {
        self.0.stats.lock().unwrap().active -= 1;
    }
}

pub(super) struct ReleaseOnDrop(pub Arc<Probe>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release_all();
    }
}
