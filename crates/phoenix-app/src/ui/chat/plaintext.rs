use std::{cell::RefCell, collections::HashMap};

use futures::AsyncReadExt;
use gpui::{
    AnyElement, App, Entity, FontWeight, IntoElement, ParentElement, SharedString, StyledText, Styled, div,
    http_client::{AsyncBody, Request},
    prelude::*,
};
use gpui_component::input::{Input, InputState};

use crate::{
    model::Attachment,
    theme::{c, generated::idx, tok, z},
    ui::context_menu::MenuItem,
};

pub const LANGUAGES: &[&str] = &[
    "1c",
    "4d",
    "abnf",
    "accesslog",
    "ada",
    "arduino",
    "ino",
    "armasm",
    "arm",
    "avrasm",
    "actionscript",
    "as",
    "ass",
    "ssa",
    "alan",
    "ansi",
    "i",
    "log",
    "ln",
    "angelscript",
    "asc",
    "apache",
    "apacheconf",
    "applescript",
    "osascript",
    "arcade",
    "asciidoc",
    "adoc",
    "aspectj",
    "autohotkey",
    "autoit",
    "awk",
    "mawk",
    "nawk",
    "gawk",
    "bash",
    "sh",
    "zsh",
    "basic",
    "bbcode",
    "blade",
    "bnf",
    "brainfuck",
    "bf",
    "csharp",
    "cs",
    "c",
    "h",
    "cpp",
    "hpp",
    "cc",
    "hh",
    "c++",
    "h++",
    "cxx",
    "hxx",
    "cal",
    "cos",
    "cls",
    "cmake",
    "cmake.in",
    "coq",
    "csp",
    "css",
    "csv",
    "capnproto",
    "capnp",
    "chaos",
    "kaos",
    "chapel",
    "chpl",
    "cisco",
    "clojure",
    "clj",
    "coffeescript",
    "coffee",
    "cson",
    "iced",
    "cpc",
    "crmsh",
    "crm",
    "pcmk",
    "crystal",
    "cr",
    "cypher",
    "d",
    "dns",
    "zone",
    "bind",
    "dos",
    "bat",
    "cmd",
    "dart",
    "delphi",
    "dpr",
    "dfm",
    "pas",
    "pascal",
    "freepascal",
    "lazarus",
    "lpr",
    "lfm",
    "diff",
    "patch",
    "django",
    "jinja",
    "dockerfile",
    "docker",
    "dsconfig",
    "dts",
    "dust",
    "dst",
    "dylan",
    "ebnf",
    "elixir",
    "ex",
    "elm",
    "erlang",
    "erl",
    "extempore",
    "xtlang",
    "xtm",
    "fsharp",
    "fs",
    "fix",
    "fortran",
    "f90",
    "f95",
    "gcode",
    "nc",
    "gams",
    "gms",
    "gauss",
    "gss",
    "godot",
    "gdscript",
    "gherkin",
    "gn",
    "gni",
    "go",
    "golang",
    "gf",
    "golo",
    "gololang",
    "gradle",
    "groovy",
    "xml",
    "html",
    "xhtml",
    "rss",
    "atom",
    "xjb",
    "xsd",
    "xsl",
    "plist",
    "svg",
    "http",
    "https",
    "haml",
    "handlebars",
    "hbs",
    "html.hbs",
    "html.handlebars",
    "haskell",
    "hs",
    "haxe",
    "hx",
    "hy",
    "hylang",
    "ini",
    "toml",
    "inform7",
    "i7",
    "irpf90",
    "json",
    "java",
    "jsp",
    "javascript",
    "js",
    "jsx",
    "jolie",
    "iol",
    "ol",
    "julia",
    "julia-repl",
    "kotlin",
    "kt",
    "tex",
    "leaf",
    "lean",
    "lasso",
    "ls",
    "lassoscript",
    "less",
    "ldif",
    "lisp",
    "livecodeserver",
    "livescript",
    "lock",
    "ls",
    "lua",
    "makefile",
    "mk",
    "mak",
    "make",
    "markdown",
    "md",
    "mkdown",
    "mkd",
    "mathematica",
    "mma",
    "wl",
    "matlab",
    "maxima",
    "mel",
    "mercury",
    "mirc",
    "mrc",
    "mizar",
    "mojolicious",
    "monkey",
    "moonscript",
    "moon",
    "n1ql",
    "nsis",
    "never",
    "nginx",
    "nginxconf",
    "nim",
    "nimrod",
    "nix",
    "ocl",
    "ocaml",
    "ml",
    "objectivec",
    "mm",
    "objc",
    "obj-c",
    "obj-c++",
    "objective-c++",
    "glsl",
    "openscad",
    "scad",
    "ruleslanguage",
    "oxygene",
    "pf",
    "pf.conf",
    "php",
    "php3",
    "php4",
    "php5",
    "php6",
    "php7",
    "parser3",
    "perl",
    "pl",
    "pm",
    "plaintext",
    "txt",
    "text",
    "pony",
    "pgsql",
    "postgres",
    "postgresql",
    "powershell",
    "ps",
    "ps1",
    "processing",
    "prolog",
    "properties",
    "proto",
    "protobuf",
    "puppet",
    "pp",
    "python",
    "py",
    "gyp",
    "profile",
    "python-repl",
    "pycon",
    "k",
    "kdb",
    "qml",
    "r",
    "cshtml",
    "razor",
    "razor-cshtml",
    "reasonml",
    "re",
    "redbol",
    "rebol",
    "red",
    "red-system",
    "rib",
    "rsl",
    "graph",
    "instances",
    "robot",
    "rf",
    "rpm-specfile",
    "rpm",
    "spec",
    "rpm-spec",
    "specfile",
    "ruby",
    "rb",
    "gemspec",
    "podspec",
    "thor",
    "irb",
    "rust",
    "rs",
    "SAS",
    "sas",
    "scss",
    "sql",
    "p21",
    "step",
    "stp",
    "scala",
    "scheme",
    "scilab",
    "sci",
    "shexc",
    "shell",
    "console",
    "smali",
    "smalltalk",
    "st",
    "sml",
    "ml",
    "solidity",
    "sol",
    "stan",
    "stanfuncs",
    "stata",
    "iecst",
    "scl",
    "structured-text",
    "stylus",
    "styl",
    "subunit",
    "supercollider",
    "sc",
    "srt",
    "svelte",
    "swift",
    "tcl",
    "tk",
    "terraform",
    "tf",
    "hcl",
    "tap",
    "thrift",
    "tp",
    "tsql",
    "ttml",
    "twig",
    "craftcms",
    "typescript",
    "ts",
    "tsx",
    "unicorn-rails-log",
    "vbnet",
    "vb",
    "vba",
    "vbscript",
    "vbs",
    "vhdl",
    "vala",
    "verilog",
    "v",
    "vim",
    "vtt",
    "axapta",
    "x++",
    "x86asm",
    "xl",
    "tao",
    "xquery",
    "xpath",
    "xq",
    "yml",
    "yaml",
    "zephir",
    "zep",
];

pub const MARKDOWN: &[&str] = &["markdown", "md", "mkd", "mkdown"];
pub const RANGE: &str = "bytes=0-50000";
pub const COLLAPSED_LINES: usize = 6;
pub const EXPANDED_LINES: usize = 100;

pub fn extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

pub fn previewable(name: &str) -> bool {
    LANGUAGES.contains(&extension(name))
}

#[derive(Clone, Default)]
pub struct Entry {
    pub text: Option<String>,
    pub bytes_left: u64,
    pub failed: bool,
    pub expanded: bool,
    pub wrap: bool,
    pub markdown: bool,
    pub language: Option<String>,
}

impl Entry {
    pub fn language(&self, filename: &str) -> String {
        self.language.clone().unwrap_or_else(|| extension(filename).to_owned())
    }

    pub fn renders_markdown(&self, filename: &str) -> bool {
        self.markdown && MARKDOWN.contains(&self.language(filename).as_str())
    }
}

pub fn filter_languages(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase();
    LANGUAGES.iter().copied().filter(|l| q.is_empty() || l.contains(q.as_str())).collect()
}

struct LangPicker {
    url: SharedString,
    pos: gpui::Point<gpui::Pixels>,
    above: bool,
    search: Entity<InputState>,
}

thread_local! {
    static ENTRIES: RefCell<HashMap<SharedString, Entry>> = RefCell::new(HashMap::new());
    static MENU: RefCell<Option<(SharedString, gpui::Point<gpui::Pixels>)>> = const { RefCell::new(None) };
    static LANG: RefCell<Option<LangPicker>> = const { RefCell::new(None) };
}

fn entry(url: &SharedString) -> Option<Entry> {
    ENTRIES.with(|e| e.borrow().get(url).cloned())
}

fn update(url: &SharedString, f: impl FnOnce(&mut Entry)) {
    ENTRIES.with(|e| f(e.borrow_mut().entry(url.clone()).or_insert_with(|| Entry { wrap: true, markdown: true, ..Default::default() })));
}

pub fn remaining(content_range: Option<&str>, received: u64) -> u64 {
    content_range.and_then(|r| r.rsplit('/').next()).and_then(|t| t.parse::<u64>().ok()).map_or(0, |total| total.saturating_sub(received))
}

pub fn visible(text: &str, expanded: bool, bytes_left: u64) -> (String, Option<(bool, usize)>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let limit = if expanded { EXPANDED_LINES } else { COLLAPSED_LINES };
    let shown = lines.iter().take(limit).copied().collect::<Vec<_>>().join("\n");
    let whole = bytes_left == 0;
    let more = lines.len().saturating_sub(limit);
    let notice = if whole && expanded && more > 0 {
        Some((true, more))
    } else if !whole || more > 0 {
        Some((whole, more))
    } else {
        None
    };
    (shown, notice)
}

static INVISIBLE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\p{Cf}|\p{Zl}|\p{Zp}|[\p{Cc}&&[^\n\t]]|[\p{Zs}&&[^ ]]|\x{34F}|\x{17B4}|\x{17B5}|\x{1160}|\x{3164}|\x{FFA0}").expect("static regex")
});

pub fn sanitize(text: &str) -> String {
    INVISIBLE.replace_all(text, "").into_owned()
}

fn fetch(url: SharedString, cx: &App) {
    update(&url, |_| {});
    let http = cx.http_client();
    cx.spawn(async move |cx| {
        let req = Request::builder().uri(url.as_ref()).header("Range", RANGE).header("Accept", "text/plain").body(AsyncBody::default());
        let result = match req {
            Ok(req) => http.send(req).await.ok(),
            Err(_) => None,
        };
        let mut done = None;
        if let Some(resp) = result.filter(|r| r.status().is_success()) {
            let range = resp.headers().get("content-range").and_then(|v| v.to_str().ok()).map(str::to_owned);
            let mut body = resp.into_body();
            let mut bytes = Vec::new();
            if body.read_to_end(&mut bytes).await.is_ok() {
                let left = remaining(range.as_deref(), bytes.len() as u64);
                let mut text = String::from_utf8_lossy(&bytes).into_owned();
                if left > 0 {
                    text.pop();
                }
                done = Some((sanitize(&text), left));
            }
        }
        update(&url, |e| match done {
            Some((text, left)) => {
                e.text = Some(text);
                e.bytes_left = left;
            }
            None => e.failed = true,
        });
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

fn tk(i: usize) -> gpui::Hsla {
    c(tok(i))
}

fn icon_button(id: impl Into<gpui::ElementId>, icon: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .cursor_pointer()
        .text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT))
        .hover(|s| s.text_color(tk(idx::INTERACTIVE_TEXT_HOVER)))
        .child(gpui::svg().path(crate::icons::path(icon)).size(z(20.)).text_color(tk(idx::INTERACTIVE_TEXT_DEFAULT)))
}

fn lang_button(id: &'static str, url: SharedString, above: bool) -> gpui::Stateful<gpui::Div> {
    icon_button(id, "angle-brackets").tooltip(crate::ui::tooltip::tip(crate::t!("plaintext.language"))).on_click(move |ev, w, cx: &mut App| open_language_picker(url.clone(), ev.position(), above, w, cx))
}

pub fn open_language_picker(url: SharedString, pos: gpui::Point<gpui::Pixels>, above: bool, w: &mut gpui::Window, cx: &mut App) {
    let search = cx.new(|cx| InputState::new(w, cx).placeholder(crate::t!("plaintext.language_search")));
    search.update(cx, |s, cx| s.focus(w, cx));
    LANG.with(|l| *l.borrow_mut() = Some(LangPicker { url, pos, above, search }));
    w.refresh();
}

pub fn set_language(url: &SharedString, language: &str) {
    update(url, |e| e.language = Some(language.to_owned()));
}

fn close_lang(w: &mut gpui::Window) {
    LANG.with(|l| *l.borrow_mut() = None);
    w.refresh();
}

fn lang_popout(url: &SharedString, current: &str, defer: bool, cx: &App) -> Option<AnyElement> {
    let (pos, above, search) = LANG.with(|l| l.borrow().as_ref().filter(|p| p.url == *url).map(|p| (p.pos, p.above, p.search.clone())))?;
    let query = search.read(cx).value().to_string();
    let mut list = div().id("plaintext-lang-list").max_h(z(240.)).overflow_y_scroll().mt(z(4.)).flex().flex_col();
    for (n, l) in filter_languages(&query).into_iter().enumerate() {
        let sel = l == current;
        let u = url.clone();
        list = list.child(
            div()
                .id(("plaintext-lang-opt", n))
                .px(z(8.))
                .py(z(6.))
                .rounded(z(4.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .text_size(z(14.))
                .line_height(z(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tk(if sel { idx::TEXT_STRONG } else { idx::TEXT_DEFAULT }))
                .when(sel, |d| d.bg(tk(idx::BACKGROUND_MOD_NORMAL)))
                .hover(|d| d.bg(tk(idx::BACKGROUND_MOD_SUBTLE)))
                .on_click(move |_, w, _| {
                    update(&u, |e| e.language = Some(l.to_owned()));
                    close_lang(w);
                })
                .child(l)
                .when(sel, |d| d.child(gpui::svg().path(crate::icons::path("check")).size(z(16.)).text_color(tk(idx::BRAND_500)))),
        );
    }
    let card = div()
        .id("plaintext-lang")
        .occlude()
        .w(z(176.))
        .pt(z(8.))
        .px(z(8.))
        .pb(z(4.))
        .rounded(z(4.))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .shadow_lg()
        .on_mouse_down_out(|_, w, _| close_lang(w))
        .child(
            div()
                .h(z(32.))
                .px(z(8.))
                .rounded(z(4.))
                .bg(tk(idx::INPUT_BACKGROUND_DEFAULT))
                .border_1()
                .border_color(tk(idx::INPUT_BORDER_DEFAULT))
                .flex()
                .items_center()
                .text_size(z(14.))
                .child(Input::new(&search).appearance(false).bordered(false)),
        )
        .child(list);
    let corner = if above { gpui::Anchor::BottomRight } else { gpui::Anchor::TopRight };
    let at = gpui::point(pos.x - z(12.), pos.y);
    let el = gpui::anchored().position(at).anchor(corner).snap_to_window_with_margin(z(8.)).child(card);
    Some(if defer { gpui::deferred(el).with_priority(20).into_any_element() } else { el.into_any_element() })
}

fn markdown_view(id: &'static str, text: &str, notice: Option<&str>) -> AnyElement {
    let style = gpui_component::text::TextViewStyle::default().paragraph_gap(gpui::rems(0.5)).heading_font_size(|level, _| match level {
        1 => z(24.),
        2 => z(20.),
        _ => z(16.),
    });
    div()
        .p(z(8.))
        .text_size(z(16.))
        .line_height(z(22.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .child(gpui_component::text::TextView::markdown(id, text.to_owned()).style(style))
        .children(notice.filter(|n| !n.is_empty()).map(|n| div().mt(z(4.)).text_color(tk(idx::TEXT_MUTED)).child(n.to_owned())))
        .into_any_element()
}

pub fn size_label(bytes: u64) -> SharedString {
    let kb = bytes as f64 / 1000.;
    if kb / 1000. < 1. {
        return crate::t!("size.kb", size = kb.ceil() as u64);
    }
    let mb = (kb / 1000.).ceil();
    if mb < 1000. {
        return crate::t!("size.mb", size = mb as u64);
    }
    let gb = (mb / 100.).round() / 10.;
    let n = if gb.fract() == 0. { format!("{gb:.0}") } else { format!("{gb:.1}").replace('.', crate::t!("size.decimal").as_ref()) };
    crate::t!("size.gb", size = n)
}

pub fn render(a: &Attachment, cx: &App) -> Option<AnyElement> {
    let url = a.url.clone();
    let Some(e) = entry(&url) else {
        fetch(url, cx);
        return Some(div().mt(z(4.)).min_w(z(302.)).h(z(118.)).rounded_t(z(4.)).border_1().border_color(tk(idx::BORDER_SUBTLE)).bg(tk(idx::BACKGROUND_BASE_LOWER)).into_any_element());
    };
    if e.failed {
        return None;
    }
    let lang = e.language(&a.filename);
    let as_md = e.renders_markdown(&a.filename);
    let text = e.text.clone().unwrap_or_default();
    let lines = text.split('\n').count();
    let (shown, notice) = visible(&text, e.expanded, e.bytes_left);
    let notice = notice.map(|(whole, more)| {
        if whole {
            if more == 1 { crate::t!("plaintext.line_left", n = more) } else { crate::t!("plaintext.lines_left", n = more) }
        } else {
            crate::t!("plaintext.bytes_left", size = size_label(e.bytes_left))
        }
    });
    let suffix = match &notice {
        Some(n) if e.expanded && e.bytes_left == 0 => format!("\n... {n}"),
        Some(n) => format!("... {n}"),
        None => String::new(),
    };
    let card_w = {
        let ts = cx.text_system();
        let id = ts.resolve_font(&gpui::font(crate::theme::FONT_MONO()));
        let widest = format!("{shown}{suffix}")
            .lines()
            .map(|l| l.chars().map(|ch| ts.advance(id, gpui::px(14.), ch).map(|a| f32::from(a.width)).unwrap_or(0.)).sum::<f32>())
            .fold(0., f32::max);
        let half = crate::ui::title_bar::viewport_w_css() * 0.5;
        (widest + 18.).min(half).max(302.)
    };
    let code = if as_md {
        markdown_view("plaintext-md", &shown, Some(suffix.trim()))
    } else {
        let body = format!("{shown}{suffix}");
        let runs = crate::richtext::code_runs(&body, Some(&lang));
        div()
            .min_h(z(100.))
            .p(z(8.))
            .font_family(crate::theme::FONT_MONO())
            .text_size(z(14.))
            .line_height(z(16.))
            .text_color(tk(idx::TEXT_CODE))
            .when(!e.wrap, |d| d.whitespace_nowrap())
            .child(StyledText::new(body).with_runs(runs))
            .into_any_element()
    };
    let max_h = if e.expanded || as_md { 500. } else { (COLLAPSED_LINES as f32 * 16.).max(100.) + 26. };
    let text_box = div()
        .id(("plaintext-body", a.size))
        .min_h(z(118.))
        .max_h(z(max_h))
        .overflow_hidden()
        .when(e.expanded, |d| d.overflow_y_scroll())
        .rounded_t(z(4.))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_BASE_LOWER))
        .child(code);
    let can_expand = e.expanded || COLLAPSED_LINES < lines;
    let count = if e.bytes_left == 0 {
        if lines == 1 { crate::t!("plaintext.line", n = lines) } else { crate::t!("plaintext.lines", n = lines) }
    } else {
        crate::t!("plaintext.more_than", n = lines)
    };
    let toggle_label: SharedString = format!("{} ({count})", if e.expanded { crate::t!("plaintext.collapse") } else { crate::t!("plaintext.expand") }).into();
    let u1 = url.clone();
    let u2 = url.clone();
    let expand = can_expand.then(|| {
        div().mr(z(8.)).child(
            div()
                .id("plaintext-expand")
                .size(z(32.))
                .rounded(z(8.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .border_1()
                .border_color(tk(idx::CONTROL_SECONDARY_BORDER_DEFAULT))
                .bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_DEFAULT))
                .hover(|s| s.bg(tk(idx::CONTROL_SECONDARY_BACKGROUND_HOVER)))
                .tooltip(crate::ui::tooltip::tip(toggle_label))
                .on_click(move |_, w, _| {
                    update(&u1, |e| e.expanded = !e.expanded);
                    w.refresh();
                })
                .child(gpui::svg().path(crate::icons::path(if e.expanded { "chevron-up" } else { "chevron" })).size(z(20.)).text_color(tk(idx::CONTROL_SECONDARY_TEXT_DEFAULT))),
        )
    });
    let name = div()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(z(14.)).line_height(z(18.)).text_color(tk(idx::TEXT_DEFAULT)).child(a.filename.clone()))
        .child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::TEXT_SUBTLE)).child(size_label(a.size)));
    let open_url = a.url.clone();
    let footer = div()
        .flex()
        .items_center()
        .justify_end()
        .min_w_0()
        .p(z(12.))
        .rounded_b(z(8.))
        .border_1()
        .border_t_0()
        .border_color(tk(idx::BORDER_SUBTLE))
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .text_size(z(14.))
        .text_color(tk(idx::TEXT_DEFAULT))
        .children(expand)
        .child(name)
        .child(div().flex_grow(200.).min_w(z(16.)))
        .child(lang_button("plaintext-lang", url.clone(), false))
        .child(icon_button("plaintext-full", "maximize").ml(z(16.)).tooltip(crate::ui::tooltip::tip(crate::t!("plaintext.whole_file"))).on_click({
            let filename = a.filename.clone();
            let size = a.size;
            move |_, w, _| {
                crate::actions::open_modal(crate::ui::modals::Modal::PlaintextFull { url: open_url.clone(), filename: filename.clone(), size, viewport: None }, Some(w));
            }
        }))
        .child(icon_button("plaintext-more", "more-horizontal").ml(z(16.)).mr(z(4.)).tooltip(crate::ui::tooltip::tip(crate::t!("plaintext.more"))).on_click(move |ev, w, _| {
            let p = ev.position();
            MENU.with(|m| *m.borrow_mut() = Some((u2.clone(), p)));
            w.refresh();
        }));
    let menu = MENU.with(|m| m.borrow().clone()).filter(|(u, _)| *u == url).map(|(u, pos)| {
        let md = MARKDOWN.contains(&lang.as_str());
        let dl = u.clone();
        let wrap_url = u.clone();
        let md_url = u.clone();
        let mut items = vec![
            MenuItem::action(crate::t!("plaintext.download"), Some(crate::icons::path("download")), move |_, _, cx: &mut App| {
                MENU.with(|m| *m.borrow_mut() = None);
                cx.open_url(&dl);
            }),
            MenuItem::check(crate::t!("plaintext.wrap"), e.wrap, move |_, w, _| {
                update(&wrap_url, |e| e.wrap = !e.wrap);
                w.refresh();
            }),
        ];
        if md {
            items.push(MenuItem::check(crate::t!("plaintext.markdown"), e.markdown, move |_, w, _| {
                update(&md_url, |e| e.markdown = !e.markdown);
                w.refresh();
            }));
        }
        let dismiss: crate::ui::context_menu::Dismiss = std::rc::Rc::new(|w, _| {
            MENU.with(|m| *m.borrow_mut() = None);
            w.refresh();
        });
        crate::ui::context_menu::render_anchored(pos, gpui::Anchor::BottomLeft, items, dismiss)
    });
    let picker = lang_popout(&url, &lang, true, cx);
    Some(div().mt(z(4.)).w(z(card_w)).max_w_full().flex().flex_col().child(text_box).child(footer).children(menu).children(picker).into_any_element())
}

pub fn render_full(url: &SharedString, filename: &SharedString, size: u64, viewport: Option<gpui::Size<gpui::Pixels>>, cx: &mut gpui::Context<crate::app::PhoenixApp>) -> AnyElement {
    let e = entry(url).unwrap_or_default();
    let lang = e.language(filename);
    let as_md = e.renders_markdown(filename);
    let text = e.text.clone().unwrap_or_default();
    let notice = if e.bytes_left > 0 { format!("... {}", crate::t!("plaintext.bytes_left", size = size_label(e.bytes_left))) } else { String::new() };
    let content = if as_md {
        markdown_view("plaintext-full-md", &text, Some(&notice))
    } else {
        let body = format!("{text}{notice}");
        let runs = crate::richtext::code_runs(&body, Some(&lang));
        div()
            .p(z(8.))
            .font_family(crate::theme::FONT_MONO())
            .text_size(z(14.))
            .line_height(z(16.))
            .text_color(tk(idx::TEXT_CODE))
            .when(!e.wrap, |d| d.whitespace_nowrap())
            .child(StyledText::new(body).with_runs(runs))
            .into_any_element()
    };
    let zoom = crate::theme::zoom();
    let (vw, vh) = viewport.map_or((1280., 800.), |v| (f32::from(v.width) / zoom, f32::from(v.height) / zoom));
    let w = (vw * 0.8).min(1280.);
    let h = vh * 0.8;
    let close = crate::ui::modals::close_action(cx);
    let card = div()
        .w(z(w))
        .h(z(h))
        .rounded(z(12.))
        .overflow_hidden()
        .bg(tk(idx::BACKGROUND_SURFACE_HIGH))
        .border_1()
        .border_color(tk(idx::BORDER_SUBTLE))
        .flex()
        .flex_col()
        .pb(z(16.))
        .child(
            div()
                .id("plaintext-full-body")
                .flex_1()
                .min_h_0()
                .overflow_scroll()
                .bg(tk(idx::BACKGROUND_BASE_LOWER))
                .child(content),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .pt(z(16.))
                .px(z(16.))
                .border_t_1()
                .border_color(tk(idx::BORDER_SUBTLE))
                .text_size(z(14.))
                .text_color(tk(idx::TEXT_DEFAULT))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_size(z(14.)).line_height(z(18.)).child(filename.clone()))
                        .child(div().text_size(z(12.)).line_height(z(16.)).text_color(tk(idx::TEXT_SUBTLE)).child(size_label(size))),
                )
                .child(div().flex_grow(200.).min_w(z(16.)))
                .child(lang_button("plaintext-full-lang", url.clone(), true)),
        )
        .children(lang_popout(url, &lang, false, cx));
    crate::ui::modal::frame("plaintext-full", close, card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetched_text_drops_the_characters_the_client_strips() {
        assert_eq!(sanitize("a\r\nb\tc\u{FEFF}d\u{200B}e\u{00A0}f\u{3164}g\u{2028}h"), "a\nb\tcdefgh");
        assert_eq!(sanitize("fn main() {\n    x\n}"), "fn main() {\n    x\n}");
    }

    #[test]
    fn previewable_extensions_come_from_the_client_list() {
        assert!(previewable("main.rs"));
        assert!(previewable("notes.txt") == LANGUAGES.contains(&"txt"));
        assert!(previewable("README.md"));
        assert!(!previewable("photo.png"));
        assert_eq!(remaining(Some("bytes 0-50000/120000"), 50001), 69999);
        assert_eq!(remaining(None, 10), 0);
    }

    #[test]
    fn size_label_follows_the_client_decimal_units() {
        assert_eq!(size_label(512), crate::t!("size.kb", size = 1));
        assert_eq!(size_label(69_999), crate::t!("size.kb", size = 70));
        assert_eq!(size_label(4_213_337), crate::t!("size.mb", size = 5));
        assert_eq!(size_label(2_000_000_000), crate::t!("size.gb", size = 2));
        assert!(size_label(1_500_000_000).starts_with('1'));
    }

    #[test]
    fn language_choice_drives_highlight_and_markdown() {
        let mut e = Entry { markdown: true, ..Default::default() };
        assert_eq!(e.language("notes.md"), "md");
        assert!(e.renders_markdown("notes.md"));
        assert!(!e.renders_markdown("main.rs"));
        e.language = Some("python".into());
        assert_eq!(e.language("notes.md"), "python");
        assert!(!e.renders_markdown("notes.md"));
        e.language = Some("markdown".into());
        assert!(e.renders_markdown("main.rs"));
        e.markdown = false;
        assert!(!e.renders_markdown("main.rs"));
        assert_eq!(filter_languages("").len(), LANGUAGES.len());
        assert!(filter_languages("PYTH").iter().all(|l| l.contains("pyth")));
        assert!(filter_languages("pyth").contains(&"python"));
    }

    #[test]
    fn collapsed_view_keeps_six_lines() {
        let text = (1..=10).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        let (shown, notice) = visible(&text, false, 0);
        assert_eq!(shown.lines().count(), 6);
        assert_eq!(notice, Some((true, 4)));
        let (shown, notice) = visible(&text, true, 0);
        assert_eq!(shown.lines().count(), 10);
        assert_eq!(notice, None);
        assert_eq!(visible("a", false, 10).1, Some((false, 0)));
    }
}
