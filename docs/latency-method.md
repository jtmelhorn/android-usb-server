# Latency measurement method

Three different quantities. Never report one as another.

| Quantity | What it is | How it is measured | Is it input latency? |
|---|---|---|---|
| **Report interval** | Time between consecutive interrupt-IN reports reaching the Shield | App status (`reportIntervalUs`) and the probe | **No.** It describes how often data arrives. The adapter may only report on change. |
| **Server turnaround** | URB completion reaped on the Shield → reply handed to TCP | App status (`turnaroundUs`) | No. It is one internal slice of the path. |
| **Network RTT** | PC ↔ Shield round trip | `ping <shield-ip>` from the PC (≥100 samples) | No. One-way network delay is roughly RTT/2. |
| **Button-to-screen latency** | Physical press → visible change on the TV showing the Moonlight stream | High-speed camera, below | **Yes.** This is the only number for "feels faster". |

## Button-to-screen procedure (TICKET-018)

Equipment: a phone that records ≥240 fps slow-motion, the TV running Moonlight, and the
controller in the camera frame together with the screen.

Test scene: something on the PC that changes instantly and visibly on a button press, such as a
full-screen gamepad tester with large indicators or an in-game action with an immediate effect. Use
the same scene, resolution, frame rate, Moonlight and Sunshine settings, and network for all
configurations.

Configurations:

- **A** — controller → PC Bluetooth (AX211) directly.
- **B** — controller → 8BitDo adapter on the Shield → Moonlight's controller forwarding (sharing off).
- **C** — controller → 8BitDo adapter on the Shield → this app → USB/IP → PC (sharing on).

For each configuration, record ≥30 presses. For each press, count frames from the first frame the
button is visibly depressed to the first frame the screen changes, and convert frames to ms (×4.17
at 240 fps). Record the raw counts in `evidence/018-latency.csv` with columns
`config,press,frames,fps`.

Report n, median, p90 and the resolution limit (±1 frame). Also note: display mode / game mode, TV
model, Moonlight version and settings, Sunshine version, Wi-Fi vs Ethernet for the Shield and the
PC, and `ping` RTT statistics.

Only a C-vs-B difference larger than the measurement resolution, with overlapping conditions
ruled out, supports a claim that this app reduces latency.
