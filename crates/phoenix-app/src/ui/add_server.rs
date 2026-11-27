use std::cell::RefCell;

use gpui::{
    AnyElement, Context, Entity, FontWeight, ParentElement, SharedString, Styled, Window, div, prelude::*, svg,
};
use gpui_component::input::{Input, InputEvent, InputState};

use crate::{
    app::PhoenixApp,
    theme::{Palette, c, ca, generated::idx, tok, z},
    ui::home::data::{NavReply, NavRequest, Overlay},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Templates,
    Intent,
    Customize,
    Join,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateSpec {
    pub id: &'static str,
    pub code: &'static str,
    pub label_key: &'static str,
    pub icon: &'static str,
}

pub const CREATE: TemplateSpec = TemplateSpec {
    id: "CREATE",
    code: "2TffvPucqHkN",
    label_key: "add_server.create_own",
    icon: "https://discord.com/assets/b30f13ee315c2568.svg",
};

pub const TEMPLATES: &[TemplateSpec] = &[
    TemplateSpec { id: "GAMING", code: "hvtBQMfw6uSJ", label_key: "add_server.tpl_gaming", icon: "https://discord.com/assets/261f952bf028fa34.svg" },
    TemplateSpec { id: "FRIENDS", code: "hgM48av5Q69A", label_key: "add_server.tpl_friends", icon: "https://discord.com/assets/d804200b134c9327.svg" },
    TemplateSpec { id: "STUDY", code: "FbwUwRp4j8Es", label_key: "add_server.tpl_study", icon: "https://discord.com/assets/4900b53e7b34c3a5.svg" },
    TemplateSpec { id: "SCHOOL_CLUB", code: "Ctg7PUHcQmZu", label_key: "add_server.tpl_school", icon: "https://discord.com/assets/2f1587b0c86b42e2.svg" },
    TemplateSpec { id: "LOCAL_COMMUNITY", code: "64UDvRNCC52Y", label_key: "add_server.tpl_local", icon: "https://discord.com/assets/31f3db39524533b6.svg" },
    TemplateSpec { id: "CREATORS", code: "6exdzMgjZgah", label_key: "add_server.tpl_creators", icon: "https://discord.com/assets/d8fed3f03866afe2.svg" },
];

pub const ROW_ARROW: &str = "https://discord.com/assets/050c2ac76232eff6.svg";

#[derive(Default)]
struct State {
    step: Step,
    template: Option<&'static str>,
    community: Option<bool>,
    name_input: Option<Entity<InputState>>,
    create_error: Option<SharedString>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

pub fn set_step(step: Step) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.step = step;
        st.create_error = None;
    });
}

pub fn current_step() -> Step {
    STATE.with(|s| s.borrow().step)
}

pub fn default_server_name(username: &str, is_fr: bool) -> String {
    let u = username.trim();
    if u.is_empty() {
        if is_fr { "Mon serveur".to_owned() } else { "My Server".to_owned() }
    } else if is_fr {
        format!("Serveur de {u}")
    } else {
        format!("{u}'s server")
    }
}

const TEMPLATE_EVERYONE_PERMS: &str = "1720707884502593";
const TEMPLATE_OFFICER_PERMS: &str = "1720709360909911";

fn tpl_ch(id: &str, parent_id: Option<&str>, name: &str, kind: u8) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "parent_id": parent_id,
        "name": name,
        "type": kind,
    })
}

fn template_spec_payload(code: &str, is_fr: bool) -> (Vec<serde_json::Value>, Option<&'static str>, Option<serde_json::Value>) {
    let text_cat = if is_fr { "Salons textuels" } else { "Text Channels" };
    let voice_cat = if is_fr { "Salons vocaux" } else { "Voice Channels" };
    let info_cat = if is_fr { "Informations" } else { "Information" };
    let general = if is_fr { "général" } else { "general" };
    let welcome = if is_fr { "bienvenue-et-règles" } else { "welcome-and-rules" };
    let announcements = if is_fr { "annonces" } else { "announcements" };
    let off_topic = if is_fr { "hors-sujet" } else { "off-topic" };
    let meeting_plans = if is_fr { "réunions-planifiées" } else { "meeting-plans" };
    let stream_room = if is_fr { "Salle de stream" } else { "Stream Room" };
    let meeting_room = if is_fr { "Salle de réunion" } else { "Meeting Room" };
    let resources = if is_fr { "ressources" } else { "resources" };

    match code {
        "hvtBQMfw6uSJ" => (
            vec![
                tpl_ch("00", None, text_cat, 4),
                tpl_ch("01", Some("00"), general, 0),
                tpl_ch("02", Some("00"), if is_fr { "clips-vidéos-et-temps-forts" } else { "clips-and-highlights" }, 0),
                tpl_ch("10", None, voice_cat, 4),
                tpl_ch("11", Some("10"), "Lobby", 2),
                tpl_ch("12", Some("10"), "Gaming", 2),
            ],
            Some("01"),
            None,
        ),
        "hgM48av5Q69A" => (
            vec![
                tpl_ch("00", None, text_cat, 4),
                tpl_ch("01", Some("00"), general, 0),
                tpl_ch("02", Some("00"), if is_fr { "jeux" } else { "games" }, 0),
                tpl_ch("03", Some("00"), if is_fr { "musique" } else { "music" }, 0),
                tpl_ch("10", None, voice_cat, 4),
                tpl_ch("11", Some("10"), "Lounge", 2),
                tpl_ch("12", Some("10"), stream_room, 2),
            ],
            Some("01"),
            None,
        ),
        "FbwUwRp4j8Es" => (
            vec![
                tpl_ch("00", None, info_cat, 4),
                tpl_ch("01", Some("00"), welcome, 0),
                tpl_ch("02", Some("00"), if is_fr { "notes-et-ressources" } else { "notes-resources" }, 0),
                tpl_ch("10", None, text_cat, 4),
                tpl_ch("11", Some("10"), general, 0),
                tpl_ch("12", Some("10"), if is_fr { "aide-aux-devoirs" } else { "homework-help" }, 0),
                tpl_ch("13", Some("10"), if is_fr { "planification-de-sessions" } else { "session-planning" }, 0),
                tpl_ch("14", Some("10"), off_topic, 0),
                tpl_ch("20", None, voice_cat, 4),
                tpl_ch("21", Some("20"), "Lounge", 2),
                tpl_ch("22", Some("20"), if is_fr { "Salle d'étude 1" } else { "Study Room 1" }, 2),
                tpl_ch("23", Some("20"), if is_fr { "Salle d'étude 2" } else { "Study Room 2" }, 2),
            ],
            Some("11"),
            None,
        ),
        "Ctg7PUHcQmZu" => (
            vec![
                tpl_ch("00", None, info_cat, 4),
                tpl_ch("01", Some("00"), welcome, 0),
                tpl_ch("02", Some("00"), announcements, 0),
                tpl_ch("03", Some("00"), resources, 0),
                tpl_ch("10", None, text_cat, 4),
                tpl_ch("11", Some("10"), general, 0),
                tpl_ch("12", Some("10"), meeting_plans, 0),
                tpl_ch("13", Some("10"), off_topic, 0),
                tpl_ch("20", None, voice_cat, 4),
                tpl_ch("21", Some("20"), "Lounge", 2),
                tpl_ch("22", Some("20"), if is_fr { "Salle de réunion\u{a0}1" } else { "Meeting Room 1" }, 2),
                tpl_ch("22", Some("20"), if is_fr { "Salle de réunion\u{a0}2" } else { "Meeting Room 2" }, 2),
            ],
            Some("11"),
            Some(serde_json::json!([
                {
                    "id": "00",
                    "name": "@everyone",
                    "permissions": TEMPLATE_EVERYONE_PERMS,
                },
                {
                    "id": "01",
                    "name": "officers (example)",
                    "mentionable": true,
                    "hoist": true,
                    "permissions": TEMPLATE_OFFICER_PERMS,
                    "color": 3066993,
                },
            ])),
        ),
        "64UDvRNCC52Y" => (
            vec![
                tpl_ch("00", None, info_cat, 4),
                tpl_ch("01", Some("00"), welcome, 0),
                tpl_ch("02", Some("00"), announcements, 0),
                tpl_ch("03", Some("00"), resources, 0),
                tpl_ch("10", None, text_cat, 4),
                tpl_ch("11", Some("10"), general, 0),
                tpl_ch("12", Some("10"), meeting_plans, 0),
                tpl_ch("13", Some("10"), off_topic, 0),
                tpl_ch("20", None, voice_cat, 4),
                tpl_ch("21", Some("20"), "Lounge", 2),
                tpl_ch("22", Some("20"), meeting_room, 2),
            ],
            Some("11"),
            None,
        ),
        "6exdzMgjZgah" => (
            vec![
                tpl_ch("00", None, info_cat, 4),
                tpl_ch("01", Some("00"), welcome, 0),
                tpl_ch("02", Some("00"), announcements, 0),
                tpl_ch("10", None, text_cat, 4),
                tpl_ch("11", Some("10"), general, 0),
                tpl_ch("12", Some("10"), if is_fr { "événements" } else { "events" }, 0),
                tpl_ch("13", Some("10"), if is_fr { "idées-et-commentaires" } else { "ideas-and-feedback" }, 0),
                tpl_ch("20", None, voice_cat, 4),
                tpl_ch("21", Some("20"), "Lounge", 2),
                tpl_ch("22", Some("20"), if is_fr { "Repaire de la communauté" } else { "Community Hangout" }, 2),
                tpl_ch("23", Some("20"), stream_room, 2),
            ],
            Some("11"),
            None,
        ),
        "s4WNnBxTDPsY" => (
            vec![
                tpl_ch("00", None, info_cat, 4),
                tpl_ch("01", Some("00"), welcome, 0),
                tpl_ch("02", Some("00"), announcements, 0),
                tpl_ch("10", None, text_cat, 4),
                tpl_ch("11", Some("10"), general, 0),
                tpl_ch("12", Some("10"), meeting_plans, 0),
                tpl_ch("20", None, voice_cat, 4),
                tpl_ch("21", Some("20"), "Lounge", 2),
                tpl_ch("22", Some("20"), meeting_room, 2),
            ],
            Some("11"),
            None,
        ),
        _ => (Vec::new(), None, None),
    }
}

pub fn create_guild_body_for_locale(
    name: &str,
    icon: Option<&str>,
    template: Option<&str>,
    _community: Option<bool>,
    is_fr: bool,
) -> serde_json::Value {
    let code = template.unwrap_or(CREATE.code);
    let (channels, system_channel_id, roles) = template_spec_payload(code, is_fr);
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), serde_json::json!(name.trim()));
    obj.insert("icon".into(), serde_json::json!(icon));
    obj.insert("channels".into(), serde_json::Value::Array(channels));
    obj.insert("system_channel_id".into(), serde_json::json!(system_channel_id));
    if let Some(r) = roles {
        obj.insert("roles".into(), r);
    }
    obj.insert("guild_template_code".into(), serde_json::json!(code));
    serde_json::Value::Object(obj)
}

pub fn plain_create_guild_body(name: &str, icon: Option<&str>) -> serde_json::Value {
    let mut body = serde_json::json!({ "name": name.trim() });
    if let Some(icon) = icon {
        body["icon"] = serde_json::json!(icon);
    }
    body
}

pub fn create_guild_body(
    name: &str,
    icon: Option<&str>,
    template: Option<&str>,
    community: Option<bool>,
) -> serde_json::Value {
    create_guild_body_for_locale(name, icon, template, community, crate::i18n::locale() == "fr")
}

fn ensure_name_input(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> Entity<InputState> {
    if let Some(existing) = STATE.with(|s| s.borrow().name_input.clone()) {
        return existing;
    }
    let uname = app.me.name.as_ref();
    let is_fr = crate::i18n::locale() == "fr";
    let def = default_server_name(uname, is_fr);
    let input = cx.new(|cx| {
        let mut st = InputState::new(window, cx);
        st.set_value(def, window, cx);
        st
    });
    STATE.with(|s| s.borrow_mut().name_input = Some(input.clone()));
    input
}

impl PhoenixApp {
    pub fn open_add_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        reset();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("https://discord.gg/hTKzmak"));
        let sub = cx.subscribe_in(&input, window, |this: &mut PhoenixApp, _, ev: &InputEvent, _w, cx| match ev {
            InputEvent::Change => this.preview_invite(cx),
            InputEvent::PressEnter { .. } => this.join_invite(cx),
            _ => {}
        });
        self.show_overlay(Overlay::JoinServer, cx);
        self.nav.subs = vec![sub];
        self.nav.invite_input = Some(input);
    }

    pub fn submit_create_guild(&mut self, cx: &mut Context<Self>) {
        let (name, template, community) = STATE.with(|s| {
            let st = s.borrow();
            let name = st.name_input.as_ref().map(|i| i.read(cx).value().to_string()).unwrap_or_default();
            (name, st.template.map(str::to_owned), st.community)
        });
        if name.trim().is_empty() || self.nav.busy {
            return;
        }
        self.nav.busy = true;
        STATE.with(|s| s.borrow_mut().create_error = None);
        self.nav_request(
            NavRequest::CreateGuild { name: name.trim().to_owned(), icon: None, template, community },
            cx,
            |this, res, cx| {
                this.nav.busy = false;
                match res {
                    Ok(reply) => {
                        if let NavReply::Json(guild) = reply
                            && let Some(id) = guild.get("id").and_then(serde_json::Value::as_str).and_then(|id| id.parse().ok())
                        {
                            crate::ui::chat::welcome::record_creation(this, crate::model::Id(id));
                        }
                        reset();
                        this.close_overlay(cx);
                    }
                    Err(e) => {
                        STATE.with(|s| s.borrow_mut().create_error = Some(e.0.into()));
                    }
                }
            },
        );
        cx.notify();
    }
}

fn header(title: SharedString, subtitle: SharedString, pad_x: f32) -> gpui::Div {
    div()
        .pt(z(24.))
        .px(z(pad_x))
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .px(z(20.))
                .text_size(z(24.))
                .line_height(z(32.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(tok(idx::TEXT_STRONG)))
                .text_center()
                .child(title),
        )
        .child(
            div()
                .mt(z(8.))
                .text_size(z(16.))
                .line_height(z(20.))
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .text_center()
                .child(subtitle),
        )
}

fn option_row(id: &'static str, icon_name: &str, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .h(z(64.))
        .mb(z(8.))
        .px(z(16.))
        .rounded(z(8.))
        .bg(c(Palette::BG_SECONDARY))
        .border_1()
        .border_color(ca(0xFFFFFF14))
        .flex()
        .items_center()
        .gap(z(12.))
        .cursor_pointer()
        .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
        .child(
            div()
                .size(z(36.))
                .rounded_full()
                .bg(ca(0x5865F226))
                .flex()
                .items_center()
                .justify_center()
                .child(svg().path(crate::icons::path(icon_name)).size(z(20.)).text_color(c(Palette::TEXT_HEADER))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(z(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(Palette::TEXT_HEADER))
                .text_ellipsis()
                .child(label),
        )
        .child(svg().path(crate::icons::path("chevron-right")).size(z(18.)).text_color(c(Palette::INTERACTIVE)))
}

fn template_row(id: &'static str, t: TemplateSpec) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .mb(z(8.))
        .rounded(z(8.))
        .bg(c(tok(idx::BACKGROUND_MOD_MUTED)))
        .border_1()
        .border_color(c(tok(idx::BORDER_SUBTLE)))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_HOVER))))
        .active(|s| s.bg(c(tok(idx::INTERACTIVE_BACKGROUND_ACTIVE))))
        .child(div().m(z(16.)).size(z(20.)).flex_none().child(gpui::img(SharedString::from(t.icon)).size(z(20.))))
        .child(
            div()
                .min_w_0()
                .text_size(z(16.))
                .line_height(z(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(tok(idx::TEXT_DEFAULT)))
                .text_ellipsis()
                .child(crate::t!(t.label_key)),
        )
        .child(div().ml_auto().mr(z(16.)).size(z(20.)).flex_none().child(gpui::img(SharedString::from(ROW_ARROW)).size(z(20.))))
}

fn back_btn(id: &'static str, to: Step, cx: &mut Context<PhoenixApp>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(38.))
        .px(z(8.))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(Palette::TEXT_NORMAL))
        .hover(|s| s.text_color(c(Palette::TEXT_HEADER)))
        .on_click(cx.listener(move |_, _, _, cx| {
            set_step(to);
            cx.notify();
        }))
        .child(crate::t!("common.back"))
}

fn primary_btn(id: &'static str, label: SharedString, disabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(z(38.))
        .px(z(20.))
        .rounded(z(4.))
        .bg(c(Palette::BLURPLE))
        .text_color(gpui::white())
        .text_size(z(14.))
        .font_weight(FontWeight::MEDIUM)
        .flex()
        .items_center()
        .justify_center()
        .when(!disabled, |d| d.cursor_pointer().hover(|s| s.opacity(0.88)))
        .when(disabled, |d| d.opacity(0.5))
        .child(label)
}

fn upload_badge() -> gpui::Div {
    let mut box_el = div()
        .mt(z(20.))
        .flex()
        .justify_center();
    if let Some(art) = crate::icons::art::guild_upload() {
        box_el = box_el.child(
            div()
                .relative()
                .size(z(80.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(art.ring.to_string_lossy().to_string())
                        .size(z(80.))
                        .text_color(c(Palette::INTERACTIVE)),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .size(z(24.))
                        .rounded_full()
                        .bg(c(Palette::BLURPLE))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(svg().path(art.plus.to_string_lossy().to_string()).size(z(24.)).text_color(gpui::white())),
                ),
        );
    } else {
        box_el = box_el.child(
            div()
                .size(z(80.))
                .rounded_full()
                .border_2()
                .border_color(c(Palette::INTERACTIVE))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_size(z(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(Palette::TEXT_MUTED))
                .child(crate::t!("add_server.upload")),
        );
    }
    box_el
}

pub fn render_step(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    match current_step() {
        Step::Templates => {
            let mut list = div()
                .id("add-server-templates")
                .mt(z(24.))
                .pt(z(8.))
                .pb(z(8.))
                .pl(z(16.))
                .pr(z(8.))
                .h(z(330.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(template_row("tpl-own", CREATE).on_click(cx.listener(
                    |_, _, _, cx| {
                        STATE.with(|s| {
                            let mut st = s.borrow_mut();
                            st.template = None;
                            st.step = Step::Intent;
                        });
                        cx.notify();
                    },
                )))
                .child(
                    div()
                        .mt(z(16.))
                        .mb(z(8.))
                        .text_size(z(12.))
                        .line_height(z(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(tok(idx::TEXT_DEFAULT)))
                        .child(crate::t!("add_server.start_template")),
                );
            for t in TEMPLATES {
                let tid = t.code;
                list = list.child(template_row(t.id, *t).on_click(cx.listener(
                    move |_, _, _, cx| {
                        STATE.with(|s| {
                            let mut st = s.borrow_mut();
                            st.template = Some(tid);
                            st.step = Step::Intent;
                        });
                        cx.notify();
                    },
                )));
            }
            div()
                .flex()
                .flex_col()
                .child(header(crate::t!("add_server.title"), crate::t!("add_server.subtitle"), 16.))
                .child(list)
                .child(
                    div()
                        .p(z(16.))
                        .bg(c(Palette::BG_SECONDARY))
                        .flex()
                        .flex_col()
                        .items_center()
                        .child(
                            div()
                                .mb(z(8.))
                                .text_size(z(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(c(Palette::TEXT_HEADER))
                                .child(crate::t!("add_server.have_invite")),
                        )
                        .child(
                            div()
                                .id("add-server-go-join")
                                .w_full()
                                .h(z(38.))
                                .rounded(z(4.))
                                .bg(c(Palette::BG_MOD_HOVER))
                                .text_color(c(Palette::TEXT_HEADER))
                                .text_size(z(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.88))
                                .on_click(cx.listener(|a, _, window, cx| {
                                    set_step(Step::Join);
                                    if let Some(inp) = &a.nav.invite_input {
                                        inp.update(cx, |s, cx| s.focus(window, cx));
                                    }
                                    cx.notify();
                                }))
                                .child(crate::t!("invite.join_title")),
                        ),
                )
        }
        Step::Intent => div()
            .flex()
            .flex_col()
            .child(header(crate::t!("add_server.intent_title"), crate::t!("add_server.intent_sub"), 16.))
            .child(
                div()
                    .mt(z(24.))
                    .px(z(16.))
                    .flex()
                    .flex_col()
                    .child(
                        option_row("intent-friends", "friends", crate::t!("add_server.intent_friends")).on_click(
                            cx.listener(|_, _, _, cx| {
                                STATE.with(|s| {
                                    let mut st = s.borrow_mut();
                                    st.community = Some(false);
                                    st.step = Step::Customize;
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        option_row("intent-comm", "globe", crate::t!("add_server.intent_community")).on_click(
                            cx.listener(|_, _, _, cx| {
                                STATE.with(|s| {
                                    let mut st = s.borrow_mut();
                                    st.community = Some(true);
                                    st.step = Step::Customize;
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .id("intent-skip")
                            .mt(z(14.))
                            .mb(z(16.))
                            .text_size(z(13.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .text_center()
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(Palette::TEXT_LINK)))
                            .on_click(cx.listener(|_, _, _, cx| {
                                STATE.with(|s| {
                                    let mut st = s.borrow_mut();
                                    st.community = None;
                                    st.step = Step::Customize;
                                });
                                cx.notify();
                            }))
                            .child(crate::t!("add_server.intent_skip")),
                    ),
            )
            .child(
                div()
                    .p(z(16.))
                    .bg(c(Palette::BG_SECONDARY))
                    .flex()
                    .items_center()
                    .child(back_btn("intent-back", Step::Templates, cx)),
            ),
        Step::Customize => {
            let name_input = ensure_name_input(app, window, cx);
            let err = STATE.with(|s| s.borrow().create_error.clone());
            let empty = name_input.read(cx).value().trim().is_empty();
            let busy = app.nav.busy;
            div()
                .flex()
                .flex_col()
                .child(header(crate::t!("add_server.custom_title"), crate::t!("add_server.custom_sub"), 24.))
                .child(upload_badge())
                .child(
                    div()
                        .mt(z(24.))
                        .px(z(16.))
                        .pb(z(16.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .mb(z(8.))
                                .text_size(z(12.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(c(Palette::TEXT_MUTED))
                                .child(crate::t!("add_server.name_label")),
                        )
                        .child(
                            div()
                                .h(z(40.))
                                .px(z(10.))
                                .rounded(z(8.))
                                .bg(c(Palette::BG_TERTIARY))
                                .border_1()
                                .border_color(ca(0xFFFFFF1A))
                                .flex()
                                .items_center()
                                .text_size(z(15.))
                                .child(Input::new(&name_input).appearance(false).bordered(false)),
                        )
                        .when_some(err, |d, e| {
                            d.child(div().mt(z(6.)).text_size(z(12.)).text_color(c(Palette::RED)).child(e))
                        })
                        .child(
                            div()
                                .mt(z(8.))
                                .text_size(z(12.))
                                .text_color(c(Palette::TEXT_MUTED))
                                .child(crate::t!("add_server.guidelines")),
                        ),
                )
                .child(
                    div()
                        .p(z(16.))
                        .bg(c(Palette::BG_SECONDARY))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(back_btn("custom-back", Step::Intent, cx))
                        .child(
                            primary_btn("custom-create", crate::t!("common.create"), empty || busy)
                                .on_click(cx.listener(|a, _, _, cx| a.submit_create_guild(cx))),
                        ),
                )
        }
        Step::Join => render_join_step(app, cx),
    }
}

fn sample_chip(text: &'static str) -> gpui::Div {
    div()
        .px(z(6.))
        .py(z(4.))
        .rounded(z(4.))
        .bg(c(Palette::BG_SECONDARY))
        .border_1()
        .border_color(ca(0xFFFFFF14))
        .text_size(z(13.))
        .text_color(c(Palette::TEXT_NORMAL))
        .child(text)
}

fn render_join_step(app: &PhoenixApp, cx: &mut Context<PhoenixApp>) -> gpui::Div {
    let preview = app.nav.invite_preview.clone();
    let busy = app.nav.busy;
    let is_err = matches!(preview, Some(Err(_)));
    let mut body = div()
        .px(z(16.))
        .pb(z(16.))
        .flex()
        .flex_col()
        .child(header(crate::t!("invite.join_title"), crate::t!("invite.join_sub"), 0.))
        .child(
            div()
                .mt(z(16.))
                .mb(z(8.))
                .text_size(z(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(if is_err { Palette::RED } else { Palette::TEXT_MUTED }))
                .child(crate::t!("invite.link")),
        )
        .child(
            div()
                .h(z(40.))
                .px(z(10.))
                .rounded(z(8.))
                .bg(c(Palette::BG_TERTIARY))
                .border_1()
                .border_color(if is_err { c(Palette::RED) } else { ca(0xFFFFFF1A) })
                .flex()
                .items_center()
                .text_size(z(15.))
                .when_some(app.nav.invite_input.clone(), |d, i| {
                    d.child(Input::new(&i).appearance(false).bordered(false))
                }),
        );
    body = match preview {
        Some(Ok(p)) => body.child(crate::ui::invite::preview_card(&p)),
        Some(Err(e)) => body.child(div().mt(z(8.)).text_size(z(12.)).text_color(c(Palette::RED)).child(e)),
        None => body
            .child(
                div()
                    .mt(z(16.))
                    .text_size(z(12.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(Palette::TEXT_MUTED))
                    .child(crate::t!("add_server.examples_title")),
            )
            .child(
                div()
                    .mt(z(8.))
                    .flex()
                    .flex_wrap()
                    .gap(z(6.))
                    .child(sample_chip("hTKzmak"))
                    .child(sample_chip("https://discord.gg/hTKzmak"))
                    .child(sample_chip("https://discord.gg/wumpus-friends")),
            ),
    };
    body = body.child(
        div()
            .id("join-open-discover")
            .mt(z(16.))
            .p(z(12.))
            .rounded(z(8.))
            .bg(c(Palette::BG_SECONDARY))
            .flex()
            .items_center()
            .gap(z(12.))
            .cursor_pointer()
            .hover(|s| s.bg(c(Palette::BG_MOD_HOVER)))
            .on_click(cx.listener(|a, _, _, cx| {
                reset();
                a.close_overlay(cx);
                crate::ui::discovery::open(crate::ui::discovery::Tab::Servers);
                cx.notify();
            }))
            .child(
                div()
                    .size(z(40.))
                    .rounded_full()
                    .bg(c(Palette::GREEN))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(svg().path(crate::icons::path("compass")).size(z(22.)).text_color(gpui::white())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(z(2.))
                    .child(
                        div()
                            .text_size(z(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(Palette::TEXT_HEADER))
                            .child(crate::t!("add_server.no_invite_title")),
                    )
                    .child(
                        div()
                            .text_size(z(12.))
                            .text_color(c(Palette::TEXT_MUTED))
                            .child(crate::t!("add_server.no_invite_sub")),
                    ),
            )
            .child(svg().path(crate::icons::path("chevron-right")).size(z(18.)).text_color(c(Palette::INTERACTIVE))),
    );
    div().flex().flex_col().child(body).child(
        div()
            .p(z(16.))
            .bg(c(Palette::BG_SECONDARY))
            .flex()
            .items_center()
            .justify_between()
            .child(back_btn("join-back", Step::Templates, cx))
            .child(
                primary_btn(
                    "join-go",
                    if busy { crate::t!("invite.joining") } else { crate::t!("invite.join") },
                    busy,
                )
                .on_click(cx.listener(|a, _, _, cx| a.join_invite(cx))),
            ),
    )
}

pub fn render_modal(app: &PhoenixApp, window: &mut Window, cx: &mut Context<PhoenixApp>) -> AnyElement {
    let inner = render_step(app, window, cx);
    crate::ui::invite::modal("add-server-modal", cx, inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_names_match_locale() {
        assert_eq!(default_server_name("Roxas", true), "Serveur de Roxas");
        assert_eq!(default_server_name("Roxas", false), "Roxas's server");
        assert_eq!(default_server_name("  ", true), "Mon serveur");
        assert_eq!(default_server_name("", false), "My Server");
    }

    #[test]
    fn create_guild_payload_matches_client_templates_in_fr_and_en() {
        let gaming_fr = create_guild_body_for_locale("  Phoenix Lab ", None, Some("hvtBQMfw6uSJ"), Some(true), true);
        assert_eq!(
            gaming_fr,
            serde_json::json!({
                "name": "Phoenix Lab",
                "icon": null,
                "channels": [
                    { "id": "00", "parent_id": null, "name": "Salons textuels", "type": 4 },
                    { "id": "01", "parent_id": "00", "name": "général", "type": 0 },
                    { "id": "02", "parent_id": "00", "name": "clips-vidéos-et-temps-forts", "type": 0 },
                    { "id": "10", "parent_id": null, "name": "Salons vocaux", "type": 4 },
                    { "id": "11", "parent_id": "10", "name": "Lobby", "type": 2 },
                    { "id": "12", "parent_id": "10", "name": "Gaming", "type": 2 },
                ],
                "system_channel_id": "01",
                "guild_template_code": "hvtBQMfw6uSJ",
            })
        );

        let gaming_en = create_guild_body_for_locale("Phoenix Lab", None, Some("hvtBQMfw6uSJ"), None, false);
        assert_eq!(
            gaming_en,
            serde_json::json!({
                "name": "Phoenix Lab",
                "icon": null,
                "channels": [
                    { "id": "00", "parent_id": null, "name": "Text Channels", "type": 4 },
                    { "id": "01", "parent_id": "00", "name": "general", "type": 0 },
                    { "id": "02", "parent_id": "00", "name": "clips-and-highlights", "type": 0 },
                    { "id": "10", "parent_id": null, "name": "Voice Channels", "type": 4 },
                    { "id": "11", "parent_id": "10", "name": "Lobby", "type": 2 },
                    { "id": "12", "parent_id": "10", "name": "Gaming", "type": 2 },
                ],
                "system_channel_id": "01",
                "guild_template_code": "hvtBQMfw6uSJ",
            })
        );

        let school_fr = create_guild_body_for_locale("Club", None, Some("Ctg7PUHcQmZu"), Some(true), true);
        assert_eq!(
            school_fr,
            serde_json::json!({
                "name": "Club",
                "icon": null,
                "channels": [
                    { "id": "00", "parent_id": null, "name": "Informations", "type": 4 },
                    { "id": "01", "parent_id": "00", "name": "bienvenue-et-règles", "type": 0 },
                    { "id": "02", "parent_id": "00", "name": "annonces", "type": 0 },
                    { "id": "03", "parent_id": "00", "name": "ressources", "type": 0 },
                    { "id": "10", "parent_id": null, "name": "Salons textuels", "type": 4 },
                    { "id": "11", "parent_id": "10", "name": "général", "type": 0 },
                    { "id": "12", "parent_id": "10", "name": "réunions-planifiées", "type": 0 },
                    { "id": "13", "parent_id": "10", "name": "hors-sujet", "type": 0 },
                    { "id": "20", "parent_id": null, "name": "Salons vocaux", "type": 4 },
                    { "id": "21", "parent_id": "20", "name": "Lounge", "type": 2 },
                    { "id": "22", "parent_id": "20", "name": "Salle de réunion\u{a0}1", "type": 2 },
                    { "id": "22", "parent_id": "20", "name": "Salle de réunion\u{a0}2", "type": 2 },
                ],
                "system_channel_id": "11",
                "roles": [
                    { "id": "00", "name": "@everyone", "permissions": "1720707884502593" },
                    {
                        "id": "01",
                        "name": "officers (example)",
                        "mentionable": true,
                        "hoist": true,
                        "permissions": "1720709360909911",
                        "color": 3066993,
                    },
                ],
                "guild_template_code": "Ctg7PUHcQmZu",
            })
        );

        let school_en = create_guild_body_for_locale("Club", None, Some("Ctg7PUHcQmZu"), None, false);
        assert_eq!(school_en["channels"][0]["name"], "Information");
        assert_eq!(school_en["channels"][1]["name"], "welcome-and-rules");
        assert_eq!(school_en["channels"][10]["name"], "Meeting Room 1");
        assert_eq!(school_en["channels"][11]["name"], "Meeting Room 2");
        assert_eq!(school_en["roles"][1]["name"], "officers (example)");
        assert_eq!(school_en["system_channel_id"], "11");

        let minimal = create_guild_body_for_locale("Friends", None, None, None, false);
        assert_eq!(
            minimal,
            serde_json::json!({
                "name": "Friends",
                "icon": null,
                "channels": [],
                "system_channel_id": null,
                "guild_template_code": "2TffvPucqHkN",
            })
        );
    }

    #[test]
    fn create_guild_is_a_normal_action_outside_the_write_guard() {
        let src = include_str!("../discord_backend/nav.rs");
        let arm = &src[src.find("NavRequest::CreateGuild {").unwrap()..src.find("NavRequest::FetchOnboarding").unwrap()];
        assert!(!arm.contains("guarded_write"));
        assert!(arm.contains("plain_create_guild_body"));
    }

    #[test]
    fn plain_create_guild_body_only_has_the_name() {
        assert_eq!(plain_create_guild_body(" Friends ", None), serde_json::json!({ "name": "Friends" }));
    }

    #[test]
    fn step_transitions_reset_cleanly() {
        reset();
        assert_eq!(current_step(), Step::Templates);
        set_step(Step::Intent);
        assert_eq!(current_step(), Step::Intent);
        set_step(Step::Join);
        assert_eq!(current_step(), Step::Join);
        reset();
        assert_eq!(current_step(), Step::Templates);
    }
}
