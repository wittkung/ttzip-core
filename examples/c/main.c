// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#include <stdio.h>
#include <ttzip.h>

int main(void) {
    printf("TTZip C11 Native SDK Example (v%s)\n", ttzip_version());
    
    const char *payload = "High performance C11 data payload";
    uint32_t crc = ttzip_crc32((const uint8_t *)payload, 33);
    printf("CRC32 Checksum: 0x%08X\n", crc);
    
    return 0;
}
