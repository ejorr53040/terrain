#!/usr/bin/env python3
"""PROTOTYPE, throwaway (branch prototype/head-look). Not the real design: see terrain#92.

Reads AirPods Max head motion with MousePods' AAP client (clone at /tmp/mousepods) and
sends head yaw/pitch in degrees to terrain over UDP 127.0.0.1:4243 as two little-endian f32.
Positive yaw = head turned left, positive pitch = looking down.

    python3 PROTOTYPE_head_bridge.py --calibrate   # learn the Max's turn and nod axes
    python3 PROTOTYPE_head_bridge.py               # stream

The airpods-tui daemon must be stopped first: only one program can hold the AACP channel.
"""
import json, math, os, select, socket, struct, subprocess, sys, time

sys.path.insert(0, "/tmp/mousepods")
import airpods_mouse_controller as m

ADDR = "08:FF:44:4C:12:98"
AXES = os.path.expanduser("~/.config/terrain/PROTOTYPE_head_axes.json")


def quat(b):
    x, y, z = (v / 32768 for v in struct.unpack_from("<3h", b, 20))
    return (math.sqrt(max(0.0, 1 - x * x - y * y - z * z)), x, y, z)


def mul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (aw * bw - ax * bx - ay * by - az * bz, aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw)


def conj(q):
    return (q[0], -q[1], -q[2], -q[3])


def rotvec(q):
    """Rotation vector (radians) of a unit quaternion."""
    w, x, y, z = q
    if w < 0:
        w, x, y, z = -w, -x, -y, -z
    n = math.sqrt(x * x + y * y + z * z)
    if n < 1e-9:
        return (0.0, 0.0, 0.0)
    a = 2 * math.atan2(n, w)
    return (x / n * a, y / n * a, z / n * a)


def dot(a, b):
    return sum(i * j for i, j in zip(a, b))


def unit(v):
    n = math.sqrt(dot(v, v))
    return tuple(i / n for i in v)


def samples(aap):
    while True:
        r, _, _ = select.select([aap.sock], [], [], 0.5)
        if not r:
            continue
        pkt = aap.sock.recv(8192)
        if not pkt:
            raise OSError("channel closed")
        for b in aap.process(pkt) or []:
            yield time.monotonic(), quat(b)


def notify(msg):
    subprocess.run(["notify-send", "-t", "3500", "Head look calibration", msg])
    print(msg, flush=True)


def calibrate(aap):
    steps = [(0, "Face the screen and hold still"), (3, "Turn your head LEFT and hold"),
             (7, "Face the screen again"), (10, "Look DOWN and hold"), (14, "Done")]
    t0, q0, qleft, qdown, shown = None, None, None, None, 0
    for t, q in samples(aap):
        t0 = t0 or t
        e = t - t0
        while shown < len(steps) and e >= steps[shown][0]:
            notify(steps[shown][1])
            shown += 1
        if 2.0 <= e < 2.5: q0 = q
        if 6.0 <= e < 6.5: qleft = q
        if 13.0 <= e < 13.5: qdown = q
        if e >= 14:
            break
    left, down = rotvec(mul(conj(q0), qleft)), rotvec(mul(conj(q0), qdown))
    yaw = unit(left)
    pitch = unit(tuple(d - dot(down, yaw) * y for d, y in zip(down, yaw)))
    os.makedirs(os.path.dirname(AXES), exist_ok=True)
    json.dump({"yaw": yaw, "pitch": pitch}, open(AXES, "w"))
    deg = lambda v: round(math.degrees(math.sqrt(dot(v, v))), 1)
    print(f"turned {deg(left)}°, looked down {deg(down)}°, turn/nod axes {math.degrees(math.acos(abs(dot(unit(left), unit(down))))):.0f}° apart")
    print("axes", yaw, pitch)


def stream(aap):
    axes = json.load(open(AXES))
    yaw_axis, pitch_axis = axes["yaw"], axes["pitch"]
    out = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    q0, n = None, 0
    for _, q in samples(aap):
        n += 1
        if n < 25:  # the first sample is a junk identity; take straight ahead half a second in
            continue
        q0 = q0 or q
        v = rotvec(mul(conj(q0), q))
        yaw, pitch = math.degrees(dot(v, yaw_axis)), math.degrees(dot(v, pitch_axis))
        out.sendto(struct.pack("<ff", yaw, pitch), ("127.0.0.1", 4243))
        if n % 50 == 0:
            print(f"yaw {yaw:6.1f}  pitch {pitch:6.1f}", flush=True)


def session(run):
    aap = m.AAPClient(ADDR, True, 20000)  # 50 Hz
    aap.connect()
    aap.start_tracking()
    try:
        run(aap)
    finally:
        aap.close()


def main():
    if "--calibrate" in sys.argv:
        return session(calibrate)
    while True:  # the Bluetooth link drops now and then: reconnect and keep streaming
        try:
            session(stream)
        except OSError as e:
            print(f"link lost ({e}), reconnecting", flush=True)
            time.sleep(2)


if __name__ == "__main__":
    main()
