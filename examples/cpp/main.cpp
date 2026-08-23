// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#include <iostream>
#include <ttzip.hpp>

int main() {
    std::cout << "TTZip Modern C++20 SDK Example (v" << ttzip::version() << ")\n";
    
    std::string text = "Modern C++20 Archiving Example with RAII";
    std::span<const uint8_t> span(reinterpret_cast<const uint8_t*>(text.data()), text.size());
    std::cout << "Computed CRC32: 0x" << std::hex << ttzip::crc32(span) << std::dec << "\n";
    
    return 0;
}
