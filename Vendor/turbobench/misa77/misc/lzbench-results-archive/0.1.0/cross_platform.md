Some cross-platform results on [silesia.tar](https://sun.aei.polsl.pl//~sdeor/corpus/silesia.zip) (the Silesia corpus) follow.

Note:

1. "Ratio" ahead is equal to `((compressed size)/(original)) * 100` (so lower is better).
2. All benchmarks were run using [this fork of lzbench](https://github.com/welcome-to-the-sunny-side/lzbench).

---

### x86-64 (Intel)

Details:

- CPU: Intel(R) Core(TM) i7-14650HX (@2.2 GHz) (Intel Turbo disabled).
- Single threaded, pinned to a single performance core.
- CPU governor set to `performance`.

| Compressor name          | Compression | Decompress. |  Ratio | Filename    |
| ------------------------ | ----------- | ----------- | ------ | ------------|
| misa77 0.1.0             |   43.9 MB/s |   4285 MB/s |  39.62 | silesia.tar |
| misa77 0.1.0 yolo        |   7.68 MB/s |   5513 MB/s |  42.75 | silesia.tar |
| lz4 1.10.0               |    370 MB/s |   2512 MB/s |  47.59 | silesia.tar |
| lz4hc 1.10.0 -12         |   7.31 MB/s |   2534 MB/s |  36.45 | silesia.tar |
| lizard 2.1 -10           |    323 MB/s |   2452 MB/s |  48.79 | silesia.tar |
| lzsse4fast 2019-04-18    |    186 MB/s |   2538 MB/s |  45.26 | silesia.tar |
| lzsse8fast 2019-04-18    |    183 MB/s |   2668 MB/s |  44.80 | silesia.tar |
| zxc 0.12.0 -3            |    115 MB/s |   2839 MB/s |  45.46 | silesia.tar |
| zxc 0.12.0 -4            |   81.0 MB/s |   2727 MB/s |  42.63 | silesia.tar |
| zxc 0.12.0 -5            |   48.7 MB/s |   2599 MB/s |  40.25 | silesia.tar |
| zstd 1.5.7 -1            |    297 MB/s |    902 MB/s |  34.54 | silesia.tar |
| snappy 1.2.2             |    376 MB/s |    857 MB/s |  47.89 | silesia.tar |

### x86-64 (AMD)

Details:

- CPU: AMD Ryzen 7 260 (@3.8 GHz) (Frequency boost disabled).

| Compressor name          | Compression | Decompress. | Ratio | Filename    |
| ------------------------ | ----------- | ----------- | ----- | ----------- |
| misa77 0.1.0             |   71.3 MB/s |   6220 MB/s | 39.62 | silesia.tar |
| misa77 0.1.0 yolo        |   13.7 MB/s |   7832 MB/s | 42.75 | silesia.tar |
| lz4hc 1.10.0 -12         |   12.8 MB/s |   4326 MB/s | 36.45 | silesia.tar |
| lz4 1.10.0               |    693 MB/s |   4455 MB/s | 47.59 | silesia.tar |
| lizard 2.1 -10           |    573 MB/s |   2887 MB/s | 48.78 | silesia.tar |
| lzsse4fast 2019-04-18    |    323 MB/s |   4195 MB/s | 45.26 | silesia.tar |
| lzsse8fast 2019-04-18    |    311 MB/s |   4416 MB/s | 44.80 | silesia.tar |
| zxc 0.12.0 -3            |    213 MB/s |   4935 MB/s | 45.99 | silesia.tar |
| zxc 0.12.0 -4            |    151 MB/s |   4776 MB/s | 43.04 | silesia.tar |
| zxc 0.12.0 -5            |   87.3 MB/s |   4570 MB/s | 40.29 | silesia.tar |
| zstd 1.5.7 -1            |    491 MB/s |   1598 MB/s | 34.55 | silesia.tar |
| snappy 1.2.2             |    691 MB/s |   1355 MB/s | 47.85 | silesia.tar |

### ARM64 (Apple Silicon)

Details: 

- CPU: Apple M3

| Compressor name          | Compression | Decompress. | Ratio | Filename    |
| ------------------------ | ----------- | ----------- | ----- | ----------- |
| misa77 0.1.0             |   94.3 MB/s |  10007 MB/s | 39.62 | silesia.tar |
| misa77 0.1.0 yolo        |   17.1 MB/s |  13088 MB/s | 42.75 | silesia.tar |
| lz4 1.10.0               |    881 MB/s |   5173 MB/s | 47.59 | silesia.tar |
| lz4hc 1.10.0 -12         |   17.0 MB/s |   4874 MB/s | 36.45 | silesia.tar |
| zxc 0.12.0 -3            |    276 MB/s |   8010 MB/s | 45.77 | silesia.tar |
| zxc 0.12.0 -4            |    192 MB/s |   7628 MB/s | 43.20 | silesia.tar |
| zxc 0.12.0 -5            |    114 MB/s |   7126 MB/s | 40.30 | silesia.tar |
| snappy 1.2.2             |    966 MB/s |   3438 MB/s | 47.91 | silesia.tar |
| zstd 1.5.7 -1            |    714 MB/s |   1614 MB/s | 34.54 | silesia.tar |
| lizard 2.1 -10           |    830 MB/s |   6530 MB/s | 48.78 | silesia.tar |

Note: There are no explicit intrinsics for NEON yet, and while I've intentionally kept the hot loops simple enough for compilers to easily vectorize, an evil compiler can still destroy performance on non-x86 targets by choosing not to vectorize.
