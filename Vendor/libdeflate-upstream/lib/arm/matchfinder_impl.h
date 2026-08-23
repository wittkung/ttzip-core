/*
 * arm/matchfinder_impl.h - ARM implementations of matchfinder functions
 *
 * Copyright 2016 Eric Biggers
 *
 * Permission is hereby granted, free of charge, to any person
 * obtaining a copy of this software and associated documentation
 * files (the "Software"), to deal in the Software without
 * restriction, including without limitation the rights to use,
 * copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following
 * conditions:
 *
 * The above copyright notice and this permission notice shall be
 * included in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
 * OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
 * HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
 * WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
 * OTHER DEALINGS IN THE SOFTWARE.
 */

#ifndef LIB_ARM_MATCHFINDER_IMPL_H
#define LIB_ARM_MATCHFINDER_IMPL_H

#include "cpu_features.h"

#if HAVE_NEON_NATIVE
static forceinline void
matchfinder_init_neon(mf_pos_t *data, size_t size)
{
	int16x8_t *p = (int16x8_t *)data;
	int16x8_t v = vdupq_n_s16(MATCHFINDER_INITVAL);

	STATIC_ASSERT(MATCHFINDER_MEM_ALIGNMENT % sizeof(*p) == 0);
	STATIC_ASSERT(MATCHFINDER_SIZE_ALIGNMENT % (4 * sizeof(*p)) == 0);
	STATIC_ASSERT(sizeof(mf_pos_t) == 2);

	do {
		p[0] = v;
		p[1] = v;
		p[2] = v;
		p[3] = v;
		p += 4;
		size -= 4 * sizeof(*p);
	} while (size != 0);
}
#define matchfinder_init matchfinder_init_neon

static forceinline void
matchfinder_rebase_neon(mf_pos_t *data, size_t size)
{
	int16x8_t *p = (int16x8_t *)data;
	int16x8_t v = vdupq_n_s16((u16)-MATCHFINDER_WINDOW_SIZE);

	STATIC_ASSERT(MATCHFINDER_MEM_ALIGNMENT % sizeof(*p) == 0);
	STATIC_ASSERT(MATCHFINDER_SIZE_ALIGNMENT % (4 * sizeof(*p)) == 0);
	STATIC_ASSERT(sizeof(mf_pos_t) == 2);

	do {
		p[0] = vqaddq_s16(p[0], v);
		p[1] = vqaddq_s16(p[1], v);
		p[2] = vqaddq_s16(p[2], v);
		p[3] = vqaddq_s16(p[3], v);
		p += 4;
		size -= 4 * sizeof(*p);
	} while (size != 0);
}
#define matchfinder_rebase matchfinder_rebase_neon

static forceinline u32
lz_extend_neon(const u8 * const strptr, const u8 * const matchptr,
               const u32 start_len, const u32 max_len)
{
    u32 len = start_len;
    machine_word_t v_word;

    /* Tier-0: 64-bit GPR SWAR fast check for short match exit (< 8 bytes) */
    if (len + sizeof(machine_word_t) <= max_len) {
        v_word = load_word_unaligned(&matchptr[len]) ^ load_word_unaligned(&strptr[len]);
        if (v_word != 0) {
#if CPU_IS_LITTLE_ENDIAN()
            return len + (bsfw(v_word) >> 3);
#else
            return len + ((WORDBITS - 1 - bsrw(v_word)) >> 3);
#endif
        }
        len += sizeof(machine_word_t);
    }

    /* Tier-1: 128-bit NEON Vector Unrolling (16 bytes per iteration) */
    while (len + 16 <= max_len) {
        uint8x16_t q1 = vld1q_u8(matchptr + len);
        uint8x16_t q2 = vld1q_u8(strptr + len);
        uint8x16_t qdiff = veorq_u8(q1, q2);
        uint64_t d0 = vgetq_lane_u64(vreinterpretq_u64_u8(qdiff), 0);
        uint64_t d1 = vgetq_lane_u64(vreinterpretq_u64_u8(qdiff), 1);

        if (d0 != 0) {
            return len + ((u32)__builtin_ctzll(d0) >> 3);
        }
        if (d1 != 0) {
            return len + 8 + ((u32)__builtin_ctzll(d1) >> 3);
        }
        len += 16;
    }

    while (len + sizeof(machine_word_t) <= max_len) {
        v_word = load_word_unaligned(&matchptr[len]) ^ load_word_unaligned(&strptr[len]);
        if (v_word != 0) {
#if CPU_IS_LITTLE_ENDIAN()
            return len + (bsfw(v_word) >> 3);
#else
            return len + ((WORDBITS - 1 - bsrw(v_word)) >> 3);
#endif
        }
        len += sizeof(machine_word_t);
    }

    while (len < max_len && matchptr[len] == strptr[len])
        len++;

    return len;
}
#define lz_extend lz_extend_neon

#endif /* HAVE_NEON_NATIVE */

#endif /* LIB_ARM_MATCHFINDER_IMPL_H */
