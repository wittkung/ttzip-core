# PR #2416 AArch64 Microarchitectural Benchmark Report

**Target Function**: `compare256_neon` (`arch/arm/compare256_neon.c`)  
**Hardware Environment**: Apple M5 Max (18 Cores: 6 Super + 12 Performance, 128 GB Unified Memory, `Mac17,6`)  
**Operating System**: macOS 26.6.1 (`arm64-apple-darwin25.6.0`)  
**Compiler**: Apple clang version 21.0.0 (`clang-2100.1.1.101`), `-O3 -DNDEBUG` (Release static builds)  
**Methodology**: 5 cross-interleaved rounds of `develop` baseline vs `candidate` branch back-to-back with thermal cooldowns (Medians reported).

---

## 1. Variance & Statistical Confidence (CV Analysis)

All benchmark runs were executed using Google Benchmark with 5 full cross-interleaved repetitions. The empirical distribution of Coefficient of Variation ($CV = \frac{\sigma}{\mu}$) is summarized below:

- **Overall Benchmark Matrix**: Median $CV = \mathbf{1.05\%}$, Mean $CV = \mathbf{1.45\%}$
- **1MB Streaming Payload Matrix**: Median $CV = \mathbf{1.21\%}$ (e.g. `text` L9 at 0.75%, `mixed` L6 at 0.78%, `striped_rgb` at 0.80%–0.89%)
- **128KB Cache-Resident Matrix**: Median $CV = \mathbf{1.95\%}$ (sub-millisecond iterations 0.3–0.8ms)
- **Sub-Nanosecond Micro Cases**: Median $CV = \mathbf{1.01\%}$, with a max of $6.20\%$ on `32B` due to timer interrupt granularity at sub-nanosecond levels (0.97ns).

---

## 2. Microbenchmark: `compare256/native`

*ns per call (medians of 5 repetitions with cooldowns):*

| len | base | fixed | fixed Δ |
|----:|-----:|------:|--------:|
| 1   | 0.76 |  0.70 | -8.4%   |
| 10  | 1.05 |  0.93 | -11.5%   |
| 16  | 1.06 |  0.91 | -14.5%   |
| 24  | 1.17 |  0.93 | -20.3%   |
| 32  | 1.16 |  0.97 | -16.5%   |
| 40  | 1.38 |  1.16 | -15.9%   |
| 48  | 1.47 |  1.20 | -18.2%   |
| 56  | 1.68 |  1.51 | -10.3%   |
| 64  | 1.83 |  1.65 | -9.5%   |
| 80  | 2.25 |  1.74 | -22.9%   |
| 100 | 2.95 |  2.07 | -29.8%   |
| 175 | 4.55 |  2.69 | -40.9%   |
| 256 | 6.17 |  3.37 | -45.4%   |

---

## 3. Comprehensive Macrobenchmark: `deflate_bench` (Full 50-Point Matrix)

*128KB & 1MB across all 8 standard workloads (medians of 5 repetitions with cooldowns):*

| benchmark | base | fixed | fixed Δ |
|---|---:|---:|---:|
| `deflate_bench` text/131072/1 | 158.5 µs | **128.7 µs** | **-18.8%** |
| `deflate_bench` text/131072/3 | 308.0 µs | 315.9 µs | +2.6% |
| `deflate_bench` text/131072/6 | 899.1 µs | 887.8 µs | -1.3% |
| `deflate_bench` text/131072/9 | 1.18 ms | 1.15 ms | -2.7% |
| `deflate_bench` text/1048576/1 | 1.75 ms | **1.53 ms** | **-12.2%** |
| `deflate_bench` text/1048576/3 | 3.69 ms | 3.60 ms | -2.4% |
| `deflate_bench` text/1048576/6 | 8.68 ms | 8.54 ms | -1.6% |
| `deflate_bench` text/1048576/9 | 10.85 ms | 10.79 ms | -0.5% |
| `deflate_bench` striped_rgb/131072/3 | 17.4 µs | 16.6 µs | -4.7% |
| `deflate_bench` striped_rgb/131072/6 | 18.0 µs | 16.9 µs | -6.2% |
| `deflate_bench` striped_rgb/131072/9 | 83.9 µs | 81.7 µs | -2.6% |
| `deflate_bench` striped_rgb/1048576/3 | 146.4 µs | 137.4 µs | -6.2% |
| `deflate_bench` striped_rgb/1048576/6 | 152.3 µs | 142.4 µs | -6.5% |
| `deflate_bench` striped_rgb/1048576/9 | 684.0 µs | 662.4 µs | -3.2% |
| `deflate_bench` dna/131072/3 | 427.8 µs | 443.2 µs | +3.6% |
| `deflate_bench` dna/131072/6 | 2.60 ms | 2.56 ms | -1.5% |
| `deflate_bench` dna/131072/9 | 20.09 ms | 19.93 ms | -0.8% |
| `deflate_bench` dna/1048576/3 | 3.87 ms | 3.89 ms | +0.3% |
| `deflate_bench` dna/1048576/6 | 23.30 ms | 22.56 ms | -3.2% |
| `deflate_bench` dna/1048576/9 | 182.27 ms | 177.03 ms | -2.9% |
| `deflate_bench` mixed/131072/3 | 346.7 µs | 351.9 µs | +1.5% |
| `deflate_bench` mixed/131072/6 | 789.8 µs | 829.0 µs | +5.0% |
| `deflate_bench` mixed/131072/9 | 4.15 ms | 4.16 ms | +0.4% |
| `deflate_bench` mixed/1048576/3 | 4.13 ms | 4.10 ms | -0.7% |
| `deflate_bench` mixed/1048576/6 | 7.60 ms | 7.70 ms | +1.3% |
| `deflate_bench` mixed/1048576/9 | 35.09 ms | 35.31 ms | +0.6% |
| `deflate_bench` short_match/131072/3 | 434.0 µs | 444.3 µs | +2.4% |
| `deflate_bench` short_match/131072/6 | 540.3 µs | 554.0 µs | +2.5% |
| `deflate_bench` short_match/131072/9 | 738.0 µs | 714.4 µs | -3.2% |
| `deflate_bench` short_match/1048576/3 | 4.89 ms | 4.89 ms | 0.0% |
| `deflate_bench` short_match/1048576/6 | 5.77 ms | 5.72 ms | -0.8% |
| `deflate_bench` short_match/1048576/9 | 7.33 ms | 7.35 ms | +0.2% |
| `deflate_bench` random/131072/3 | 871.3 µs | 870.3 µs | -0.1% |
| `deflate_bench` random/131072/6 | 817.1 µs | 826.4 µs | +1.1% |
| `deflate_bench` random/131072/9 | 1.22 ms | 1.15 ms | -5.6% |
| `deflate_bench` random/1048576/3 | 9.20 ms | 9.15 ms | -0.5% |
| `deflate_bench` random/1048576/6 | 8.03 ms | 8.10 ms | +0.8% |
| `deflate_bench` random/1048576/9 | 11.30 ms | 11.23 ms | -0.6% |
| `deflate_bench` literals/131072/3 | 957.0 µs | 959.7 µs | +0.3% |
| `deflate_bench` literals/131072/6 | 916.6 µs | 916.9 µs | 0.0% |
| `deflate_bench` literals/131072/9 | 2.11 ms | 2.12 ms | +0.5% |
| `deflate_bench` literals/1048576/3 | 9.72 ms | 9.78 ms | +0.6% |
| `deflate_bench` literals/1048576/6 | 8.63 ms | 8.62 ms | -0.2% |
| `deflate_bench` literals/1048576/9 | 19.13 ms | 19.37 ms | +1.3% |
| `deflate_bench` realistic_rgb/131072/3 | 877.5 µs | 892.4 µs | +1.7% |
| `deflate_bench` realistic_rgb/131072/6 | 850.0 µs | 876.5 µs | +3.1% |
| `deflate_bench` realistic_rgb/131072/9 | 1.42 ms | 1.45 ms | +2.1% |
| `deflate_bench` realistic_rgb/1048576/3 | 9.15 ms | 9.21 ms | +0.7% |
| `deflate_bench` realistic_rgb/1048576/6 | 8.13 ms | 8.13 ms | +0.1% |
| `deflate_bench` realistic_rgb/1048576/9 | 12.88 ms | 12.86 ms | -0.1% |

---

## 4. End-to-End Reproduction Instructions

To reproduce these numbers independently:

```bash
# 1. Build the develop baseline
git checkout develop
cmake -B build_develop -DZLIB_COMPAT=ON -DWITH_NATIVE_INSTRUCTIONS=ON -DBUILD_BENCHMARKS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build_develop -j8

# 2. Build candidate branch
git checkout feat-arm64-swar-compare256
cmake -B build_candidate -DZLIB_COMPAT=ON -DWITH_NATIVE_INSTRUCTIONS=ON -DBUILD_BENCHMARKS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build_candidate -j8

# 3. Run Microbenchmarks (13 match lengths)
./build_candidate/test/benchmarks/benchmark_zlib --benchmark_filter=compare256/native --benchmark_repetitions=5

# 4. Run Macrobenchmarks (all workloads and buffer sizes)
./build_candidate/test/benchmarks/benchmark_zlib --benchmark_filter="deflate_bench/level/(text|dna|striped_rgb|mixed|short_match|random|literals|realistic_rgb)/(131072|1048576)/(1|3|6|9)" --benchmark_data_types=all --benchmark_repetitions=5
```

---

## 5. Related Deliverables & Context

- 📝 [Main PR Description](https://github.com/wittkung/TTZip/blob/main/docs/upstream/pr2416_description_updated.md)
- 💬 [Maintainer Reply Final](https://github.com/wittkung/TTZip/blob/main/docs/upstream/pr2416_maintainer_reply_final.md)
- 💌 [Open Letter of Apology and Reflection](https://gist.github.com/wittkung/0874f8afe78020325a3db3326ef7d7e5)

---

## 6. Raw Benchmark Telemetry & JSON Dump

<details>
<summary><b>Click to expand full raw benchmark JSON dump (50+ test points, 5-repetition medians)</b></summary>

```json
{
  "dev_micro": {
    "compare256/native/1": 0.760139276692737,
    "compare256/native/10": 1.0461120257429406,
    "compare256/native/16": 1.0621889970450278,
    "compare256/native/24": 1.1679214362587076,
    "compare256/native/32": 1.1630640439922373,
    "compare256/native/40": 1.3785218972039686,
    "compare256/native/48": 1.4664415533526232,
    "compare256/native/56": 1.6843672767232811,
    "compare256/native/64": 1.8252275222842511,
    "compare256/native/80": 2.2536769705768935,
    "compare256/native/100": 2.9506556244235433,
    "compare256/native/175": 4.554274962121429,
    "compare256/native/256": 6.171730974108218
  },
  "cand_micro": {
    "compare256/native/1": 0.6965157911601615,
    "compare256/native/10": 0.926156156857165,
    "compare256/native/16": 0.9082334390804628,
    "compare256/native/24": 0.9303072943437781,
    "compare256/native/32": 0.9709629123065969,
    "compare256/native/40": 1.1592506483384148,
    "compare256/native/48": 1.2001111113047045,
    "compare256/native/56": 1.511635647307689,
    "compare256/native/64": 1.6514966519064944,
    "compare256/native/80": 1.7382570898254441,
    "compare256/native/100": 2.0719628104200014,
    "compare256/native/175": 2.6915298726980783,
    "compare256/native/256": 3.3703277702754075
  },
  "dev_macro": {
    "deflate_bench/level/text/131072/1": 158456.68549905834,
    "deflate_bench/level/text/131072/3": 308026.08695652167,
    "deflate_bench/level/text/131072/6": 899079.8934753663,
    "deflate_bench/level/text/131072/9": 1179829.0155440427,
    "deflate_bench/level/text/1048576/1": 1747158.5677749354,
    "deflate_bench/level/text/1048576/3": 3686752.631578949,
    "deflate_bench/level/text/1048576/6": 8683309.859154938,
    "deflate_bench/level/text/1048576/9": 10850603.174603151,
    "deflate_bench/level/short_match/131072/3": 433955.94713656366,
    "deflate_bench/level/short_match/131072/6": 540287.068965517,
    "deflate_bench/level/short_match/131072/9": 737954.3378995429,
    "deflate_bench/level/short_match/1048576/3": 4889986.111111108,
    "deflate_bench/level/short_match/1048576/6": 5768099.173553712,
    "deflate_bench/level/short_match/1048576/9": 7333642.10526315,
    "deflate_bench/level/dna/131072/3": 427822.42990654317,
    "deflate_bench/level/dna/131072/6": 2600029.850746271,
    "deflate_bench/level/dna/131072/9": 20094914.285714336,
    "deflate_bench/level/dna/1048576/3": 3872335.1648351755,
    "deflate_bench/level/dna/1048576/6": 23300133.333333287,
    "deflate_bench/level/dna/1048576/9": 182266749.99999925,
    "deflate_bench/level/random/131072/3": 871288.5906040319,
    "deflate_bench/level/random/131072/6": 817076.832151301,
    "deflate_bench/level/random/131072/9": 1221280.6394316128,
    "deflate_bench/level/random/1048576/3": 9200520.00000003,
    "deflate_bench/level/random/1048576/6": 8032505.747126425,
    "deflate_bench/level/random/1048576/9": 11299721.311475463,
    "deflate_bench/level/literals/131072/3": 957004.1608876579,
    "deflate_bench/level/literals/131072/6": 916615.1832460727,
    "deflate_bench/level/literals/131072/9": 2110434.650455928,
    "deflate_bench/level/literals/1048576/3": 9721916.666666638,
    "deflate_bench/level/literals/1048576/6": 8633160.49382717,
    "deflate_bench/level/literals/1048576/9": 19129297.297297303,
    "deflate_bench/level/mixed/131072/3": 346665.84645669314,
    "deflate_bench/level/mixed/131072/6": 789825.0591016521,
    "deflate_bench/level/mixed/131072/9": 4146488.2352941064,
    "deflate_bench/level/mixed/1048576/3": 4131841.1764705884,
    "deflate_bench/level/mixed/1048576/6": 7602373.626373658,
    "deflate_bench/level/mixed/1048576/9": 35091299.99999984,
    "deflate_bench/level/realistic_rgb/131072/3": 877492.2077922067,
    "deflate_bench/level/realistic_rgb/131072/6": 850022.4719101171,
    "deflate_bench/level/realistic_rgb/131072/9": 1424230.6079664582,
    "deflate_bench/level/realistic_rgb/1048576/3": 9147197.368421014,
    "deflate_bench/level/realistic_rgb/1048576/6": 8125395.34883725,
    "deflate_bench/level/realistic_rgb/1048576/9": 12881320.754716983,
    "deflate_bench/level/striped_rgb/131072/3": 17444.881889763707,
    "deflate_bench/level/striped_rgb/131072/6": 17966.853918304074,
    "deflate_bench/level/striped_rgb/131072/9": 83879.3993993993,
    "deflate_bench/level/striped_rgb/1048576/3": 146410.94465251782,
    "deflate_bench/level/striped_rgb/1048576/6": 152320.20997375288,
    "deflate_bench/level/striped_rgb/1048576/9": 684018.4824902699
  },
  "cand_macro": {
    "deflate_bench/level/text/131072/1": 128653.07294658244,
    "deflate_bench/level/text/131072/3": 315944.91525423725,
    "deflate_bench/level/text/131072/6": 887831.9999999995,
    "deflate_bench/level/text/131072/9": 1148037.0370370357,
    "deflate_bench/level/text/1048576/1": 1534407.0796460186,
    "deflate_bench/level/text/1048576/3": 3598649.484536088,
    "deflate_bench/level/text/1048576/6": 8541137.499999996,
    "deflate_bench/level/text/1048576/9": 10794169.230769262,
    "deflate_bench/level/short_match/131072/3": 444285.1851851842,
    "deflate_bench/level/short_match/131072/6": 553981.7549956554,
    "deflate_bench/level/short_match/131072/9": 714364.0256959312,
    "deflate_bench/level/short_match/1048576/3": 4892191.780821902,
    "deflate_bench/level/short_match/1048576/6": 5724142.857142837,
    "deflate_bench/level/short_match/1048576/9": 7345197.916666657,
    "deflate_bench/level/dna/131072/3": 443211.7722328847,
    "deflate_bench/level/dna/131072/6": 2560014.652014651,
    "deflate_bench/level/dna/131072/9": 19925428.57142853,
    "deflate_bench/level/dna/1048576/3": 3885777.777777793,
    "deflate_bench/level/dna/1048576/6": 22562677.419354912,
    "deflate_bench/level/dna/1048576/9": 177029499.99999884,
    "deflate_bench/level/random/131072/3": 870257.142857142,
    "deflate_bench/level/random/131072/6": 826356.0334528105,
    "deflate_bench/level/random/131072/9": 1152787.1621621551,
    "deflate_bench/level/random/1048576/3": 9150394.73684213,
    "deflate_bench/level/random/1048576/6": 8095321.839080499,
    "deflate_bench/level/random/1048576/9": 11229161.290322557,
    "deflate_bench/level/literals/131072/3": 959670.4225352121,
    "deflate_bench/level/literals/131072/6": 916908.440629471,
    "deflate_bench/level/literals/131072/9": 2120279.5031055883,
    "deflate_bench/level/literals/1048576/3": 9777464.788732424,
    "deflate_bench/level/literals/1048576/6": 8618544.303797478,
    "deflate_bench/level/literals/1048576/9": 19372361.11111111,
    "deflate_bench/level/mixed/131072/3": 351946.77419354965,
    "deflate_bench/level/mixed/131072/6": 829020.8333333327,
    "deflate_bench/level/mixed/131072/9": 4164455.0898203664,
    "deflate_bench/level/mixed/1048576/3": 4104218.390804606,
    "deflate_bench/level/mixed/1048576/6": 7700846.153846177,
    "deflate_bench/level/mixed/1048576/9": 35305400.000000015,
    "deflate_bench/level/realistic_rgb/131072/3": 892389.1050583655,
    "deflate_bench/level/realistic_rgb/131072/6": 876504.3586550385,
    "deflate_bench/level/realistic_rgb/131072/9": 1454798.283261793,
    "deflate_bench/level/realistic_rgb/1048576/3": 9207842.105263148,
    "deflate_bench/level/realistic_rgb/1048576/6": 8134883.720930194,
    "deflate_bench/level/realistic_rgb/1048576/9": 12864777.777777692,
    "deflate_bench/level/striped_rgb/131072/3": 16620.328600598164,
    "deflate_bench/level/striped_rgb/131072/6": 16856.016207220877,
    "deflate_bench/level/striped_rgb/131072/9": 81713.12048473548,
    "deflate_bench/level/striped_rgb/1048576/3": 137361.84994045357,
    "deflate_bench/level/striped_rgb/1048576/6": 142353.52512155633,
    "deflate_bench/level/striped_rgb/1048576/9": 662426.1523988685
  }
}
```

</details>
