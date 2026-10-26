// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#include "zbuild.h"
#include "zmemory.h"
#include "deflate.h"
#include "fallback_builtins.h"

#if defined(ARM_NEON)
#include "neon_intrins.h"

/*
 * ----------------------------------------------------------------------------
 * 1. 体系结构检测
 * ----------------------------------------------------------------------------
 * 仅在 AArch64 (64位 ARM) 下启用后变址加载。
 */
#if defined(ARCH_ARM) && defined(ARCH_64BIT) && (!defined(_MSC_VER) || defined(__clang__))
#  define COMPARE256_NEON_POSTINDEX
#endif

/*
 * ----------------------------------------------------------------------------
 * 2. 原版核心比对函数 (compare256_neon_static)
 * ----------------------------------------------------------------------------
 */
Z_FORCEINLINE static uint32_t compare256_neon_static(const uint8_t *src0, const uint8_t *src1) {

    uint32_t len = 0;

    /*
     * 计算两指针间距。
     * 后续指令只需让 src1 递增，src0 自动通过硬件变址寻址定位。
     */
#ifdef COMPARE256_NEON_POSTINDEX
    intptr_t offset = (intptr_t)src0 - (intptr_t)src1;
#endif

    /*
     * 单向量 16 字节固定步长循环
     * 每一轮迭代处理 16 个字节，最多循环 16 轮 (16 * 16 = 256 字节)
     */
    do {
        uint8x16_t a;
        uint8x16_t b;
        uint8x16_t cmp;
        uint64_t lane;

        /*
         * 步骤 1: 硬件级后变址加载 16 字节
         * 从 (src1 + offset) 读取 16 字节到寄存器 a (即 src0)
         * 从 src1 读取 16 字节到寄存器 b，并自动递增 src1 指针 16 字节
         */
#ifdef COMPARE256_NEON_POSTINDEX
        __asm__("ldr %q0, [%2, %3]
	"
                "ldr %q1, [%2], #16"
                : "=w"(a), "=w"(b), "+r"(src1)
                : "r"(offset)
                : "memory");
#else
        a = vld1q_u8(src0);
        b = vld1q_u8(src1);
        src0 += 16;
        src1 += 16;
#endif

        /*
         * 步骤 2: 按位异或 (XOR) 找出差异
         * 相同位为 0，不同位为非 0
         */
        cmp = veorq_u8(a, b);

        /*
         * 步骤 3: 检查低 64 位 (前 8 字节)
         * 提取低 8 字节到 64 位通用寄存器
         */
        lane = vgetq_lane_u64(vreinterpretq_u64_u8(cmp), 0);
        if (lane) {
            return len + zng_first_diff_byte64(lane);
        }

        len += 8;

        /*
         * 步骤 4: 检查高 64 位 (后 8 字节)
         * 提取高 8 字节到 64 位通用寄存器
         */
        lane = vgetq_lane_u64(vreinterpretq_u64_u8(cmp), 1);
        if (lane) {
            return len + zng_first_diff_byte64(lane);
        }

        len += 8;

    } while (len < 256);

    /*
     * 16 轮循环全部通过，说明 256 字节全部匹配成功，返回满匹配 256
     */
    return 256;
}

/*
 * ----------------------------------------------------------------------------
 * 3. 宏清理与模板实例化
 * ----------------------------------------------------------------------------
 */
#undef COMPARE256_NEON_POSTINDEX

Z_INTERNAL uint32_t compare256_neon(const uint8_t *src0, const uint8_t *src1) {
    return compare256_neon_static(src0, src1);
}

#define LONGEST_MATCH       longest_match_neon
#define COMPARE256          compare256_neon_static
#include "match_tpl.h"

#define LONGEST_MATCH_ROLL
#define LONGEST_MATCH       longest_match_roll_neon
#define COMPARE256          compare256_neon_static
#include "match_tpl.h"

#endif /* defined(ARM_NEON) */
