#!/usr/bin/env python3
import argparse, os, re, shutil, subprocess, sys, tempfile

def frames_from_concat(path):
    base = os.path.dirname(os.path.abspath(path))
    out = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        m = re.match(r"file\s+'(.*)'$", line) or re.match(r"file\s+(\S+)$", line)
        if m:
            f = m.group(1)
            out.append([f if os.path.isabs(f) else os.path.join(base, f), None])
            continue
        m = re.match(r"duration\s+([0-9.]+)$", line)
        if m and out:
            out[-1][1] = float(m.group(1))
    if len(out) > 1 and out[-1][1] is None and out[-1][0] == out[-2][0]:
        out.pop()
    return out

def plan(cmd, src, fps, work):
    if cmd == "concat":
        frames = frames_from_concat(src)
        known = [d for _, d in frames if d]
        fallback = sum(known) / len(known) if known else 1 / fps
        frames = [(f, d if d else fallback) for f, d in frames]
    else:
        files = sorted(os.path.join(src, f) for f in os.listdir(src) if f.lower().endswith(".png"))
        frames = [(f, 1 / fps) for f in files]
    if not frames:
        sys.exit("gif.py: no frame found in " + src)
    kept, delays, t, shown = [], [], 0.0, 0
    for f, d in frames:
        start = round(t * 100)
        t += d
        end = round(t * 100)
        if kept and start - shown < 2:
            delays[-1] = end - shown
            continue
        if kept:
            delays[-1] = start - shown
        kept.append(f)
        delays.append(end - start)
        shown = start
    if delays and delays[-1] < 2:
        delays[-1] = 2
    for n, f in enumerate(kept):
        os.symlink(os.path.abspath(f), os.path.join(work, "%06d.png" % n))
    with open(os.path.join(work, "delays"), "w") as fh:
        fh.write(" ".join(map(str, delays)))
    if len(kept) != len(frames):
        print("merged %d of %d frames under 20 ms apart" % (len(frames) - len(kept), len(frames)))

def patch(gif, delays_file):
    delays = [int(v) for v in open(delays_file).read().split()]
    data = bytearray(open(gif, "rb").read())
    n = 0
    flags = data[10]
    pos = 13 + (3 * (2 ** ((flags & 7) + 1)) if flags & 0x80 else 0)
    while pos < len(data):
        b = data[pos]
        if b == 0x3B:
            break
        if b == 0x21:
            label = data[pos + 1]
            if label == 0xF9 and n < len(delays):
                data[pos + 4] = delays[n] & 0xFF
                data[pos + 5] = (delays[n] >> 8) & 0xFF
                n += 1
            pos += 2
            while data[pos]:
                pos += data[pos] + 1
            pos += 1
        elif b == 0x2C:
            lflags = data[pos + 9]
            pos += 10
            if lflags & 0x80:
                pos += 3 * (2 ** ((lflags & 7) + 1))
            pos += 1
            while data[pos]:
                pos += data[pos] + 1
            pos += 1
        else:
            sys.exit("gif.py: unexpected GIF block")
    if n != len(delays):
        sys.exit("gif.py: %d frame delays for %d frames" % (n, len(delays)))
    open(gif, "wb").write(bytes(data))

def main():
    ap = argparse.ArgumentParser(description="Frames to a full-palette GIF at the recorded timing.")
    ap.add_argument("input", help=".ffconcat written by the record script step, or a directory of PNGs")
    ap.add_argument("output")
    ap.add_argument("--fps", type=float, default=30.0, help="frame rate for a directory without .ffconcat")
    ap.add_argument("--dither", choices=["sierra2_4a", "none"], default="sierra2_4a")
    a = ap.parse_args()
    src, mode = a.input, "concat"
    if os.path.isdir(src):
        lists = sorted(f for f in os.listdir(src) if f.endswith(".ffconcat"))
        if len(lists) == 1:
            src = os.path.join(src, lists[0])
        else:
            mode = "dir"
    work = tempfile.mkdtemp()
    try:
        plan(mode, src, a.fps, work)
        graph = ("split[a][b];[a]palettegen=stats_mode=full:max_colors=256:reserve_transparent=0[p];"
                 "[b][p]paletteuse=dither=%s:diff_mode=rectangle" % a.dither)
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-framerate", "100",
                        "-i", os.path.join(work, "%06d.png"), "-filter_complex", graph, "-loop", "0", a.output],
                       check=True)
        patch(a.output, os.path.join(work, "delays"))
    finally:
        shutil.rmtree(work)
    print("wrote %s (%d KB)" % (a.output, os.path.getsize(a.output) // 1024))

if __name__ == "__main__":
    main()
