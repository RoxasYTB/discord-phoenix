import re
import sys
from pathlib import Path

CSS = Path(sys.argv[1] if len(sys.argv) > 1 else Path.home() / "discord-phoenix-ref/current-client/css/952007.8c2302e28f050501.css")
OUT = Path(__file__).resolve().parents[2] / "crates/phoenix-app/src/theme/client_mix.rs"

src = CSS.read_text()
blocks = dict(re.findall(r"@supports \(color:color-mix\(in lch,red,blue\)\)\{(\.theme-[a-z]+)\{([^}]*)\}\}", src))


def classify(body):
    out = []
    for decl in body.split(";"):
        if ":" not in decl:
            continue
        name, value = decl.split(":", 1)
        if not value.startswith("color-mix(in oklab,"):
            continue
        color = "text" if "--custom-theme-text-color," in value else "base"
        amount = "text" if "--custom-theme-text-color-amount" in value else "base"
        border = "--custom-theme-border-color-amount" in value
        out.append((name.strip(), (color == "text") * 1 + (amount == "text") * 2 + border * 4))
    return out


def table(name, entries):
    rows = "\n".join(f'    ("{n}", {k}),' for n, k in entries)
    return f"pub static {name}: &[(&str, u8)] = &[\n{rows}\n];\n"


OUT.write_text(table("DARK", classify(blocks[".theme-darker"])) + "\n" + table("LIGHT", classify(blocks[".theme-light"])))
print(OUT)
