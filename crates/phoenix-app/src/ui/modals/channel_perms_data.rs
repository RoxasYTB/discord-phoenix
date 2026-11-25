pub struct PermRow {
    pub bit: u64,
    pub label: &'static str,
    pub desc: &'static str,
}

pub struct PermSection {
    pub legend: &'static str,
    pub rows: &'static [PermRow],
}

pub const TEXT: &[PermSection] = &[
    PermSection {
        legend: "cperm.s0",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l0", desc: "cperm.d0" },
            PermRow { bit: 1 << 4, label: "cperm.l1", desc: "cperm.d1" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d2" },
            PermRow { bit: 1 << 29, label: "cperm.l3", desc: "cperm.d3" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d4" }] },
    PermSection {
        legend: "cperm.s2",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l5", desc: "cperm.d5" },
            PermRow { bit: 1 << 38, label: "cperm.l6", desc: "cperm.d6" },
            PermRow { bit: 1 << 35, label: "cperm.l7", desc: "cperm.d7" },
            PermRow { bit: 1 << 36, label: "cperm.l8", desc: "cperm.d8" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d9" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d10" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d11" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d12" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d13" },
            PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d14" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d15" },
            PermRow { bit: 1 << 51, label: "cperm.l16", desc: "cperm.d16" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d17" },
            PermRow { bit: 1 << 34, label: "cperm.l18", desc: "cperm.d18" },
            PermRow { bit: 1 << 16, label: "cperm.l19", desc: "cperm.d19" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d20" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d21" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d22" },
        ],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[
            PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d23" },
            PermRow { bit: 1 << 39, label: "cperm.l24", desc: "cperm.d24" },
            PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d25" },
        ],
    },
];

pub const NEWS: &[PermSection] = &[
    PermSection {
        legend: "cperm.s0",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l0", desc: "cperm.d0" },
            PermRow { bit: 1 << 4, label: "cperm.l1", desc: "cperm.d1" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d2" },
            PermRow { bit: 1 << 29, label: "cperm.l3", desc: "cperm.d3" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d4" }] },
    PermSection {
        legend: "cperm.s2",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l5", desc: "cperm.d26" },
            PermRow { bit: 1 << 38, label: "cperm.l6", desc: "cperm.d6" },
            PermRow { bit: 1 << 35, label: "cperm.l7", desc: "cperm.d7" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d9" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d10" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d11" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d12" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d13" },
            PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d14" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d27" },
            PermRow { bit: 1 << 51, label: "cperm.l16", desc: "cperm.d16" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d17" },
            PermRow { bit: 1 << 34, label: "cperm.l18", desc: "cperm.d18" },
            PermRow { bit: 1 << 16, label: "cperm.l19", desc: "cperm.d19" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d20" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d21" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d22" },
        ],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[
            PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d23" },
            PermRow { bit: 1 << 39, label: "cperm.l24", desc: "cperm.d24" },
            PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d25" },
        ],
    },
];

pub const VOICE: &[PermSection] = &[
    PermSection {
        legend: "cperm.s0",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l0", desc: "cperm.d28" },
            PermRow { bit: 1 << 4, label: "cperm.l1", desc: "cperm.d29" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d2" },
            PermRow { bit: 1 << 29, label: "cperm.l3", desc: "cperm.d3" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d30" }] },
    PermSection {
        legend: "cperm.s4",
        rows: &[
            PermRow { bit: 1 << 20, label: "cperm.l26", desc: "cperm.d31" },
            PermRow { bit: 1 << 21, label: "cperm.l27", desc: "cperm.d32" },
            PermRow { bit: 1 << 9, label: "cperm.l28", desc: "cperm.d33" },
            PermRow { bit: 1 << 42, label: "cperm.l29", desc: "cperm.d34" },
            PermRow { bit: 1 << 45, label: "cperm.l30", desc: "cperm.d35" },
            PermRow { bit: 1 << 25, label: "cperm.l31", desc: "cperm.d36" },
            PermRow { bit: 1 << 8, label: "cperm.l32", desc: "cperm.d37" },
            PermRow { bit: 1 << 22, label: "cperm.l33", desc: "cperm.d38" },
            PermRow { bit: 1 << 23, label: "cperm.l34", desc: "cperm.d39" },
            PermRow { bit: 1 << 24, label: "cperm.l35", desc: "cperm.d40" },
            PermRow { bit: 1 << 48, label: "cperm.l36", desc: "cperm.d41" },
        ],
    },
    PermSection {
        legend: "cperm.s5",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l5", desc: "cperm.d5" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d9" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d10" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d42" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d12" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d13" },
            PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d14" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d15" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d17" },
            PermRow { bit: 1 << 16, label: "cperm.l19", desc: "cperm.d19" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d20" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d21" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d22" },
        ],
    },
    PermSection {
        legend: "cperm.s6",
        rows: &[PermRow { bit: 1 << 44, label: "cperm.l37", desc: "cperm.d43" }, PermRow { bit: 1 << 33, label: "cperm.l38", desc: "cperm.d44" }],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[
            PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d23" },
            PermRow { bit: 1 << 39, label: "cperm.l24", desc: "cperm.d24" },
            PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d25" },
        ],
    },
];

pub const STAGE: &[PermSection] = &[
    PermSection {
        legend: "cperm.s0",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l0", desc: "cperm.d28" },
            PermRow { bit: 1 << 4, label: "cperm.l1", desc: "cperm.d45" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d2" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d30" }] },
    PermSection {
        legend: "cperm.s4",
        rows: &[
            PermRow { bit: 1 << 20, label: "cperm.l26", desc: "cperm.d46" },
            PermRow { bit: 1 << 9, label: "cperm.l28", desc: "cperm.d47" },
            PermRow { bit: 1 << 22, label: "cperm.l33", desc: "cperm.d48" },
            PermRow { bit: 1 << 24, label: "cperm.l35", desc: "cperm.d49" },
        ],
    },
    PermSection {
        legend: "cperm.s7",
        rows: &[PermRow { bit: 1 << 32, label: "cperm.l39", desc: "cperm.d50" }, PermRow { bit: 1 << 17, label: "cperm.l40", desc: "cperm.d51" }],
    },
    PermSection {
        legend: "cperm.s6",
        rows: &[PermRow { bit: 1 << 44, label: "cperm.l37", desc: "cperm.d43" }, PermRow { bit: 1 << 33, label: "cperm.l38", desc: "cperm.d44" }],
    },
    PermSection {
        legend: "cperm.s5",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l5", desc: "cperm.d5" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d9" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d10" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d11" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d12" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d13" },
            PermRow { bit: 1 << 17, label: "cperm.l40", desc: "cperm.d51" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d15" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d17" },
            PermRow { bit: 1 << 16, label: "cperm.l19", desc: "cperm.d19" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d20" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d21" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d22" },
        ],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d23" }, PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d25" }],
    },
];

pub const FORUM: &[PermSection] = &[
    PermSection {
        legend: "cperm.s0",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l0", desc: "cperm.d0" },
            PermRow { bit: 1 << 4, label: "cperm.l1", desc: "cperm.d1" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d2" },
            PermRow { bit: 1 << 29, label: "cperm.l3", desc: "cperm.d3" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d4" }] },
    PermSection {
        legend: "cperm.s8",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l41", desc: "cperm.d52" },
            PermRow { bit: 1 << 38, label: "cperm.l42", desc: "cperm.d53" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d9" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d10" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d11" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d12" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d13" },
            PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d14" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d15" },
            PermRow { bit: 1 << 51, label: "cperm.l16", desc: "cperm.d16" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d17" },
            PermRow { bit: 1 << 34, label: "cperm.l43", desc: "cperm.d54" },
            PermRow { bit: 1 << 16, label: "cperm.l44", desc: "cperm.d55" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d20" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d21" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d22" },
        ],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[
            PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d23" },
            PermRow { bit: 1 << 39, label: "cperm.l24", desc: "cperm.d56" },
            PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d57" },
        ],
    },
];

pub const CAT: &[PermSection] = &[
    PermSection {
        legend: "cperm.s9",
        rows: &[
            PermRow { bit: 1 << 10, label: "cperm.l45", desc: "cperm.d58" },
            PermRow { bit: 1 << 4, label: "cperm.l46", desc: "cperm.d59" },
            PermRow { bit: 1 << 28, label: "cperm.l2", desc: "cperm.d60" },
            PermRow { bit: 1 << 29, label: "cperm.l3", desc: "cperm.d61" },
        ],
    },
    PermSection { legend: "cperm.s1", rows: &[PermRow { bit: 1 << 0, label: "cperm.l4", desc: "cperm.d62" }] },
    PermSection {
        legend: "cperm.s2",
        rows: &[
            PermRow { bit: 1 << 11, label: "cperm.l47", desc: "cperm.d63" },
            PermRow { bit: 1 << 38, label: "cperm.l48", desc: "cperm.d64" },
            PermRow { bit: 1 << 35, label: "cperm.l7", desc: "cperm.d65" },
            PermRow { bit: 1 << 36, label: "cperm.l8", desc: "cperm.d66" },
            PermRow { bit: 1 << 14, label: "cperm.l9", desc: "cperm.d67" },
            PermRow { bit: 1 << 15, label: "cperm.l10", desc: "cperm.d68" },
            PermRow { bit: 1 << 6, label: "cperm.l11", desc: "cperm.d69" },
            PermRow { bit: 1 << 18, label: "cperm.l12", desc: "cperm.d70" },
            PermRow { bit: 1 << 37, label: "cperm.l13", desc: "cperm.d71" },
            PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d72" },
            PermRow { bit: 1 << 13, label: "cperm.l15", desc: "cperm.d73" },
            PermRow { bit: 1 << 51, label: "cperm.l16", desc: "cperm.d74" },
            PermRow { bit: 1 << 52, label: "cperm.l17", desc: "cperm.d75" },
            PermRow { bit: 1 << 34, label: "cperm.l49", desc: "cperm.d76" },
            PermRow { bit: 1 << 16, label: "cperm.l19", desc: "cperm.d77" },
            PermRow { bit: 1 << 12, label: "cperm.l20", desc: "cperm.d78" },
            PermRow { bit: 1 << 46, label: "cperm.l21", desc: "cperm.d79" },
            PermRow { bit: 1 << 49, label: "cperm.l22", desc: "cperm.d80" },
        ],
    },
    PermSection {
        legend: "cperm.s4",
        rows: &[
            PermRow { bit: 1 << 20, label: "cperm.l26", desc: "cperm.d81" },
            PermRow { bit: 1 << 21, label: "cperm.l27", desc: "cperm.d82" },
            PermRow { bit: 1 << 9, label: "cperm.l28", desc: "cperm.d83" },
            PermRow { bit: 1 << 42, label: "cperm.l29", desc: "cperm.d84" },
            PermRow { bit: 1 << 45, label: "cperm.l30", desc: "cperm.d35" },
            PermRow { bit: 1 << 25, label: "cperm.l31", desc: "cperm.d85" },
            PermRow { bit: 1 << 8, label: "cperm.l32", desc: "cperm.d86" },
            PermRow { bit: 1 << 22, label: "cperm.l33", desc: "cperm.d87" },
            PermRow { bit: 1 << 23, label: "cperm.l34", desc: "cperm.d88" },
            PermRow { bit: 1 << 24, label: "cperm.l35", desc: "cperm.d89" },
            PermRow { bit: 1 << 48, label: "cperm.l36", desc: "cperm.d41" },
        ],
    },
    PermSection {
        legend: "cperm.s3",
        rows: &[
            PermRow { bit: 1 << 31, label: "cperm.l23", desc: "cperm.d90" },
            PermRow { bit: 1 << 39, label: "cperm.l24", desc: "cperm.d91" },
            PermRow { bit: 1 << 50, label: "cperm.l25", desc: "cperm.d92" },
        ],
    },
    PermSection {
        legend: "cperm.s7",
        rows: &[PermRow { bit: 1 << 32, label: "cperm.l39", desc: "cperm.d93" }, PermRow { bit: 1 << 17, label: "cperm.l14", desc: "cperm.d72" }],
    },
    PermSection {
        legend: "cperm.s6",
        rows: &[PermRow { bit: 1 << 44, label: "cperm.l37", desc: "cperm.d94" }, PermRow { bit: 1 << 33, label: "cperm.l38", desc: "cperm.d95" }],
    },
];
