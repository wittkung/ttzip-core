// SPDX-License-Identifier: 0BSD

///////////////////////////////////////////////////////////////////////////////
//
/// \file       arm64.c
/// \brief      Filter for ARM64 binaries
///
/// This converts ARM64 relative addresses in the BL and ADRP immediates
/// to absolute values to increase redundancy of ARM64 code.
///
/// Converting B or ADR instructions was also tested but it's not useful.
/// A majority of the jumps for the B instruction are very small (+/- 0xFF).
/// These are typical for loops and if-statements. Encoding them to their
/// absolute address reduces redundancy since many of the small relative
/// jump values are repeated, but very few of the absolute addresses are.
//
//  Authors:    Lasse Collin
//              Jia Tan
//              Igor Pavlov
//
///////////////////////////////////////////////////////////////////////////////

#include "simple_private.h"

#if (defined(__ARM_NEON) || defined(__ARM_NEON__)) && !defined(HAVE_SMALL)
#	include <arm_neon.h>
#endif


static size_t
arm64_code(void *simple lzma_attribute((__unused__)),
		uint32_t now_pos, bool is_encoder,
		uint8_t *buffer, size_t size)
{
	size &= ~(size_t)3;

	size_t i = 0;

#if (defined(__ARM_NEON) || defined(__ARM_NEON__)) && !defined(HAVE_SMALL)
	// 16-byte NEON vector mask filter.
	//
	// In ARM64 binaries, the vast majority of instructions are basic
	// ALU, loads, stores, and conditional jumps that do not match BL
	// or ADRP. We test 4 instructions in parallel (16 bytes) and fast-skip
	// the whole chunk if no BL/ADRP candidates are found.
	const uint32x4_t mask_bl = vdupq_n_u32(0xFC000000);
	const uint32x4_t pattern_bl = vdupq_n_u32(0x94000000);
	const uint32x4_t mask_adrp = vdupq_n_u32(0x9F000000);
	const uint32x4_t pattern_adrp = vdupq_n_u32(0x90000000);

	while (i + 16 <= size) {
		const uint32x4_t v = vreinterpretq_u32_u8(vld1q_u8(buffer + i));
		const uint32x4_t match_bl = vceqq_u32(
				vandq_u32(v, mask_bl), pattern_bl);
		const uint32x4_t match_adrp = vceqq_u32(
				vandq_u32(v, mask_adrp), pattern_adrp);
		const uint32x4_t any_match = vorrq_u32(match_bl, match_adrp);

		if (vmaxvq_u32(any_match) == 0) {
			i += 16;
			continue;
		}

		for (size_t lane = 0; lane < 4; ++lane) {
			const size_t cur = i + lane * 4;
			uint32_t pc = (uint32_t)(now_pos + cur);
			uint32_t instr = read32le(buffer + cur);

			if ((instr >> 26) == 0x25) {
				const uint32_t src = instr;
				instr = 0x94000000;

				pc >>= 2;
				if (!is_encoder)
					pc = 0U - pc;

				instr |= (src + pc) & 0x03FFFFFF;
				write32le(buffer + cur, instr);

			} else if ((instr & 0x9F000000) == 0x90000000) {
				const uint32_t src = ((instr >> 29) & 3)
						| ((instr >> 3) & 0x001FFFFC);

				if ((src + 0x00020000) & 0x001C0000)
					continue;

				instr &= 0x9000001F;

				pc >>= 12;
				if (!is_encoder)
					pc = 0U - pc;

				const uint32_t dest = src + pc;
				instr |= (dest & 3) << 29;
				instr |= (dest & 0x0003FFFC) << 3;
				instr |= (0U - (dest & 0x00020000)) & 0x00E00000;
				write32le(buffer + cur, instr);
			}
		}
		i += 16;
	}
#endif

	for (; i < size; i += 4) {
		uint32_t pc = (uint32_t)(now_pos + i);
		uint32_t instr = read32le(buffer + i);

		if ((instr >> 26) == 0x25) {
			const uint32_t src = instr;
			instr = 0x94000000;

			pc >>= 2;
			if (!is_encoder)
				pc = 0U - pc;

			instr |= (src + pc) & 0x03FFFFFF;
			write32le(buffer + i, instr);

		} else if ((instr & 0x9F000000) == 0x90000000) {
			const uint32_t src = ((instr >> 29) & 3)
					| ((instr >> 3) & 0x001FFFFC);

			if ((src + 0x00020000) & 0x001C0000)
				continue;

			instr &= 0x9000001F;

			pc >>= 12;
			if (!is_encoder)
				pc = 0U - pc;

			const uint32_t dest = src + pc;
			instr |= (dest & 3) << 29;
			instr |= (dest & 0x0003FFFC) << 3;
			instr |= (0U - (dest & 0x00020000)) & 0x00E00000;
			write32le(buffer + i, instr);
		}
	}

	return i;
}


static lzma_ret
arm64_coder_init(lzma_next_coder *next, const lzma_allocator *allocator,
		const lzma_filter_info *filters, bool is_encoder)
{
	return lzma_simple_coder_init(next, allocator, filters,
			&arm64_code, 0, 4, 4, is_encoder);
}


#ifdef HAVE_ENCODER_ARM64
extern lzma_ret
lzma_simple_arm64_encoder_init(lzma_next_coder *next,
		const lzma_allocator *allocator,
		const lzma_filter_info *filters)
{
	return arm64_coder_init(next, allocator, filters, true);
}


extern LZMA_API(size_t)
lzma_bcj_arm64_encode(uint32_t start_offset, uint8_t *buf, size_t size)
{
	// start_offset must be a multiple of four.
	start_offset &= ~UINT32_C(3);
	return arm64_code(NULL, start_offset, true, buf, size);
}
#endif


#ifdef HAVE_DECODER_ARM64
extern lzma_ret
lzma_simple_arm64_decoder_init(lzma_next_coder *next,
		const lzma_allocator *allocator,
		const lzma_filter_info *filters)
{
	return arm64_coder_init(next, allocator, filters, false);
}


extern LZMA_API(size_t)
lzma_bcj_arm64_decode(uint32_t start_offset, uint8_t *buf, size_t size)
{
	// start_offset must be a multiple of four.
	start_offset &= ~UINT32_C(3);
	return arm64_code(NULL, start_offset, false, buf, size);
}
#endif
