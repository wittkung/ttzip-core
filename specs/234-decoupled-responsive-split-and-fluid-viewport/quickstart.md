# Quickstart Guide: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Verification & Diagnostic Workflow**  

---

## 1. Quick Verification Commands

### Test Layout State Migration & 2:1 Proportions
```bash
swift test --package-path products/ttzip/apple --filter ResponsiveLayoutBoundsTests
```

### Full Monorepo Build Gate
```bash
bazel test -c opt //products/ttzip/apple:TTZipAppTests
```

### Build and Install Release App Bundle
```bash
bash products/ttzip/apple/scripts/bundle_app.sh --release --install
```

### Read Cleaned Host Preferences
```bash
defaults read com.metastudyline.ttzip TTZip_UserRightInspectorRatio
defaults read com.metastudyline.ttzip TTZip_LayoutStateVersion
```
Expected output:
```
0.3333333333333333
1
```
And `defaults read com.metastudyline.ttzip TTZip_UserRightSidebarWidth` must return "The domain/default pair of ... does not exist".
