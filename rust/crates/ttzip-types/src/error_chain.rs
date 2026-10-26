// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-allocation causal error chain iterator traversing `std::error::Error::source`.

/// Zero-allocation causal error chain iterator traversing `std::error::Error::source`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ErrorChain<'a> {
    current: Option<&'a (dyn std::error::Error + 'static)>,
}

impl<'a> ErrorChain<'a> {
    /// Creates an empty causal error chain iterator.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self { current: None }
    }

    /// Creates a causal error chain iterator starting from the given error.
    #[inline]
    #[must_use]
    pub fn new(err: &'a (dyn std::error::Error + 'static)) -> Self {
        Self { current: Some(err) }
    }
}

impl<'a> Iterator for ErrorChain<'a> {
    type Item = &'a (dyn std::error::Error + 'static);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.current = current.source();
        Some(current)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.current.is_some() {
            (1, None)
        } else {
            (0, Some(0))
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        let mut last = self.current?;
        while let Some(cause) = last.source() {
            last = cause;
        }
        Some(last)
    }
}

impl<'a> std::iter::FusedIterator for ErrorChain<'a> {}
