#ifndef THIRD_PARTY_SNAPPY_OPENSOURCE_CMAKE_CONFIG_H_
#define THIRD_PARTY_SNAPPY_OPENSOURCE_CMAKE_CONFIG_H_

/* Define to 1 if the compiler supports __attribute__((always_inline)). */
#define HAVE_ATTRIBUTE_ALWAYS_INLINE 1

/* Define to 1 if the compiler supports __builtin_ctz and friends. */
#define HAVE_BUILTIN_CTZ 1

/* Define to 1 if the compiler supports __builtin_expect. */
#define HAVE_BUILTIN_EXPECT 1

/* Define to 1 if the compiler supports __builtin_prefetch. */
#define HAVE_BUILTIN_PREFETCH 1

/* Define to 1 if you have a definition for mmap() in <sys/mman.h>. */
#define HAVE_FUNC_MMAP 1

/* Define to 1 if you have a definition for sysconf() in <unistd.h>. */
#define HAVE_FUNC_SYSCONF 1

/* Define to 1 if you have the `lzo2' library (-llzo2). */
#define HAVE_LIBLZO2 0

/* Define to 1 if you have the `z' library (-lz). */
#define HAVE_LIBZ 1

/* Define to 1 if you have the `lz4' library (-llz4). */
#define HAVE_LIBLZ4 0

/* Define to 1 if you have the <sys/mman.h> header file. */
#define HAVE_SYS_MMAN_H 1

/* Define to 1 if you have the <sys/resource.h> header file. */
#define HAVE_SYS_RESOURCE_H 1

/* Define to 1 if you have the <sys/time.h> header file. */
#define HAVE_SYS_TIME_H 1

/* Define to 1 if you have the <sys/uio.h> header file. */
#define HAVE_SYS_UIO_H 1

/* Define to 1 if you have the <unistd.h> header file. */
#define HAVE_UNISTD_H 1

/* Define to 1 if you have the <windows.h> header file. */
#define HAVE_WINDOWS_H 0

/* Define to 1 if you target processors with SSSE3+ and have <tmmintrin.h>. */
#define SNAPPY_HAVE_SSSE3 0

/* Define to 1 if you target processors with SSE4.2 and have <crc32intrin.h>. */
#define SNAPPY_HAVE_X86_CRC32 0

/* Define to 1 if you target processors with BMI2+ and have <bmi2intrin.h>. */
#define SNAPPY_HAVE_BMI2 0

/* Define to 1 if you target processors with NEON and have <arm_neon.h>. */
#define SNAPPY_HAVE_NEON 1

/* Define to 1 if you target processors with RVV1.0 and have <riscv_vector.h>. */
#define SNAPPY_RVV_1 0

/* Define to 1 if you target processors with RVV0.7 and have <riscv_vector.h>. */
#define SNAPPY_RVV_0_7 0

/* Define to 1 if you have <arm_neon.h> and <arm_acle.h> and want to optimize
   compression speed by using __crc32cw from <arm_acle.h>. */
#define SNAPPY_HAVE_NEON_CRC32 1

/* Define to 1 if your processor stores words with the most significant byte
   first (like Motorola and SPARC, unlike Intel and VAX). */
#define SNAPPY_IS_BIG_ENDIAN 0

#if defined(__APPLE__)
/* Apple multi-architecture universal builds (x86_64, arm64, etc.)
   Override configure-time single-architecture probes with slice-aware compiler macros. */
#undef SNAPPY_HAVE_SSSE3
#undef SNAPPY_HAVE_X86_CRC32
#undef SNAPPY_HAVE_BMI2
#undef SNAPPY_HAVE_NEON
#undef SNAPPY_HAVE_NEON_CRC32

#if defined(__arm64__) || defined(__aarch64__)
#define SNAPPY_HAVE_NEON 1
#if defined(__ARM_FEATURE_CRC32)
#define SNAPPY_HAVE_NEON_CRC32 1
#else
#define SNAPPY_HAVE_NEON_CRC32 0
#endif
#define SNAPPY_HAVE_SSSE3 0
#define SNAPPY_HAVE_X86_CRC32 0
#define SNAPPY_HAVE_BMI2 0
#elif defined(__x86_64__) || defined(_M_X64) || defined(__i386__) || defined(_M_IX86)
#define SNAPPY_HAVE_NEON 0
#define SNAPPY_HAVE_NEON_CRC32 0
#if defined(__SSSE3__)
#define SNAPPY_HAVE_SSSE3 1
#else
#define SNAPPY_HAVE_SSSE3 0
#endif
#if defined(__SSE4_2__)
#define SNAPPY_HAVE_X86_CRC32 1
#else
#define SNAPPY_HAVE_X86_CRC32 0
#endif
#if defined(__BMI2__)
#define SNAPPY_HAVE_BMI2 1
#else
#define SNAPPY_HAVE_BMI2 0
#endif
#else
#define SNAPPY_HAVE_NEON 0
#define SNAPPY_HAVE_NEON_CRC32 0
#define SNAPPY_HAVE_SSSE3 0
#define SNAPPY_HAVE_X86_CRC32 0
#define SNAPPY_HAVE_BMI2 0
#endif
#endif  /* defined(__APPLE__) */

#endif  // THIRD_PARTY_SNAPPY_OPENSOURCE_CMAKE_CONFIG_H_
