// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Media Transfer Protocol (MTP 1.1) stack, operations, and device storage driver.

pub mod driver;
pub mod operations;
pub mod protocol;

// Re-export primary types for ergonomic usage
pub use driver::MtpDeviceDriver;
pub use operations::{
    close_session, delete_object, get_object_handles, get_object_info, get_partial_object_64,
    get_storage_ids, get_storage_info, open_session, send_object, send_object_info, MtpInOut,
    DEFAULT_MTP_TIMEOUT, STREAM_MTP_TIMEOUT,
};
pub use protocol::{
    decode_mtp_string, decode_mtp_timestamp, decode_u32_array, encode_mtp_string,
    encode_mtp_timestamp, encode_u32_array, MtpContainer, MtpObjectInfo, MtpStorageInfo,
    CONTAINER_TYPE_COMMAND, CONTAINER_TYPE_DATA, CONTAINER_TYPE_EVENT, CONTAINER_TYPE_RESPONSE,
    FORMAT_ASSOCIATION, FORMAT_UNDEFINED, MTP_CONTAINER_HEADER_SIZE, MTP_PARENT_ROOT,
    OP_CLOSE_SESSION, OP_DELETE_OBJECT, OP_GET_OBJECT_HANDLES, OP_GET_OBJECT_INFO,
    OP_GET_PARTIAL_OBJECT_64, OP_GET_STORAGE_IDS, OP_GET_STORAGE_INFO, OP_OPEN_SESSION,
    OP_SEND_OBJECT, OP_SEND_OBJECT_INFO, RESP_GENERAL_ERROR, RESP_INVALID_OBJECT_HANDLE,
    RESP_INVALID_STORAGE_ID, RESP_OK,
};
