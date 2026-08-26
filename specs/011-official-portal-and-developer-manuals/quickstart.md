# Quickstart & Verification Guide: Official Portal & Manuals

- **Feature**: `specs/011-official-portal-and-developer-manuals`
- **Domain**: `https://ttzip.app`

---

## 1. Local Testing & Verification

### Step 1: Start Local HTTP Static Server
Run Python built-in static server on the `docs/` or `apple/site/` directory:
```bash
python3 -m http.server 8080 --directory docs
```

### Step 2: Open & Validate Subpages
Open the following routes in Safari / Chrome:
1. `http://localhost:8080/index.html`: Verify 3-orb fluid canvas animation, interactive 3-column Miller column simulator, App Store and Steam download links.
2. `http://localhost:8080/sdk.html`: Verify 8-language tab switching (C++, Rust, Python, Go, JVM, C#, Dart, Swift) and clipboard copy functionality.
3. `http://localhost:8080/cli.html`: Verify CLI command reference and streaming recipe syntax.
4. `http://localhost:8080/performance.html`: Verify Silesia benchmark charts and PMULL throughput metrics.
5. `http://localhost:8080/formats.html`: Verify 16-format matrix and capability icons.
6. `http://localhost:8080/licensing.html`: Verify 4-channel breakdown (Community / Direct / MAS / Steam) and Ed25519 offline verification instructions.
7. `http://localhost:8080/privacy.html` & `http://localhost:8080/terms.html`: Verify legal disclosures.

---

## 2. Production CDN Verification (`https://ttzip.app`)

1. Check HTTP to HTTPS 301 redirection.
2. Verify Let's Encrypt SSL certificate validity.
3. Confirm DNS resolution via `dig ttzip.app +short` returning GitHub Anycast IPs:
   - `185.199.108.153`
   - `185.199.109.153`
   - `185.199.110.153`
   - `185.199.111.153`
