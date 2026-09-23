// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Atomic cancellation tokens and cross-thread notification channels.
pub use ttzip_types::{CancellationReason, CancellationToken, TTZipStatus};


#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cancellation_token_basic() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        assert_eq!(token.check(), Ok(()));
        assert_eq!(token.cancellation_reason(), None);

        token.cancel(CancellationReason::Timeout);
        assert!(token.is_cancelled());
        assert_eq!(token.check(), Err(TTZipStatus::Cancelled));
        assert_eq!(token.cancellation_reason(), Some(CancellationReason::Timeout));
    }

    #[test]
    fn test_cancellation_token_cross_thread() {
        let token = CancellationToken::new();
        let token_clone = token.child_token();

        let handle = thread::spawn(move || {
            while !token_clone.is_cancelled() {
                thread::yield_now();
            }
            token_clone.cancellation_reason()
        });

        thread::sleep(std::time::Duration::from_millis(10));
        token.cancel(CancellationReason::UserRequested);

        let reason = handle.join().expect("join");
        assert_eq!(reason, Some(CancellationReason::UserRequested));
    }
}
