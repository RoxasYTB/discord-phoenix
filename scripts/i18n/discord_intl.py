import collections
import concurrent.futures as cf
import json
import os
import re
import urllib.request

LOADER = re.compile(r'createLoader:\w+\}=n\(\d+\),\w+=\w+\(\{(.*?)\},"en-US"\)')
ENTRY = re.compile(r'(?:"([\w-]+)"|(\w+)):\(\)=>n\.e\((\d+)\)')
CHUNK = re.compile(r'(\d+):"([0-9a-f]{16})"')
PARSE = re.compile(r"JSON\.parse\('((?:[^'\\]|\\.)*)'\)")


def loaders(bundle):
    cmap = dict(CHUNK.findall(bundle))
    out = []
    for block in LOADER.findall(bundle):
        row = {}
        for m in ENTRY.finditer(block):
            cid = m.group(3)
            if cid in cmap:
                row[m.group(1) or m.group(2)] = cmap[cid] + ".js"
        out.append(row)
    return out


def fetch(loader_rows, outdir, locales):
    jobs = [(loc, i, row[loc]) for i, row in enumerate(loader_rows) for loc in locales if loc in row]

    def get(job):
        loc, i, name = job
        path = os.path.join(outdir, loc, f"{i:02d}.js")
        if os.path.exists(path):
            return
        os.makedirs(os.path.dirname(path), exist_ok=True)
        req = urllib.request.Request("https://discord.com/assets/" + name, headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=30) as r:
            data = r.read()
        with open(path, "wb") as f:
            f.write(data)

    with cf.ThreadPoolExecutor(6) as ex:
        list(ex.map(get, jobs))


def unjs(t):
    out = []
    i = 0
    while i < len(t):
        c = t[i]
        if c == "\\":
            n = t[i + 1]
            if n == "x":
                out.append(chr(int(t[i + 2:i + 4], 16)))
                i += 4
                continue
            if n == "u":
                out.append(chr(int(t[i + 2:i + 6], 16)))
                i += 6
                continue
            out.append({"n": "\n", "t": "\t", "r": "\r", "b": "\b", "f": "\f", "0": "\0"}.get(n, n))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out).encode("utf-16", "surrogatepass").decode("utf-16")


def messages(js):
    m = {}
    for g in PARSE.finditer(js):
        m.update(json.loads(unjs(g.group(1))))
    return m


def locale_messages(d):
    m = {}
    for f in sorted(os.listdir(d)):
        with open(os.path.join(d, f), encoding="utf8") as fh:
            m.update(messages(fh.read()))
    return m


def yml(path):
    m = {}
    with open(path, encoding="utf8") as fh:
        for line in fh:
            s = line.strip()
            if not s or s.startswith("#") or ":" not in s:
                continue
            k, v = s.split(":", 1)
            v = v.strip()
            if v.startswith('"') and v.endswith('"'):
                v = v[1:-1]
            m[k.strip()] = v.replace('\\"', '"').replace("\\n", "\n")
    return m


def variants(nodes, plural=None):
    outs = [("", [])]
    for n in nodes:
        if isinstance(n, str):
            outs = [(o + n, a) for o, a in outs]
            continue
        t = n[0]
        if t in (1, 2, 3, 4):
            outs = [(o + "{}", a + [n[1]]) for o, a in outs]
            continue
        if t == 7:
            outs = [(o + "{}", a + [plural or "#"]) for o, a in outs]
            continue
        if t == 8:
            sub = variants(n[2], plural) if len(n) > 2 else [("", [])]
        elif t in (5, 6):
            sub = []
            for ch in n[2].values():
                sub.extend(variants(ch, n[1] if t == 6 else plural))
        else:
            sub = [("", [])]
        outs = [(o + s, a + sa) for o, a in outs for s, sa in sub[:8]][:64]
    return outs


def norm(s):
    for a, b in (("…", "..."), ("’", "'"), ("‘", "'"), ("“", '"'), ("”", '"'), (" ", " ")):
        s = s.replace(a, b)
    s = re.sub(r"\*\*|__", "", s)
    return " ".join(s.split())


def phoenix_text(v):
    return norm(re.sub(r"\{\w+\}", "{}", v)), re.findall(r"\{(\w+)\}", v)


def has_hook(n):
    if isinstance(n, list):
        if n and n[0] == 8 and len(n) > 2 and n[2] == []:
            return True
        return any(has_hook(x) for x in n)
    if isinstance(n, dict):
        return any(has_hook(v) for v in n.values())
    return False


def build_map(en, phx_en, phx_fr, fr, others):
    idx = collections.defaultdict(list)
    for h, v in en.items():
        if has_hook(v):
            continue
        for s, a in variants(v):
            ns = norm(s)
            idx[ns].append((h, a, None))
            if ns.upper() != ns:
                idx[ns.upper()].append((h, a, "upper"))
    out = {}
    for k, v in phx_en.items():
        text, names = phoenix_text(v)
        if not text.strip("{} .,:"):
            continue
        cands = [c for c in idx.get(text, []) if len(c[1]) == len(names) and (c[2] is None or text == text.upper())]
        if not cands:
            continue
        if any(c[2] is None for c in cands):
            cands = [c for c in cands if c[2] is None]
        fy = phoenix_text(phx_fr.get(k, ""))[0]

        def score(c):
            h = c[0]
            present = sum(1 for o in others if h in o)
            fv = [norm(s) for s, _ in variants(fr.get(h, []))]
            return (-present, 0 if fy in fv else 1, -sum(1 for y in cands if fr.get(y[0]) == fr.get(h)), h)

        cands.sort(key=score)
        h, a, tr = cands[0]
        ren = [f"{p}={d}" for p, d in dict(zip(names, a)).items() if p != d]
        out[k] = " ".join([h] + ren + ([f"^{tr}"] if tr else []))
    return out
