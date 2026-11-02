# Pixfer — Research: Light-Based Device-to-Device Communication

> One device's **screen** emits modulated visible light; the other device's **camera** receives it.
> No Bluetooth, no Wi-Fi, no GPS — purely optical link, implemented in Rust.

---

## 1. What this is

Pixfer belongs to the family of **Visible Light Communication (VLC)**, the "Li-Fi" subset that
uses everyday light sources (screens, LEDs) as the transmitter and **image sensors (phone cameras)
as the receiver**. A phone camera's sensor is literally a grid of photodiodes, so it can act as a
VLC detector. This is the same physical principle behind:

- **Bell's photophone** (1880) — speech over modulated sunlight.
- **Li-Fi** (popularized at TED 2011) — high-speed data over LED light.
- **QR codes** — the "static 2D" version of screen→camera transfer (slow, needs rescanning).

Key physical property: **light cannot pass through opaque walls**, so the link is naturally
confined to line-of-sight spaces — a built-in privacy/safety barrier radio doesn't have.

## 2. Prior art

| Year | Project / event | What it showed |
|------|------------------|------------------|
| 1880 | Photophone (A. G. Bell) | Modulated light carrying voice |
| 2003 | Keio Univ. (Nakagawa lab) | LED-based VLC data transmission research begins |
| 2010 | Siemens/Fraunhofer | 500 Mbit/s over 5 m with a white LED; 100 Mbit/s long-range |
| 2011 | TED Global | Live HD video over LED; term "Li-Fi" coined |
| 2012+ | IEEE 802.15.7 | Standardizes OOK, VPPM, CSK modulation for WPAN VLC |
| 2014 | **LightShare** (Dartmouth CS project, press coverage) | Laptop **screen → phone camera** file transfer, reported on the order of ~10 MB/s |
| 2015 | Philips + Carrefour (Lille), Edeka (Düsseldorf) | Commercial indoor positioning via LED lamps → phone cameras |
| 2014 | Axrtek "MOMO" | Bidirectional RGB-LED VLC, 300 Mbit/s, ~7.6 m range |

**Takeaway:** screen→camera file transfer is *demonstrated and practical* — the core idea is not
new, but a clean, open, Rust-native implementation with a designed protocol + FEC is still a real
contribution.

## 3. Modulation options (grounded in IEEE 802.15.7)

| Scheme | Encoding | Pros | Cons |
|--------|----------|------|------|
| **OOK** (on-off keying) | Pulse width / on-off states | Simplest TX + RX, easy to implement on a screen | Needs a *compensation pulse* to keep time-averaged brightness constant (else visible flicker/dimming shifts) |
| **VPPM** (variable pulse position) | Symbol = pulse *position* within the symbol period | No compensation pulse needed | Harder to demodulate (edge-position sampling); needs tight TX/RX timing sync |
| **CSK** (color-shift keying) | Jump between RGB color states; time-averaged color is constant | 3 parallel color sub-channels; human eye can't see the rapid color jumps (below flicker-fusion threshold) | More complex color management; needs a known target chromaticity |

**Pixfer's recommended approach — 2D block code (QR-like bursts):**

Instead of bit-level clock sync (fragile), encode each *screen frame* as a **2D grid of on/off
blocks** (like a QR code, but refreshed at the display's frame rate). The receiver detects the grid
geometry (like QR finder patterns) and reads the block states. This sidesteps bit-clock recovery
entirely — frame-level sync, not bit-level.

## 4. Proposed protocol — "Pixfer v0"

```
Device A (TX)                        Device B (RX)
+----------------+   visible light    +----------------+
| screen: render | ================   | camera: capture|
| block grid     | (direct line-of-  | + grid detect  |
| @ display rate |  sight required)   | + block decode |
+----------------+                    +----------------+
```

**Frame layout (per screen frame):**

```
+----------------------------------------------------------+
| SYNC ZONE: 3 finder markers (QR-like, in 3 corners)    |  -> grid auto-detection
| HEADER: magic "PIXF" + chunk_idx + total_chunks +      |
|           payload_len + CRC32                             |
| PAYLOAD: file chunk bytes -> bits -> blocks (MSB first) |
| FEC: Reed-Solomon RS(255,223) parity blocks            |
+----------------------------------------------------------+
```

- **Session handshake**: TX opens with a `PING` frame; RX replies with a `PONG` frame (its own
  screen also emits light — the link is inherently bidirectional, time-division duplex).
- **File transfer**: split file into 223-byte chunks (fits RS(255,223)); one chunk per frame.
- **Integrity**: CRC32 per chunk; RS corrects up to ~16 byte-errors per frame; on uncorrectable
  errors, request retransmit (simple NAK/ACK frames).

## 5. Throughput estimates

Assuming a 1080p display @ 60 fps:

| Block size | Grid | Bits/frame | Raw throughput | Est. effective (after sync/header/FEC overhead) |
|------------|------|-----------|----------------|---------------------------------------------------|
| 4×4 px | 270×480 = 129,600 | 129.6 kb | **~7.8 Mbps ≈ 0.95 MB/s** | ~0.6–0.8 MB/s |
| 2×2 px | 540×960 = 518,400 | 518.4 kb | **~31 Mbps ≈ 3.6 MB/s** | ~2.5–3 MB/s |

Camera-side caveat: phone cameras typically cap full-res capture at 30 fps — use a **region of
interest (ROI) crop** to keep frame rate high. 30 fps halves the numbers above.

## 6. Key engineering challenges

1. **Auto-exposure (AGC) on the camera**: if AGC is enabled it will track the on/off modulation
   and destroy the signal. **Exposure must be locked** (manual exposure + fixed gain).
2. **Ambient light / sunlight**: the main enemy. Mitigations: max screen brightness, dark
   background around the pattern, and color keying (CSK) for robustness.
3. **Grid alignment**: the receiver must auto-detect block geometry (QR finder markers in 3
   corners), then a homography to the camera's distorted view.
4. **Flicker compensation**: per IEEE 802.15.7 OOK, insert a compensation pulse to equalize
   time-averaged brightness (no visible flicker or dimming shifts).
5. **LCD vs OLED**: LCDs modulate the backlight; per-pixel RGB control is limited by panel
   response (~5–30 ms) — fine at 60 Hz. OLEDs are inherently per-pixel, easier to modulate.
6. **Distance/SNR**: a phone screen at ~400–800 nits has good SNR at 1 m; degrades by 5 m.
   Indirect (multipath) reflections from walls add noise; 2D codes + RS FEC absorb it.

## 7. Rust implementation plan

Cargo workspace (desktop-first, phones second):

| Crate | Responsibility | Key deps |
|-------|---------------|-----------|
| `pixfer-core` | Protocol, block encoding/decoding, RS FEC, CRC32, chunking. Pure Rust, fully unit-testable | `reed-solomon`, `crc32fast` |
| `pixfer-tx` | Screen rendering of the block grid | `wgpu` (desktop); Android camera2 / iOS AVFoundation via FFI (mobile) |
| `pixfer-rx` | Camera capture + grid detect + decode pipeline | `gst` (GStreamer, Linux); mobile camera HAL |
| `pixfer` | CLI: `pixfer send <file>`, `pixfer recv` | `clap`, `ratatui` (optional TUI) |

**Milestones:**

- **M0**: `pixfer-core` + unit tests (encode/decode round-trip, FEC, chunking).
- **M1**: Desktop 2-device prototype — one laptop renders the grid via `wgpu`, the other captures
  with GStreamer; validate link at 1–3 m and measure throughput.
- **M2**: Protocol v1 — handshake (PING/PONG), RS(255,223), CRC32, NAK/ACK retransmit.
- **M3**: End-to-end file-transfer CLI.
- **M4**: Mobile HALs — Android camera2 (exposure lock), iOS AVCaptureSession; screen at max
  brightness; camera ROI crop for high fps.

## 8. Open questions / risks

- **Verification**: does a 2×2-px block grid survive a phone camera's anti-aliasing + lens
  blur at 1–3 m? (Prototype M1 answers this; fall back to 4×4 if needed.)
- **Bidirectionality**: RX→TX needs the receiving device's screen too (phones have one); confirm
  duplex timing so both devices alternate TX/RX roles.
- **Battery**: continuous camera capture + max-brightness screen will drain fast — acceptable for
  a short transfer session.
- **Standardization gap**: IEEE 802.15.7 targets LED-lamp VLC; a *display-as-LED-array* approach
  is a practical niche — worth documenting the deviations in the spec.

## 9. Web (browser) implementation

**Feasible, and it's the fastest path to a two-device demo.** No installs: two browser tabs on two devices pointed at each other — `/tx` (canvas light) and `/rx` (camera capture + decode).

| Role | Browser API | Notes |
|------|-------------|-------|
| **TX tab** | Fullscreen `<canvas>` + `requestAnimationFrame`, block grid drawn as WebGL rects (cheap) | Keep the tab foreground — background tabs get rAF throttled. Avoid per-pixel `ImageData` at 1080p@60fps (≈2 GB/s memory bandwidth); use WebGL or a downscaled pattern. |
| **RX tab** | `navigator.mediaDevices.getUserMedia({video:{width:640,height:480,facingMode:'environment'}}` + `requestVideoFrameCallback` (Chrome 107+) | Request a smaller resolution to unlock higher camera fps. `getImageData` on a small canvas for block reads. |
| **Exposure lock** | `track.applyConstraints({advanced:[{exposureMode:'manual', exposureTime:T, gain:G}])` | Solid on Chrome; more limited on Safari/iOS. AGC is the #1 link killer, so this matters. |
| **Protocol core** | `pixfer-core` (Rust) compiled to **WASM** | One protocol implementation shared by the web UI and the native builds. |

**Honest caveats:**
- `getUserMedia` requires a **secure context** (HTTPS or localhost) — fine for a hosted demo.
- **Background-tab throttling**: the TX tab must stay foreground/fullscreen.
- **Camera fps caps**: full-res capture is ~30fps on phones; request 640×480 for higher fps.
- **Mobile browsers**: permission prompts work, but manual-exposure constraints are far less controllable than a native camera HAL — the web demo works on phones, but the production mobile app (M4) still needs native APIs for reliable exposure lock.

**Milestone impact**: a web demo (call it **M0.5**) lets you validate the link end-to-end in a browser before any Rust camera/display HALs exist.

## 10. Sources

- Wikipedia: *Visible light communication* (modulation families, IEEE 802.15.7 OOK/VPPM/CSK,
  camera-sensor-as-VLC-receiver).
- LightShare (2014) press coverage: laptop screen → phone camera file transfer (~10 MB/s reported).
- IEEE 802.15.7 (VLC standard); Haas "Li-Fi" (TED 2011).