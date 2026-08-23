// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
package com.ttzip.examples;

import com.ttzip.TTZip;
import java.util.List;

public class Quickstart {
    public static void main(String[] args) throws Exception {
        System.out.println("⚡️ TTZip Java 21+ SDK Example (v" + TTZip.version() + ")");
        
        byte[] payload = "Enterprise Java 21 FFM Native Payload".getBytes();
        int crc = TTZip.crc32(payload);
        System.out.println("CRC-32 Checksum: 0x" + Integer.toHexString(crc).toUpperCase());
        
        TTZip.compress(List.of("README.md"), "demo_java.zip");
        TTZip.extract("demo_java.zip", "extracted_java");
        System.out.println("Successfully created and extracted archive.");
    }
}
