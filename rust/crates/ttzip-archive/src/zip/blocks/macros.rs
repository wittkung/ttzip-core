// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Declarative little-endian SerDe code generation macros for ZIP binary blocks.

/// Declarative macro to define fixed-size binary structures with automatic `Pod`, `FixedSizeBlock`,
/// and endian conversion implementation.
#[macro_export]
macro_rules! to_and_from_le {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            magic: $magic:expr,
            options: $options:ident,
            $(
                $(#[$field_meta:meta])*
                $fvis:vis $field:ident : $type:ident
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C, packed)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $fvis $field : $type,
            )*
        }

        /// Strongly-typed parameters and options container for constructing [`$name`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $options {
            $(
                $(#[$field_meta])*
                $fvis $field : $type,
            )*
        }

        impl From<$options> for $name {
            #[inline(always)]
            fn from(opt: $options) -> Self {
                Self {
                    $( $field: opt.$field, )*
                }
            }
        }

        impl From<$name> for $options {
            #[inline(always)]
            fn from(block: $name) -> Self {
                Self {
                    $( $field: block.$field, )*
                }
            }
        }

        unsafe impl $crate::zip::blocks::Pod for $name {}

        impl $name {
            /// Fixed byte size of the block.
            pub const SIZE: usize = std::mem::size_of::<Self>();

            /// 4-byte signature magic constant for this block.
            pub const MAGIC: u32 = $magic;

            /// Constructs a new instance of the block from strongly-typed options.
            #[inline(always)]
            pub const fn new(options: $options) -> Self {
                Self {
                    $( $field: options.$field, )*
                }
            }

            /// Constructs a new instance of the block from strongly-typed options.
            #[inline(always)]
            pub const fn from_options(options: $options) -> Self {
                Self::new(options)
            }

            /// Converts all numerical fields from little-endian to host-endian.
            #[allow(clippy::wrong_self_convention)]
            #[inline(always)]
            pub fn from_le(self) -> Self {
                Self {
                    $(
                        $field: $type::from_le(self.$field),
                    )*
                }
            }

            /// Converts all numerical fields from host-endian to little-endian.
            #[inline(always)]
            pub fn to_le(self) -> Self {
                Self {
                    $(
                        $field: $type::to_le(self.$field),
                    )*
                }
            }
        }

        impl $crate::zip::blocks::FixedSizeBlock for $name {
            const MAGIC: u32 = $magic;

            #[inline(always)]
            fn from_le(self) -> Self {
                self.from_le()
            }

            #[inline(always)]
            fn to_le(self) -> Self {
                self.to_le()
            }
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            magic: $magic:expr,
            $(
                $(#[$field_meta:meta])*
                $fvis:vis $field:ident : $type:ident
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr(C, packed)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $name {
            $(
                $(#[$field_meta])*
                $fvis $field : $type,
            )*
        }

        unsafe impl $crate::zip::blocks::Pod for $name {}

        impl $name {
            /// Fixed byte size of the block.
            pub const SIZE: usize = std::mem::size_of::<Self>();

            /// 4-byte signature magic constant for this block.
            pub const MAGIC: u32 = $magic;

            /// Constructs a new instance of the block with the given field values.
            #[inline(always)]
            pub const fn new($( $field : $type ),*) -> Self {
                Self {
                    $( $field, )*
                }
            }

            /// Converts all numerical fields from little-endian to host-endian.
            #[allow(clippy::wrong_self_convention)]
            #[inline(always)]
            pub fn from_le(self) -> Self {
                Self {
                    $(
                        $field: $type::from_le(self.$field),
                    )*
                }
            }

            /// Converts all numerical fields from host-endian to little-endian.
            #[inline(always)]
            pub fn to_le(self) -> Self {
                Self {
                    $(
                        $field: $type::to_le(self.$field),
                    )*
                }
            }
        }

        impl $crate::zip::blocks::FixedSizeBlock for $name {
            const MAGIC: u32 = $magic;

            #[inline(always)]
            fn from_le(self) -> Self {
                self.from_le()
            }

            #[inline(always)]
            fn to_le(self) -> Self {
                self.to_le()
            }
        }
    };
}

pub use to_and_from_le;
