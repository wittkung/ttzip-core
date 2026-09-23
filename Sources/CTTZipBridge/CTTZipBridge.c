// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

#include "CTTZipBridge.h"
#include <stddef.h>

// MARK: - Legacy Device Android UniFFI Compatibility Shims

uint16_t uniffi_ttzip_engine_checksum_func_uniffi_download_file(void) { return 49260; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_extract_to_device(void) { return 48815; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_inspect_remote_archive(void) { return 9315; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_list_device_directory(void) { return 6755; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_open_device(void) { return 34322; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_pair_wireless_device(void) { return 8415; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_scan_usb_devices(void) { return 40150; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_start_hotplug_monitoring(void) { return 36397; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_stop_hotplug_monitoring(void) { return 37312; }
uint16_t uniffi_ttzip_engine_checksum_func_uniffi_upload_file(void) { return 934; }
uint16_t uniffi_ttzip_engine_checksum_method_uniffideviceeventlistener_on_devices_changed(void) { return 45810; }

static RustBuffer alloc_empty_seq(void) {
    RustCallStatus status = {0, {0, 0, NULL}};
    RustBuffer b = ffi_ttzip_engine_rustbuffer_alloc(4, &status);
    if (b.data) {
        b.data[0] = 0;
        b.data[1] = 0;
        b.data[2] = 0;
        b.data[3] = 0;
        b.len = 4;
    }
    return b;
}

static void set_error_cancelled(RustCallStatus *status) {
    if (!status) return;
    status->code = 1;
    RustCallStatus alloc_status = {0, {0, 0, NULL}};
    RustBuffer b = ffi_ttzip_engine_rustbuffer_alloc(4, &alloc_status);
    if (b.data) {
        b.data[0] = 0;
        b.data[1] = 0;
        b.data[2] = 0;
        b.data[3] = 7; // TTZipError.Cancelled
        b.len = 4;
        status->errorBuf = b;
    }
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_download_file(RustBuffer device_id, RustBuffer remote_path, RustBuffer local_path, RustCallStatus *_Nonnull out_status) {
    (void)device_id; (void)remote_path; (void)local_path;
    set_error_cancelled(out_status);
    RustBuffer b = {0, 0, NULL};
    return b;
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_extract_to_device(RustBuffer archive_path, RustBuffer destination_device_id, RustBuffer destination_dir, RustCallStatus *_Nonnull out_status) {
    (void)archive_path; (void)destination_device_id; (void)destination_dir;
    set_error_cancelled(out_status);
    RustBuffer b = {0, 0, NULL};
    return b;
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_inspect_remote_archive(RustBuffer device_id, RustBuffer archive_path, RustCallStatus *_Nonnull out_status) {
    (void)device_id; (void)archive_path;
    if (out_status) { out_status->code = 0; }
    return alloc_empty_seq();
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_list_device_directory(RustBuffer device_id, RustBuffer path, RustCallStatus *_Nonnull out_status) {
    (void)device_id; (void)path;
    if (out_status) { out_status->code = 0; }
    return alloc_empty_seq();
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_open_device(RustBuffer device_id, RustCallStatus *_Nonnull out_status) {
    (void)device_id;
    set_error_cancelled(out_status);
    RustBuffer b = {0, 0, NULL};
    return b;
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_pair_wireless_device(RustBuffer host, uint16_t port, RustBuffer pin, RustCallStatus *_Nonnull out_status) {
    (void)host; (void)port; (void)pin;
    set_error_cancelled(out_status);
    RustBuffer b = {0, 0, NULL};
    return b;
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_scan_usb_devices(RustCallStatus *_Nonnull out_status) {
    if (out_status) { out_status->code = 0; }
    return alloc_empty_seq();
}

void uniffi_ttzip_engine_fn_func_uniffi_start_hotplug_monitoring(uint64_t listener, RustCallStatus *_Nonnull out_status) {
    (void)listener;
    if (out_status) { out_status->code = 0; }
}

void uniffi_ttzip_engine_fn_func_uniffi_stop_hotplug_monitoring(RustCallStatus *_Nonnull out_status) {
    if (out_status) { out_status->code = 0; }
}

RustBuffer uniffi_ttzip_engine_fn_func_uniffi_upload_file(RustBuffer device_id, RustBuffer local_path, RustBuffer remote_dir, RustCallStatus *_Nonnull out_status) {
    (void)device_id; (void)local_path; (void)remote_dir;
    set_error_cancelled(out_status);
    RustBuffer b = {0, 0, NULL};
    return b;
}

void uniffi_ttzip_engine_fn_init_callback_vtable_uniffideviceeventlistener(UniffiVTableCallbackInterfaceUniFfiDeviceEventListener *_Nonnull vtable) {
    (void)vtable;
}
