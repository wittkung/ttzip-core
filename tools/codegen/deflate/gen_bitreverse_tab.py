#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

def reverse_byte(v):
    return sum(1 << (7 - bit) for bit in range(8) if (v & (1 << bit)) != 0)

tab = [reverse_byte(v) for v in range(256)]

print('static const u8 bitreverse_tab[256] = {')
for i in range(0, len(tab), 8):
    print('\t', end='')
    for j, v in enumerate(tab[i:i+8]):
        print(f'0x{v:02x},', end='')
        if j == 7:
            print('')
        else:
            print(' ', end='')
print('};')
