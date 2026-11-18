# The airpods-tui daemon owns the AirPods connection; apps consume motion over its IPC

Head look reads AirPods head motion from the airpods-tui daemon, never from the AirPods directly. The motion comes over AACP, the L2CAP channel the daemon already holds for battery, noise control and ear detection, and the AirPods take only one AACP connection per host. A client sends `HeadMotion(true)` on the daemon's Unix socket (`$XDG_RUNTIME_DIR/airpods-tui.sock`, u32 big-endian length + JSON) and receives `HeadMotion` samples (timestamp, device-to-world quaternion) and `HeadMotionEnded` (off head, disconnected, unavailable) until it sends `HeadMotion(false)` or disconnects. The daemon streams while at least one client wants motion. The protocol research is MousePods' (Matteo Gaudioso, starman-tech).

## Considered options

- Each app opens its own AACP connection (the prototype bridged one over UDP): a second connection fights the daemon for the channel, and every app would repeat the RemoteHID start and stop sequence.
- A separate motion daemon: two processes would own one connection.

## Consequences

- Head look needs the daemon running; without it, switching on reads Off (no daemon) at once rather than blocking.
- Only clients that asked get motion events, and they are never replayed to new clients, so the TUI and waybar don't see 50 samples a second.
- The daemon's other events (the snapshot replayed on connect, battery, ear detection) reach motion clients too; terrain-head skips any it doesn't know.
- When motion ends, Head look turns Off with the reason and stops asking; the user switches it on again.
