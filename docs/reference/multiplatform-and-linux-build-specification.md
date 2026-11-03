# Multi-Platform and Linux Build Specification for CortexKit & Fleet Modules

**Status**: Ratified Design Requirement  
**Target Modules**: `ck-uc-discussions`, `ck-mc`, `ck-claustrum`, and fleet Rust services  
**Owning Infrastructure**: Subconscious (in-tree descriptors), Cloudhome (Jenkins & BuildKit runners on `altos-worker-01`), Arcus (catalog & release distribution)  
**Date**: November 2026

---

## 1. Architectural Strategy & Scope

To achieve broad cross-platform operational reach, binaries in the CortexKit and UltraCreative fleet must support three tier-1 operating systems (**macOS/Darwin**, **Linux**, and **Windows**) across modern CPU architectures (**x86_64** and **aarch64**).

For Linux specifically, compatibility requires addressing two distinct layers:
1. **Low-Level Binary & C-Library Layer (ELF)**: Dictates whether an executable can link and load on a host OS without C runtime symbol crashes.
2. **Distribution Packaging & Delivery Layer**: Dictates how operators, automated tools, and system init managers install, verify, and supervise the binary.

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Target Architectures                            │
│                  x86_64 (amd64)  │  aarch64 (arm64)                    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          ▼                         ▼                         ▼
   ┌──────────────┐         ┌───────────────┐         ┌──────────────┐
   │ macOS/Darwin │         │     Linux     │         │   Windows    │
   │  arm64 / x64 │         │  glibc / musl │         │    x86_64    │
   └──────────────┘         └───────┬───────┘         └──────────────┘
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        ▼                           ▼                           ▼
┌───────────────┐           ┌───────────────┐           ┌───────────────┐
│ System Pkgs   │           │ Static Archive│           │ OCI Container │
│ .deb / .rpm   │           │ .tar.zst / gz │           │ Multi-Arch    │
│ (systemd/apt) │           │ (zero-dep)    │           │ (CI / Daemon) │
└───────────────┘           └───────────────┘           └───────────────┘
```

---

## 2. Layer 1: Low-Level Binary & C-Library Layer (ELF)

All standard Linux executables use the **ELF** (Executable and Linkable Format). ELF binary incompatibility on Linux almost exclusively stems from C standard library mismatches.

### 2.1 The glibc vs. musl Divide
- **glibc (GNU C Library)**: The standard runtime across desktop, enterprise, and cloud distributions (Ubuntu, Debian, Fedora, RHEL, CentOS, openSUSE, Arch). Executables linked against glibc are dynamically resolved against the host system's `libc.so.6`.
- **musl libc**: The lightweight standard runtime used by security-focused, embedded, and minimal container distributions (Alpine Linux, Void Linux).
- **Incompatibility Rule**: A binary dynamically linked against glibc **cannot** run on a musl-based system, failing at load time with `No such file or directory` or dynamic interpreter refusal.

### 2.2 The "glibc Version Trap" (Forward Incompatibility)
- **Invariant**: glibc guarantees **backward compatibility, not forward compatibility**. A binary compiled against glibc 2.38 cannot resolve symbols (such as `GLIBC_2.38`) when executed on a system running glibc 2.31 or 2.35.
- **Requirement**: Dynamically linked Linux binaries must be compiled against an **older LTS baseline** (glibc 2.31, corresponding to Ubuntu 20.04 LTS / Debian 11 / AlmaLinux 8), guaranteeing execution on any newer host.
- **musl Static Linking**: For Rust daemons and CLI tools, static compilation targeting `*-unknown-linux-musl` produces a completely self-contained binary with zero dynamic library dependencies. This guarantees universal execution across all Linux distributions regardless of host glibc or musl versions.

---

## 3. Layer 2: Packaging & Distribution Formats

The fleet categorizes software packaging into three delivery tiers based on target consumption:

### 3.1 Native System Packages (`.deb` and `.rpm`)
- **Primary Audience**: System administrators, automated workstation setups (`ucs-setup`), server nodes (`altos-worker-01`, `nimbus`), and enterprise servers.
- **Formats**:
  - **`.deb` (Debian Package)**: Debian, Ubuntu, Linux Mint, Pop!_OS, Raspberry Pi OS.
  - **`.rpm` (Red Hat Package Manager)**: RHEL, Fedora, AlmaLinux, Rocky Linux, openSUSE.
- **Requirements**:
  - Integrate daemon service definitions (`systemd` system/user units).
  - Place binaries in standard filesystem hierarchy (`/usr/local/bin` or `/usr/bin`).
  - Automated tooling: Generated via `cargo-deb` and `cargo-generate-rpm` or `nfpm`.

### 3.2 Universal Archives (`.tar.zst` and `.tar.gz`)
- **Primary Audience**: Standalone users, custom install scripts, Arcus package hydration, and offline nodes.
- **Requirements**:
  - Bundle the statically linked binary (`*-unknown-linux-musl` or LTS glibc baseline).
  - Include shell completions (bash, zsh, fish) and man pages.
  - Formatted as sequence-first Arcus bundles (`.tar.zst`) and universal `.tar.gz`.

### 3.3 Multi-Arch OCI / Container Images
- **Primary Audience**: Cloudhome Kubernetes workloads, Jenkins build agents, and automated test environments.
- **Requirements**:
  - Multi-platform manifests published for `linux/amd64` and `linux/arm64`.
  - Minimal attack surface (scratch or Alpine base image with static musl binary).

### 3.4 Desktop Sandboxes (Flatpak / AppImage / Snap)
- **Applicability**: Reserved for interactive desktop GUI applications (such as Dashboard Tauri builds).
- **Rule for Daemons**: Headless daemon services (`ck-uc-discussions`, `ck-mc`) and CLI tools must **not** be packaged as Flatpak or AppImage, as these formats introduce sandbox isolation boundaries that obstruct subc loopback sockets and supervisor signals.

---

## 4. Concrete Target Matrix for Fleet Modules

For `ck-uc-discussions` and companion fleet modules, the build pipeline targets seven primary binary variants:

| Platform ID | Target Triple | C Library / Linkage | Primary Use Case |
|---|---|---|---|
| `darwin-arm64` | `aarch64-apple-darwin` | macOS Libsystem (Hardened Runtime) | Apple Silicon Macs (M1/M2/M3/M4) |
| `darwin-x64` | `x86_64-apple-darwin` | macOS Libsystem (Hardened Runtime) | Intel Macs |
| `linux-x64-gnu` | `x86_64-unknown-linux-gnu` | glibc >= 2.31 (dynamic) | Standard Linux desktops/servers (.deb/.rpm) |
| `linux-x64-musl` | `x86_64-unknown-linux-musl` | musl (fully static) | Portable Linux x86_64 tarballs & Alpine |
| `linux-arm64-gnu` | `aarch64-unknown-linux-gnu` | glibc >= 2.31 (dynamic) | ARM64 Linux servers & Raspberry Pi (.deb/.rpm) |
| `linux-arm64-musl` | `aarch64-unknown-linux-musl` | musl (fully static) | Portable Linux aarch64 tarballs & containers |
| `windows-x64` | `x86_64-pc-windows-msvc` | MSVC C++ Runtime | Windows 10/11 x86_64 workstations |

---

## 5. Build Pipeline Contract (Jenkins & Cloudhome)

### 5.1 Infrastructure Ownership
- **Subconscious (Repository)**:
  - Maintains `pipeline/Jenkinsfile` and Arcus packaging manifests (`packaging/arcus/`).
  - Stamps build versions and canonical source SHAs into binary metadata.
- **Cloudhome (`altos-worker-01`)**:
  - Provides the Jenkins Kubernetes agent pool and BuildKit runners.
  - Executes containerized multi-architecture cross-compilation using `cargo-zigbuild` or multi-platform BuildKit stages.
  - Signs Darwin binaries (when coordinated with macOS runners) and generates `.deb`/`.rpm` artifacts.
- **Arcus Gateway**:
  - Ingests verified multi-target payloads and updates `CATALOG-1` channel metadata (`stable`/`latest`).

### 5.2 Verification Gates
Every multi-platform release candidate must pass three mandatory gates before promotion:
1. **Compilation Gate**: Clean exit 0 with `--locked` dependencies across all 7 targets.
2. **Binary Hygiene Gate**:
   - macOS binaries must have hardened runtime enabled (`codesign --verify --strict`).
   - Linux musl binaries must show `statically linked` via `file(1)`.
   - Linux gnu binaries must not require symbols newer than `GLIBC_2.31`.
3. **Execution Gate**: Native execution of `--version` and `--help` on real hardware or native container runners (no mock emulation).

