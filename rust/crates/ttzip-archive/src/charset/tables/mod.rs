// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Language frequency distributions and transition probability tables for CJK character sets.

pub mod big5;
pub mod euc_kr;
pub mod gb18030;
pub mod shift_jis;

pub use big5::score_big5_2byte;
pub use euc_kr::score_euc_kr_2byte;
pub use gb18030::score_gb18030_2byte;
pub use shift_jis::score_shift_jis_2byte;
