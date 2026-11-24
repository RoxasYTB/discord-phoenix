#!/usr/bin/env python3
"""Side-by-side + diff heatmap + per-region mismatch scores, Discord vs Phoenix.

Usage (see README.md):
  ~/discord-phoenix-ref/.venv/bin/python scripts/compare/compare.py            # capture both, compare
  ... compare.py --discord d.png --phoenix p.png [--regions regions.json]      # compare existing shots
Output: /mnt/windata/project-phoenix/compare/<timestamp>/ (PRIVATE data: never commit or post images).
"""
import argparse, json, os, subprocess, sys, time
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFilter

HOME = Path.home()
CDP = HOME / "discord-phoenix-ref" / "cdp.mjs"
REPO = Path(__file__).resolve().parents[2]
OUT_ROOT = Path("/mnt/windata/project-phoenix/compare")
BIN = Path(os.environ.get("PHX_REAL_SHOTS", "/home/yohan/discord-phoenix/target/debug/examples/real_shots"))

BOXES_JS = r"""(()=>{const q=s=>{const e=document.querySelector(s);if(!e)return null;const b=e.getBoundingClientRect();
return b.width&&b.height?[b.x,b.y,b.width,b.height].map(Math.round):null};
return {vw:innerWidth,vh:innerHeight,dpr:devicePixelRatio,path:location.pathname,
guilds:q('nav[class*="guilds_"]'),sidebar:q('[class*="sidebar_"]'),chat:q('[class*="chat_"]'),
header:q('section[class*="title_"]'),form:q('main form, [class*="chatContent_"] form, form'),
members:q('[class*="membersWrap_"]'),
dmprofile:q('[class*="profilePanel_"]')||q('aside[class*="aside_"]'),panels:q('section[class*="panels_"]')}})()"""


def cdp(*args):
    return subprocess.run(["node", str(CDP), *args], capture_output=True, text=True, check=True).stdout


def take_lock(wait_s):
    """Hold the CDP driver lock for the whole live capture (`hover body` acquires it).
    Waits up to `wait_s` seconds while another agent holds it, then fails."""
    env = dict(os.environ, CDP_DRIVER=os.environ.get("CDP_DRIVER") or "compare.py")
    t0 = time.time()
    while True:
        r = subprocess.run(["node", str(CDP), "hover", "body"], capture_output=True, text=True, env=env)
        if r.returncode == 0:
            return env
        if r.returncode != 3 or time.time() - t0 > wait_s:
            sys.exit(f"CDP driver lock not acquired: {(r.stderr or r.stdout).strip()}")
        print(f"waiting for the CDP lock: {r.stderr.strip()}", flush=True)
        time.sleep(10)


def release_lock(env):
    subprocess.run(["node", str(CDP), "release"], capture_output=True, text=True, env=env)


OVERLAY_JS = r"""(()=>{const o=document.querySelector('.user-profile-modal-v2')||document.querySelector('.user-profile-popout');
if(!o)return null;const b=o.getBoundingClientRect();let id=null;
for(const e of o.querySelectorAll('*')){const k=Object.keys(e).find(k=>k.startsWith('__reactFiber'));let f=k&&e[k];
for(let i=0;i<8&&f&&!id;i++,f=f.return){const p=f.memoizedProps;if(p&&p.user&&p.user.id)id=p.user.id;else if(p&&p.userId)id=p.userId}if(id)break}
return {kind:o.classList.contains('user-profile-modal-v2')?'modal':'popout',box:[b.x,b.y,b.width,b.height].map(Math.round),user:id}})()"""

OPEN_MEMBER_JS = r"""(n=>{const l=[...document.querySelectorAll('div[class^=member_]')].filter(e=>{const b=e.getBoundingClientRect();return b.y>0&&b.y<innerHeight});
if(!l[n])return false;l[n].dataset.phxcmp=1;return true})(%d)"""


HIDE_FX_JS = r"""(()=>{const s=document.createElement('style');s.id='phx-nofx';s.textContent='[class*=profileEffects_]{visibility:hidden!important}';document.head.appendChild(s);return true})()"""
SHOW_FX_JS = r"""(()=>{document.getElementById('phx-nofx')?.remove();return true})()"""


def open_overlay(env, member, modal):
    """Drive Discord (read-only): open member `member`'s popout, then its full profile."""
    if json.loads(cdp("eval", OPEN_MEMBER_JS % member)) is not True:
        sys.exit("member row not found")
    subprocess.run(["node", str(CDP), "click", '[data-phxcmp="1"]'], check=True, capture_output=True, env=env)
    time.sleep(1.5)
    if modal:
        cdp("eval", "(()=>{const a=document.querySelector('.user-profile-popout [class*=avatar_WRLIGC]');if(a)a.dataset.phxcmpa=1})()")
        subprocess.run(["node", str(CDP), "click", '[data-phxcmpa="1"]'], check=True, capture_output=True, env=env)
        time.sleep(2)


def close_overlay(env, n):
    for _ in range(n):
        subprocess.run(["node", str(CDP), "key", "Escape"], capture_output=True, env=env)
        time.sleep(0.4)


def regions_from(b):
    W, H = b["vw"], b["vh"]
    g = b.get("guilds") or [0, 32, 72, H - 32]
    top = g[1]
    side = b.get("sidebar") or [0, top, 312, H - top]
    chat = b.get("chat") or [side[0] + side[2], top, W - side[2], H - top]
    hdr = b.get("header") or [chat[0], top, chat[2], 48]
    form = b.get("form") or [chat[0], H - 68, chat[2], 68]
    mem = b.get("members")
    dmp = b.get("dmprofile")
    right = mem or dmp
    msg_right = (right[0] if right else chat[0] + chat[2])
    r = {
        "title bar": [0, 0, W, top],
        "guild rail": g,
        "channel list": [g[0] + g[2], top, side[0] + side[2] - (g[0] + g[2]), side[3]],
        "header": hdr,
        "messages": [chat[0], hdr[1] + hdr[3], msg_right - chat[0], form[1] - (hdr[1] + hdr[3])],
        "composer": form,
    }
    if mem:
        r["member list"] = [mem[0], hdr[1] + hdr[3], mem[2], H - hdr[1] - hdr[3]]
    elif dmp:
        r["dm profile panel"] = dmp
    r.update(b.get("extra") or {})
    return {k: v for k, v in r.items() if v and v[2] > 0 and v[3] > 0}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--discord"); ap.add_argument("--phoenix"); ap.add_argument("--regions")
    ap.add_argument("--out")
    ap.add_argument("--lock-wait", type=float, default=300, help="seconds to wait for the CDP lock")
    ap.add_argument("--overlay", action="store_true", help="score the profile popout/modal open in Discord")
    ap.add_argument("--open-member", type=int, help="open the n-th visible member's popout first (implies --overlay); Escape after")
    ap.add_argument("--modal", action="store_true", help="with --open-member: open the full profile from the popout")
    ap.add_argument("--no-effects", action="store_true", help="hide animated profile effects on both sides (frame-stable scores)")
    a = ap.parse_args()
    live = not (a.regions and a.discord and a.phoenix)
    env = take_lock(a.lock_wait) if live else None
    opened = 0
    try:
        if env and a.open_member is not None:
            open_overlay(env, a.open_member, a.modal)
            opened = 2 if a.modal else 1
            a.overlay = True
        if env and a.no_effects:
            cdp("eval", HIDE_FX_JS)
            time.sleep(0.3)
        return run(a)
    finally:
        if env and a.no_effects:
            cdp("eval", SHOW_FX_JS)
        if env:
            close_overlay(env, opened)
            release_lock(env)


def run(a):
    out = Path(a.out) if a.out else OUT_ROOT / time.strftime("%Y%m%d-%H%M%S")
    out.mkdir(parents=True, exist_ok=True)
    if a.regions:
        boxes = json.load(open(a.regions))
    else:
        boxes = json.loads(cdp("eval", BOXES_JS))
        if a.overlay:
            ov = json.loads(cdp("eval", OVERLAY_JS))
            if not ov or not ov.get("user"):
                sys.exit("no profile popout/modal open in Discord")
            boxes["overlay"] = ov
            boxes.setdefault("extra", {})[f"profile {ov['kind']}"] = ov["box"]
        (out / "boxes.json").write_text(json.dumps(boxes))
    d_path = Path(a.discord) if a.discord else out / "discord.png"
    if not a.discord:
        cdp("shot", str(d_path))
    p_path = Path(a.phoenix) if a.phoenix else out / "phoenix.png"
    if not a.phoenix:
        if not BIN.exists():
            sys.exit(f"build first: cargo rustc -p phoenix-app --example real_shots -- -L ~/.local/lib/devlinks")
        env = dict(os.environ)
        ov = boxes.get("overlay")
        if a.no_effects:
            env["PHX_NO_FX"] = "1"
        if ov:
            env.update(PHX_PROFILE=str(ov["user"]), PHX_PROFILE_KIND=ov["kind"], PHX_PROFILE_BOX=f"{ov['box'][0]},{ov['box'][1]}")
        subprocess.run([str(BIN), str(p_path)], check=True, cwd=out, env=env)

    W, H = boxes["vw"], boxes["vh"]
    s = float(boxes.get("dpr") or 2)
    size = (round(W * s), round(H * s))
    d = Image.open(d_path).convert("RGB").resize(size, Image.LANCZOS)
    p = Image.open(p_path).convert("RGB").resize(size, Image.LANCZOS)

    side = Image.new("RGB", (size[0] * 2 + 8, size[1]), (255, 0, 255))
    side.paste(d, (0, 0)); side.paste(p, (size[0] + 8, 0))
    side.save(out / "side_by_side.png")

    diff = ImageChops.difference(d, p).convert("L")
    lut = [min(255, v * 3) for v in range(256)]
    hot = diff.point(lut).filter(ImageFilter.BoxBlur(2))
    heat = Image.merge("RGB", (hot, hot.point(lambda v: max(0, v - 128) * 2), Image.new("L", size, 0)))
    heat = Image.blend(d.convert("L").convert("RGB"), heat, 0.75)
    dr = ImageDraw.Draw(heat)

    regions = regions_from(boxes)
    mask = diff.point(lambda v: 255 if v > 24 else 0)
    scores = {}
    for name, (x, y, w, h) in regions.items():
        box = tuple(round(v * s) for v in (x, y, x + w, y + h))
        crop = mask.crop(box)
        n = crop.width * crop.height or 1
        mism = sum(crop.histogram()[255:]) / n
        mean = sum(i * c for i, c in enumerate(diff.crop(box).histogram())) / n / 255
        scores[name] = {"mismatch_pct": round(mism * 100, 1), "mean_diff": round(mean * 100, 1), "box_css": [x, y, w, h]}
        dr.rectangle(box, outline=(0, 200, 255), width=3)
        dr.text((box[0] + 6, box[1] + 4), f"{name} {mism*100:.0f}%", fill=(0, 255, 255))
    heat.save(out / "diff_heatmap.png")
    total = sum(mask.histogram()[255:]) / (size[0] * size[1])
    res = {"path": boxes.get("path"), "viewport": [W, H], "dpr": boxes.get("dpr"), "overall_mismatch_pct": round(total * 100, 1), "regions": scores}
    (out / "scores.json").write_text(json.dumps(res, indent=1))
    print(out)
    for k, v in scores.items():
        print(f"{k:14s} mismatch {v['mismatch_pct']:5.1f}%  mean diff {v['mean_diff']:5.1f}%")
    print(f"{'overall':14s} mismatch {res['overall_mismatch_pct']:5.1f}%")


if __name__ == "__main__":
    main()
