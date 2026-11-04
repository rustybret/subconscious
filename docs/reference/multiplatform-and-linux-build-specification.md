# Multi-Platform Build Specification & 12-Target Fleet Standard

**Status**: Ratified Design Requirement  
**Target Modules**: `ck-uc-discussions`, `ck-mc`, `ck-claustrum`, and fleet services  
**Alignment Reference**: Upstream `omo` & `uc-studio` 12-Target Build Standard  
**Owning Infrastructure**: Subconscious (in-tree descriptors), Cloudhome (Jenkins & BuildKit runners on `altos-worker-01`), Arcus (catalog & release distribution)  
**Date**: November 2026

---

## 1. Architectural Strategy & Scope

To achieve complete operational coverage across modern developer workstations, headless cloud VMs, and legacy hardware, the fleet adopts the authoritative **12-target build standard** established by `omo` and `uc-studio`.

This standard addresses three orthogonal layers:
1. **Operating System Families**: macOS (Darwin), Linux, and Windows.
2. **CPU Architecture & Instruction Set Tiers**:
   - `aarch64` (ARM64) for Apple Silicon, modern Linux servers, and ARM Windows.
   - `x86_64` (amd64) split into **AVX2 (`x86-64-v3`)** for performance and **Baseline (`x86-64-v2` / no-AVX2)** for legacy CPU and virtualization compatibility.
3. **C-Library Linkage (ELF on Linux)**:
   - Dynamic linking against an older `glibc` LTS baseline (`glibc 2.31`).
   - Completely standalone, zero-dependency static linking via `musl`.

```
┌────────────────────────────────────────────────────────────────────────┐
│               UltraCreative 12-Target Fleet Matrix                     │
├───────────────────┬────────────────────────────────┬───────────────────┤
│    macOS (Darwin) │             Linux              │      Windows      │
├───────────────────┼────────────────────────────────┼───────────────────┤
│ • macOS ARM64     │ • Linux x64 (glibc)            │ • Windows x64     │
│ • macOS x64       │ • Linux x64 (glibc, no-AVX2)   │ • Windows x64     │
│ • macOS x64       │ • Linux ARM64 (glibc)          │   (no-AVX2)       │
│   (no-AVX2)       │ • Linux x64 (musl)             │ • Windows ARM64   │
│                   │ • Linux x64 (musl, no-AVX2)    │   (native / node) │
│                   │ • Linux ARM64 (musl)           │                   │
└───────────────────┴────────────────────────────────┴───────────────────┘
```

---

## 2. Microarchitecture Levels: AVX2 vs. Baseline (No-AVX2)

Modern x86_64 software often silently enables AVX2 instructions (Haswell/Zen+ baseline), which triggers fatal `Illegal instruction (SIGILL)` crashes on older CPUs, low-cost VPS instances, nested virtualization (e.g., cloud hypervisors lacking AVX2 flags), and legacy Macs.

The fleet explicitly splits all x86_64 targets into two discrete microarchitecture tiers:

| Tier | Microarchitecture Level | CPU Target Baseline | Compiler Flags (Rust / Zig) | Minimum Hardware Equivalent |
|---|---|---|---|---|
| **Standard (AVX2)** | `x86-64-v3` | Haswell, Excavator, Zen+ | `-C target-cpu=haswell` or `-C target-feature=+avx,+avx2,+fma` | Intel Core 4th Gen+ (2013), AMD Ryzen (2017) |
| **Baseline (No-AVX2)** | `x86-64-v2` (or generic) | Nehalem, Westmere, Sandy Bridge | `-C target-cpu=generic` or `-C target-feature=-avx,-avx2,-fma` | Any 64-bit x86 CPU with SSE4.2 / POPCNT (2008+) |

### Compilation Rules for No-AVX2 Variants
- When compiling baseline variants, the build toolchain must explicitly disable AVX, AVX2, and FMA:
  ```bash
  # Rust / Cargo
  RUSTFLAGS="-C target-cpu=generic -C target-feature=-avx,-avx2,-fma" cargo zigbuild ...
  ```
- Binary filenames and package manifests must clearly distinguish baseline builds (e.g., `ck-uc-discussions-x86_64-linux-no-avx2`).

---

## 3. The Linux C-Library Layer: glibc vs. musl

All Linux variants use the ELF format under the hood, but C runtime linkage dictates portable execution:

### 3.1 glibc Variants (Dynamic Linking)
- **Forward-Incompatibility Invariant**: glibc is backward-compatible, never forward-compatible. Compiling on Ubuntu 24.04 leaks `GLIBC_2.38` symbols and will not run on Ubuntu 22.04 or Debian 12.
- **LTS Baseline Requirement**: All dynamic Linux binaries must be compiled against **glibc 2.31** (Ubuntu 20.04 LTS / Debian 11 / AlmaLinux 8 baseline) using `cargo zigbuild --target <target>.2.31`.

### 3.2 musl Variants (Static Linking)
- **Zero-Dependency Invariant**: Targeting `*-unknown-linux-musl` produces fully statically linked ELF binaries.
- **Portability**: Executes without external libc dependencies across Alpine Linux, minimal container images, immutable distros (NixOS, SteamOS, Talos), and standard glibc distributions.

---

## 4. Full 12-Target Authoritative Fleet Matrix

The canonical matrix implemented across `subconscious`, `omo`, and `uc-studio`:

| # | Fleet Target Identifier | Rust Target Triple | Feature / Linkage Profile | Primary Target Environment |
|---|---|---|---|---|
| 1 | `darwin-arm64` | `aarch64-apple-darwin` | Apple Silicon (M1–M4), Hardened Runtime | Modern macOS workstations |
| 2 | `darwin-x64` | `x86_64-apple-darwin` | Intel Mac, `x86-64-v3` (AVX2), Hardened Runtime | Modern Intel Macs |
| 3 | `darwin-x64-no-avx2` | `x86_64-apple-darwin` | Intel Mac, `x86-64-v2` (Baseline / no-AVX2) | Legacy Intel Macs, virtualization |
| 4 | `linux-x64-gnu` | `x86_64-unknown-linux-gnu.2.31` | glibc >= 2.31, `x86-64-v3` (AVX2) | Modern Linux x86_64 desktops & servers |
| 5 | `linux-x64-gnu-no-avx2`| `x86_64-unknown-linux-gnu.2.31` | glibc >= 2.31, `x86-64-v2` (Baseline / no-AVX2) | Older cloud VPS, legacy servers, KVM |
| 6 | `linux-arm64-gnu` | `aarch64-unknown-linux-gnu.2.31` | glibc >= 2.31 | ARM64 Linux servers, Raspberry Pi OS |
| 7 | `linux-x64-musl` | `x86_64-unknown-linux-musl` | musl, `x86-64-v3` (AVX2), Static | Alpine Linux, minimal containers, NixOS |
| 8 | `linux-x64-musl-no-avx2`| `x86_64-unknown-linux-musl` | musl, `x86-64-v2` (Baseline / no-AVX2), Static | Universal fallback across all x86 Linux |
| 9 | `linux-arm64-musl` | `aarch64-unknown-linux-musl` | musl, Static | ARM64 containers, Alpine, edge devices |
| 10 | `windows-x64` | `x86_64-pc-windows-gnu` / `msvc` | Windows x86_64 PE, `x86-64-v3` (AVX2) | Modern Windows 10/11 x86_64 PCs |
| 11 | `windows-x64-no-avx2` | `x86_64-pc-windows-gnu` / `msvc` | Windows x86_64 PE, `x86-64-v2` (Baseline / no-AVX2)| Older Windows PCs, Hyper-V, VMs |
| 12 | `windows-arm64` | `aarch64-pc-windows-msvc` | Windows ARM64 (or x64 emulation fallback) | Windows on Snapdragon / Surface Pro |

---

## 5. System Packaging & Distribution Tiers

Each of the 12 target binaries maps into the appropriate distribution channels:

1. **Native Distro Packages (`.deb` & `.rpm`)**:
   - Generated from `linux-x64-gnu` and `linux-arm64-gnu` using `cargo-deb` and `cargo-generate-rpm`.
   - Embeds systemd service unit definitions for headless daemon management.
2. **Universal Tarballs (`.tar.zst` & `.tar.gz`)**:
   - Generated for all 12 variants containing the binary, shell completions, and licensing.
   - Standard release format for Arcus sequence-first catalog distribution (`dist/<seq>/<pkg>/<ver>/`).
3. **Multi-Arch OCI Container Images**:
   - Multi-platform images published for `linux/amd64` and `linux/arm64` using the statically linked musl binaries.

---

## 6. Known Upstream Blockers & Compiler Exceptions

### 6.1 Upstream Defect in `subc-os` (Windows Named Pipes)
- **Affects**: Targets 10, 11, and 12 (`windows-x64`, `windows-x64-no-avx2`, `windows-arm64`).
- **Location**: `subc-os` (0.1.25 through 0.1.27) `src/named_pipe/windows.rs:167:52`.
- **Defect**: Direct un-gated call to `Box::as_mut_ptr`, triggering `error[E0658]: use of unstable library feature box_as_ptr (issue #129090)`.
- **Status**: Compiling for Windows targets requires an upstream fix in `subc-os` (replacing `Box::as_mut_ptr` with `descriptor.as_mut() as *mut _` or `Box::into_raw`) or unstable compiler overrides.
- **Impact**: All 9 macOS and Linux targets build cleanly with zero errors.
