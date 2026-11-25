import json
import os
import sys

FIELD = {
    "year": {"2009": "{y}", "09": "{yy}", "2552": "{by}", "52": "{byy}"},
    "day": {"5": "{d}", "05": "{dd}"},
    "hour": {"3": "{h}", "03": "{hh}"},
    "minute": {"4": "{m}", "04": "{mm}"},
    "second": {"6": "{s}", "06": "{ss}"},
}


def pattern(parts):
    out = []
    for p in parts:
        t, v = p["type"], p["value"]
        if t in ("literal", "era"):
            out.append(v.replace("{", "(").replace("}", ")"))
        elif t == "month":
            out.append({"2": "{M}", "02": "{MM}"}.get(v, "{MMM}"))
        elif t == "weekday":
            out.append("{E}")
        elif t == "dayPeriod":
            out.append("{a}")
        elif t in FIELD:
            if v not in FIELD[t]:
                raise SystemExit(f"unexpected {t} value {v!r}")
            out.append(FIELD[t][v])
        else:
            raise SystemExit(f"unexpected part {t}")
    return "".join(out)


def number(parts, four):
    group = next((p["value"] for p in parts if p["type"] == "group"), "")
    dec = next((p["value"] for p in parts if p["type"] == "decimal"), ".")
    ints = [p["value"] for p in parts if p["type"] == "integer"]
    sizes = "3,2" if len(ints) > 2 and len(ints[1]) == 2 else "3"
    min_group = "1" if group and group in four else "2"
    return f"{group}|{dec}|{sizes}|{min_group}"


def main():
    src, dst = sys.argv[1], sys.argv[2]
    with open(src, encoding="utf8") as f:
        data = json.load(f)
    lines = []
    for loc in sorted(data):
        o = data[loc]
        lines.append(f"@{loc}")
        lines.append(f"hc\t{o['hourCycle']}")
        lines.append(f"first\t{o['week']['firstDay']}")
        lines.append(f"dir\t{o['dir']}")
        lines.append(f"num\t{number(o['num'], o['num4'])}")
        lines.append("plural\t" + "".join({"zero": "z", "one": "o", "two": "t", "few": "f", "many": "m", "other": "x"}[c] for c in o["plural"]))
        for style, by_hc in o["fmt"].items():
            base = pattern(by_hc["auto"][0])
            lines.append(f"{style}\t{base}")
            for hc in ("h12", "h23"):
                p = pattern(by_hc[hc][0])
                if p != base:
                    lines.append(f"{style}.{hc}\t{p}")
        for style, months in o["months"].items():
            lines.append(f"{style}.months\t" + "|".join(months))
        lines.append("weekdays\t" + "|".join(o["weekdays"]))
        for hc, periods in o["periods"].items():
            lines.append(f"periods.{hc}\t" + "|".join(periods))
    with open(dst, "w", encoding="utf8") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
