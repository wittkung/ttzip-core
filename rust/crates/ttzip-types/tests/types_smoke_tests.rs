// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

use ttzip_types::*;

#[test]
fn test_types_smoke() {
    assert_eq!(TTZipArchiveFormat::Zip.as_str(), "zip");
    assert_eq!(TTZipArchiveFormat::SevenZip.as_str(), "7z");
    assert_eq!(TTZipStatus::Ok.to_i32(), 0);
    assert!(TTZipStatus::Ok.is_ok());
    assert!(TTZipStatus::ErrInvalidParam.is_err());
    assert_eq!(TTZIP_ABI_VERSION, 2);

    let buf = TTZipBufferRef::from_slice(b"hello");
    assert_eq!(unsafe { buf.as_slice() }, b"hello");

    let mut vec = vec![1, 2, 3];
    let mut buf_mut = TTZipBufferMut::from_vec(&mut vec);
    assert_eq!(unsafe { buf_mut.as_slice() }, &[1, 2, 3]);
    unsafe { buf_mut.as_mut_slice()[0] = 42; }
    assert_eq!(vec[0], 42);

    let bytes = bytes::Bytes::from_static(b"world");
    let buf_bytes = TTZipBufferRef::from_bytes(&bytes);
    assert_eq!(unsafe { buf_bytes.as_slice() }, b"world");
}
