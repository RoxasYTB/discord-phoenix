import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import discord_intl as di

ap = argparse.ArgumentParser()
ap.add_argument("bundle")
ap.add_argument("cache")
ap.add_argument("--locales", default="en-US,fr,de,ja,pt-BR,ru,pl,zh-CN")
args = ap.parse_args()

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "phoenix-app", "locales")
with open(args.bundle, encoding="utf8") as f:
    rows = di.loaders(f.read())
locs = args.locales.split(",")
di.fetch(rows, args.cache, locs)
msgs = {loc: di.locale_messages(os.path.join(args.cache, loc)) for loc in locs}
phx_en = di.yml(os.path.join(root, "en.yml"))
phx_fr = di.yml(os.path.join(root, "fr.yml"))
others = [m for loc, m in msgs.items() if loc != "en-US"]
out = di.build_map(msgs["en-US"], phx_en, phx_fr, msgs.get("fr", {}), others)
with open(os.path.join(root, "discord.yml"), "w", encoding="utf8") as f:
    for k in phx_en:
        if k in out:
            f.write(f'{k}: "{out[k]}"\n')
print(f"{len(out)} of {len(phx_en)} keys mapped")
