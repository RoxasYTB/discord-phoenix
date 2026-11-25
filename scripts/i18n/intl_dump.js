(() => {
  const L = ["bg","cs","da","de","el","en-GB","en-US","es-419","es-ES","fi","fr","hi","hr","hu","id","it","ja","ko","lt","nl","no","pl","pt-BR","ro","ru","sv-SE","th","tr","uk","vi","zh-CN","zh-TW"];
  const S = { L: { dateStyle: "short" }, ll: { dateStyle: "medium" }, LL: { dateStyle: "long" }, LLL: { dateStyle: "long", timeStyle: "short" }, LLLL: { dateStyle: "full", timeStyle: "short" }, LT: { timeStyle: "short" }, LTS: { timeStyle: "medium" }, "L LT": { dateStyle: "short", timeStyle: "short" }, "L LTS": { dateStyle: "short", timeStyle: "medium" } };
  const dates = [new Date(2009, 1, 5, 3, 4, 6), new Date(2009, 10, 25, 15, 44, 56)];
  const out = {};
  for (const l of L) {
    const o = {};
    const pr = new Intl.PluralRules(l);
    o.plural = [];
    for (let n = 0; n <= 200; n++) o.plural.push(pr.select(n));
    o.fmt = {};
    for (const [k, s] of Object.entries(S)) {
      o.fmt[k] = {};
      for (const hc of [null, "h12", "h23"]) {
        const opt = { ...s };
        if (hc) opt.hourCycle = hc;
        o.fmt[k][hc || "auto"] = dates.map(d => new Intl.DateTimeFormat(l, opt).formatToParts(d));
      }
    }
    o.periods = {};
    for (const hc of ["auto", "h12"]) {
      const opt = { timeStyle: "short" };
      if (hc !== "auto") opt.hourCycle = hc;
      const f = new Intl.DateTimeFormat(l, opt);
      o.periods[hc] = [];
      for (let h = 0; h < 24; h++) o.periods[hc].push(f.formatToParts(new Date(2009, 1, 5, h, 30)).find(p => p.type === "dayPeriod")?.value ?? "");
    }
    o.hourCycle = new Intl.DateTimeFormat(l, { timeStyle: "short" }).resolvedOptions().hourCycle;
    o.months = {};
    for (const k of ["ll", "LL", "LLLL"]) {
      o.months[k] = [];
      for (let m = 0; m < 12; m++) o.months[k].push(new Intl.DateTimeFormat(l, S[k]).formatToParts(new Date(2009, m, 5)).find(p => p.type === "month")?.value ?? "");
    }
    o.weekdays = [];
    for (let d = 0; d < 7; d++) o.weekdays.push(new Intl.DateTimeFormat(l, S.LLLL).formatToParts(new Date(2009, 1, 2 + d)).find(p => p.type === "weekday")?.value ?? "");
    o.num = new Intl.NumberFormat(l).formatToParts(1234567.891);
    o.num4 = new Intl.NumberFormat(l).format(1234);
    const loc = new Intl.Locale(l);
    o.week = loc.getWeekInfo?.() ?? loc.weekInfo;
    o.dir = (loc.getTextInfo?.() ?? loc.textInfo)?.direction;
    out[l] = o;
  }
  return JSON.stringify(out);
})()
