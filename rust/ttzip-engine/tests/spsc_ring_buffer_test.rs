// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use ttzip_engine::runtime::ring_buffer::spsc::SpscRingBuffer;

#[test]
fn test_spsc_ring_buffer_concurrent_stress() {
    const COUNT: usize = 100_000;
    let ring = SpscRingBuffer::<usize>::new(1024);
    let (producer, consumer) = ring.split();

    let done = Arc::new(AtomicBool::new(false));
    let done_c = Arc::clone(&done);

    let t_prod = thread::spawn(move || {
        for i in 0..COUNT {
            let mut val = i;
            loop {
                match producer.push(val) {
                    Ok(()) => break,
                    Err(returned) => {
                        val = returned;
                        std::hint::spin_loop();
                    }
                }
            }
        }
        done_c.store(true, Ordering::Release);
    });

    let t_cons = thread::spawn(move || {
        let mut received = 0;
        let mut expected = 0;
        while received < COUNT {
            if let Some(val) = consumer.pop() {
                assert_eq!(val, expected);
                expected += 1;
                received += 1;
            } else {
                std::hint::spin_loop();
            }
        }
    });

    t_prod.join().unwrap();
    t_cons.join().unwrap();
}
