// SPDX-License-Identifier: 0BSD

///////////////////////////////////////////////////////////////////////////////
//
/// \file       memcmplen.h
/// \brief      Optimized comparison of two buffers
//
//  Author:     Lasse Collin
//
///////////////////////////////////////////////////////////////////////////////

#ifndef LZMA_MEMCMPLEN_H
#define LZMA_MEMCMPLEN_H

#include "common.h"

#ifdef HAVE_IMMINTRIN_H
#	include <immintrin.h>
#endif

#if (defined(__ARM_NEON) || defined(__ARM_NEON__)) && !defined(WORDS_BIGENDIAN)
#	include <arm_neon.h>
#endif

// Only include <intrin.h> if it is needed. The header is only needed
// on Windows when using an MSVC compatible compiler. The Intel compiler
// can use the intrinsics without the header file.
#if defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& defined(_MSC_VER) \
		&& (defined(_M_X64) \
			|| defined(_M_ARM64) || defined(_M_ARM64EC)) \
		&& !defined(__INTEL_COMPILER)
#	include <intrin.h>
#endif


/// Find out how many equal bytes the two buffers have.
///
/// \param      buf1    First buffer
/// \param      buf2    Second buffer
/// \param      len     How many bytes have already been compared and will
///                     be assumed to match
/// \param      limit   How many bytes to compare at most, including the
///                     already-compared bytes. This must be significantly
///                     smaller than UINT32_MAX to avoid integer overflows.
///                     Up to LZMA_MEMCMPLEN_EXTRA bytes may be read past
///                     the specified limit from both buf1 and buf2.
///
/// \return     Number of equal bytes in the buffers is returned.
///             This is always at least len and at most limit.
///
/// \note       LZMA_MEMCMPLEN_EXTRA defines how many extra bytes may be read.
///             It's rounded up to 2^n. This extra amount needs to be
///             allocated in the buffers being used. It needs to be
///             initialized too to keep Valgrind quiet.
static lzma_always_inline uint32_t
lzma_memcmplen(const uint8_t *buf1, const uint8_t *buf2,
		uint32_t len, uint32_t limit)
{
	assert(len <= limit);
	assert(limit <= UINT32_MAX / 2);

#if defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& (defined(__ARM_NEON) || defined(__ARM_NEON__)) \
		&& !defined(WORDS_BIGENDIAN)
	// 128-bit ARM NEON vector comparison for little-endian 64-bit ARM.
	//
	// The 16-byte vector loop is guarded by (len + 16 <= limit) to
	// guarantee that vector loads never read past the specified limit.
	// The remaining bytes (< 16) are compared using 8-byte scalar SWAR
	// reads covered by LZMA_MEMCMPLEN_EXTRA = 8 padding.
#	define LZMA_MEMCMPLEN_EXTRA 8
	while (len + 16 <= limit) {
		const uint8x16_t v1 = vld1q_u8(buf1 + len);
		const uint8x16_t v2 = vld1q_u8(buf2 + len);
		const uint8x16_t eq = vceqq_u8(v1, v2);
		const uint64x2_t eq64 = vreinterpretq_u64_u8(eq);
		const uint64_t low = vgetq_lane_u64(eq64, 0);
		const uint64_t high = vgetq_lane_u64(eq64, 1);

		if (low != UINT64_MAX) {
			const uint64_t diff = ~low;
#	if defined(_MSC_VER)
			unsigned long tmp;
			_BitScanForward64(&tmp, diff);
			len += (uint32_t)tmp >> 3;
#	else
			len += (uint32_t)__builtin_ctzll(diff) >> 3;
#	endif
			return len;
		}

		if (high != UINT64_MAX) {
			const uint64_t diff = ~high;
#	if defined(_MSC_VER)
			unsigned long tmp;
			_BitScanForward64(&tmp, diff);
			len += 8 + ((uint32_t)tmp >> 3);
#	else
			len += 8 + ((uint32_t)__builtin_ctzll(diff) >> 3);
#	endif
			return len;
		}

		len += 16;
	}

	while (len < limit) {
		const uint64_t x = read64ne(buf1 + len) - read64ne(buf2 + len);
		if (x != 0) {
#	if defined(_MSC_VER)
			unsigned long tmp;
			_BitScanForward64(&tmp, x);
			len += (uint32_t)tmp >> 3;
#	else
			len += (uint32_t)__builtin_ctzll(x) >> 3;
#	endif
			return my_min(len, limit);
		}
		len += 8;
	}

	return limit;

#elif defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& (((TUKLIB_GNUC_REQ(3, 4) || defined(__clang__)) \
				&& SIZE_MAX == UINT64_MAX) \
			|| (defined(__INTEL_COMPILER) && defined(__x86_64__)) \
			|| (defined(__INTEL_COMPILER) && defined(_M_X64)) \
			|| (defined(_MSC_VER) && (defined(_M_X64) \
				|| defined(_M_ARM64) || defined(_M_ARM64EC))))
	// This is only for x86-64 and ARM64 for now. This might be fine on
	// other 64-bit processors too.
	//
	// Reasons to use subtraction instead of xor:
	//
	//   - On some x86-64 processors (Intel Sandy Bridge to Tiger Lake),
	//     sub+jz and sub+jnz can be fused but xor+jz or xor+jnz cannot.
	//     Thus using subtraction has potential to be a tiny amount faster
	//     since the code checks if the quotient is non-zero.
	//
	//   - Some processors (Intel Pentium 4) used to have more ALU
	//     resources for add/sub instructions than and/or/xor.
	//
	// The processor info is based on Agner Fog's microarchitecture.pdf
	// table "Instruction statistics: Macro-op fusion".
#	define LZMA_MEMCMPLEN_EXTRA 8
	while (len < limit) {
		const uint64_t x = read64ne(buf1 + len) - read64ne(buf2 + len);
		if (x != 0) {
#	if defined(_MSC_VER)
			// MSVC on x86-64 or ARM64
			unsigned long tmp;
			_BitScanForward64(&tmp, x);
			len += (uint32_t)tmp >> 3;
#	else
			// GCC or Clang on x86-64 or ARM64
			len += (uint32_t)__builtin_ctzll(x) >> 3;
#	endif
			return my_min(len, limit);
		}

		len += 8;
	}

	return limit;

#elif defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& defined(__SSE2__) \
		&& (defined(__GNUC__) || defined(__clang__) \
			|| defined(__INTEL_COMPILER))
	// NOTE: This will use SSE2 on x86-64 too if the compiler happens
	// to support __SSE2__ but not __builtin_ctzll(). That situation is
	// unlikely though.
#	define LZMA_MEMCMPLEN_EXTRA 16
	while (len < limit) {
		const uint32_t mask = (uint32_t)_mm_movemask_epi8(
			_mm_cmpeq_epi8(
			_mm_loadu_si128((const __m128i *)(buf1 + len)),
			_mm_loadu_si128((const __m128i *)(buf2 + len))));
		if (mask != 0xFFFF) {
			// Cast to uint32_t to silence a warning from
			// -Wsign-conversion.
			len += (uint32_t)__builtin_ctz(~mask);
			return my_min(len, limit);
		}

		len += 16;
	}

	return limit;

#elif defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& !defined(WORDS_BIGENDIAN) \
		&& (defined(__GNUC__) || defined(__clang__) \
			|| defined(__INTEL_COMPILER))
	// Generic 32-bit little endian method
#	define LZMA_MEMCMPLEN_EXTRA 4
	while (len < limit) {
		uint32_t x = read32ne(buf1 + len) - read32ne(buf2 + len);
		if (x != 0) {
			len += (uint32_t)__builtin_ctz(x) >> 3;
			return my_min(len, limit);
		}

		len += 4;
	}

	return limit;

#elif defined(TUKLIB_FAST_UNALIGNED_ACCESS) \
		&& !defined(WORDS_BIGENDIAN) \
		&& defined(_MSC_VER)
	// Generic 32-bit little endian method for MSVC on 32-bit x86.
#	define LZMA_MEMCMPLEN_EXTRA 4
	while (len < limit) {
		uint32_t x = read32ne(buf1 + len) - read32ne(buf2 + len);
		if (x != 0) {
			unsigned long tmp;
			_BitScanForward(&tmp, x);
			len += (uint32_t)tmp >> 3;
			return my_min(len, limit);
		}

		len += 4;
	}

	return limit;

#else
	// Simple portable version that doesn't read past the end of the buffers.
#	define LZMA_MEMCMPLEN_EXTRA 0
	while (len < limit) {
		if (buf1[len] != buf2[len])
			break;

		++len;
	}

	return len;
#endif
}

#endif // LZMA_MEMCMPLEN_H
