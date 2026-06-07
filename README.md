# 🩺 ArchDoctor

**ArchDoctor** — One command to diagnose common Arch Linux issues.

ArchDoctor is an ultra-lightweight, minimalist Terminal User Interface (TUI) dashboard that instantly diagnoses common issues on Arch Linux systems, provides user-friendly solutions, and analyzes boot performance with visual graphs.

```text
    █████╗ ██████╗  ██████╗██╗  ██╗██████╗  ██████╗  ██████╗████████╗ ██████╗ ██████╗ 
   ██╔══██╗██╔══██╗██╔════╝██║  ██║██╔══██╗██╔═══██╗██╔════╝╚══██╔══╝██╔═══██╗██╔══██╗
   ███████║██████╔╝██║     ███████║██║  ██║██║   ██║██║        ██║   ██║   ██║██████╔╝
   ██╔══██║██╔══██╗██║     ██╔══██║██║  ██║██║   ██║██║        ██║   ██║   ██║██╔══██╗
   ██║  ██║██║  ██║╚██████╗██║  ██║██████╔╝╚██████╔╝╚██████╗   ██║   ╚██████╔╝██║  ██║
   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═════╝  ╚═════╝ ╚═════╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝
```

## ✨ Features

### Live Health Score

Monitor your system's overall health with a dynamic progress bar and a real-time score out of 100.

### Failed Service Detection

Quickly identify crashed, failed, or inactive systemd services in seconds.

### Disk Space Analysis

Track root filesystem (`/`) usage and receive cleanup recommendations when usage exceeds safe thresholds.

### Orphan Package Detection

Find orphaned packages that are wasting disk space and remove them with a single command.

### Smart Kernel & Log Analyzer

Detect critical boot and hardware-related issues—such as Nvidia/Nouveau conflicts—using a regex-powered log analysis engine.

### Boot Performance Analysis

Visualize kernel, initrd, and userspace boot times with an attractive horizontal performance chart powered by `systemd-analyze`.

---

## 🛠️ Installation & Usage

### Method 1: Install as a Global `archdoctor` Command (Recommended)

From the project directory, run:

```bash
cargo install --path .
```

**Note:** If `~/.cargo/bin` is included in your `$PATH`, you can launch ArchDoctor from anywhere by simply running:

```bash
archdoctor
```

### Method 2: Build and Run from Source

```bash
git clone https://github.com/florex/archdoctor.git
cd archdoctor
cargo run --release
```

---

## 🎹 Keyboard Shortcuts

| Key                | Action                                           |
| ------------------ | ------------------------------------------------ |
| ▲ / ▼ or `k` / `j` | Navigate between system checks in the left panel |
| `R` / `r`          | Re-run all diagnostics and refresh results       |
| `Q` / `q` or `Esc` | Safely exit ArchDoctor                           |

---

## 📦 Publishing on the AUR

To allow Arch Linux users to install the project directly via:

```bash
yay -S archdoctor-git
```

you can publish it to the AUR using the included `PKGBUILD` file. This makes installation, updates, and maintenance seamless for Arch Linux users.
