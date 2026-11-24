use crate::model::{Message, MessageKind};

pub const GROUP_WINDOW_SECS: i64 = 7 * 60;

const USER_MESSAGE: [u8; 9] = [0, 19, 20, 23, 26, 41, 45, 47, 68];
const COMMAND: [u8; 2] = [20, 23];
const FLAG_HAS_THREAD: u64 = 1 << 5;
const FLAG_SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;

fn is_user_message(m: &Message) -> bool {
    matches!(m.kind, MessageKind::Default | MessageKind::Reply) && USER_MESSAGE.contains(&m.type_id)
}

fn non_default(m: &Message) -> bool {
    m.type_id > 0 || !matches!(m.kind, MessageKind::Default) || m.reply_to.is_some()
}

fn ms(m: &Message) -> i64 {
    if m.id.0 >= 1 << 40 {
        ((m.id.0 >> 22) + 1_420_070_400_000) as i64
    } else {
        m.timestamp * 1000
    }
}

pub fn starts_group(prev: Option<&Message>, cur: &Message, utc_offset_secs: i64) -> bool {
    let Some(prev) = prev else { return true };
    if cur.thread.is_some() || cur.flags & FLAG_HAS_THREAD != 0 || COMMAND.contains(&cur.type_id) {
        return true;
    }
    let silent = |m: &Message| m.flags & FLAG_SUPPRESS_NOTIFICATIONS != 0;
    let ephemeral = |m: &Message| m.flags & crate::model::FLAG_EPHEMERAL != 0;
    let split = if non_default(cur) {
        is_user_message(prev) || matches!(cur.kind, MessageKind::Reply) || cur.type_id == 19 || cur.reply_to.is_some()
    } else {
        let (a, b) = (ms(prev), ms(cur));
        !is_user_message(prev)
            || prev.author != cur.author
            || ephemeral(prev) != ephemeral(cur)
            || cur.webhook_name.is_some() && prev.webhook_name != cur.webhook_name
            || prev.id == cur.channel && prev.id.0 >= 1 << 40
            || !((a - b).abs() <= 86_400_000 && day(prev.timestamp, utc_offset_secs) == day(cur.timestamp, utc_offset_secs))
            || (a - b).abs() >= GROUP_WINDOW_SECS * 1000
            || silent(cur) && !silent(prev)
            || silent(prev) && !silent(cur) && (cur.has_mentions || cur.mention_everyone)
    };
    split || cur.application_id != prev.application_id
}

pub fn day_divider(prev: Option<&Message>, cur: &Message, utc_offset_secs: i64) -> bool {
    prev.is_some_and(|p| day(p.timestamp, utc_offset_secs) != day(cur.timestamp, utc_offset_secs))
}

fn day(ts: i64, off: i64) -> i64 {
    (ts + off).div_euclid(86_400)
}

pub fn group_flags(msgs: &[Message], utc_offset_secs: i64) -> Vec<bool> {
    msgs.iter()
        .enumerate()
        .map(|(i, m)| starts_group(i.checked_sub(1).map(|p| &msgs[p]), m, utc_offset_secs))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Id;

    fn msg(id: u64, author: u64, ts: i64) -> Message {
        Message {
            id: Id(id),
            channel: Id(1_000),
            author: Id(author),
            timestamp: ts,
            edited: false,
            content: "x".into(),
            reply_to: None,
            embeds: vec![],
            attachments: vec![],
            reactions: vec![],
            ..Default::default()
        }
    }

    const T0: i64 = 1_763_800_000;

    #[test]
    fn first_message_starts_group() {
        assert!(starts_group(None, &msg(1, 1, T0), 0));
    }

    #[test]
    fn same_author_within_window_groups() {
        let a = msg(1, 1, T0);
        let b = msg(2, 1, T0 + 60);
        assert!(!starts_group(Some(&a), &b, 0));
        let c = msg(3, 1, T0 + GROUP_WINDOW_SECS - 1);
        assert!(!starts_group(Some(&a), &c, 0));
        let d = msg(4, 1, T0 + GROUP_WINDOW_SECS);
        assert!(starts_group(Some(&a), &d, 0));
    }

    #[test]
    fn system_message_breaks_the_group() {
        let poll = msg(1, 1, T0);
        let result = Message { kind: MessageKind::PollResult(Some(Id(1))), type_id: 46, ..msg(2, 1, T0 + 243) };
        let next = msg(3, 1, T0 + 599);
        assert!(starts_group(Some(&poll), &result, 0));
        assert!(starts_group(Some(&result), &next, 0));
        let local = Message { kind: MessageKind::Pin, ..msg(4, 1, T0 + 600) };
        assert!(starts_group(Some(&local), &msg(5, 1, T0 + 601), 0));
    }

    #[test]
    fn snowflake_times_and_other_splits() {
        let a = Message { id: Id(1574814389830422673), ..msg(0, 1, 0) };
        let b = Message { id: Id(1574815884067999868), ..msg(0, 1, 0) };
        assert_eq!(ms(&b) - ms(&a), 356_254);
        assert!(!starts_group(Some(&a), &b, 0));
        let base = msg(1, 1, T0);
        let reply = Message { kind: MessageKind::Reply, type_id: 19, reply_to: Some(Id(1)), ..msg(2, 1, T0 + 1) };
        assert!(starts_group(Some(&base), &reply, 0));
        let other = msg(2, 2, T0 + 1);
        assert!(starts_group(Some(&base), &other, 0));
        let hook = |name: &str, id| Message { webhook_name: Some(name.to_owned().into()), ..msg(id, 9, T0 + 1) };
        assert!(starts_group(Some(&hook("a", 1)), &hook("b", 2), 0));
        assert!(!starts_group(Some(&hook("a", 1)), &hook("a", 2), 0));
        let silent = Message { flags: FLAG_SUPPRESS_NOTIFICATIONS, ..msg(2, 1, T0 + 1) };
        assert!(starts_group(Some(&base), &silent, 0));
        assert!(!starts_group(Some(&silent), &msg(3, 1, T0 + 2), 0));
        let pinging = Message { has_mentions: true, ..msg(3, 1, T0 + 2) };
        assert!(starts_group(Some(&silent), &pinging, 0));
        let app = Message { application_id: Some(Id(7)), ..msg(2, 1, T0 + 1) };
        assert!(starts_group(Some(&base), &app, 0));
        let threaded = Message { thread: Some(crate::model::ThreadRef::default()), ..msg(2, 1, T0 + 1) };
        assert!(starts_group(Some(&base), &threaded, 0));
        let post = Id(1574814389830422673);
        let starter = Message { id: post, channel: post, ..msg(0, 1, 0) };
        let next = Message { id: Id(1574815884067999868), channel: post, ..msg(0, 1, 0) };
        assert!(starts_group(Some(&starter), &next, 0));
    }

    #[test]
    fn window_expires_after_seven_minutes() {
        let a = msg(1, 1, T0);
        let b = msg(2, 1, T0 + GROUP_WINDOW_SECS + 1);
        assert!(starts_group(Some(&a), &b, 0));
    }

    #[test]
    fn different_author_breaks_group() {
        assert!(starts_group(Some(&msg(1, 1, T0)), &msg(2, 2, T0 + 5), 0));
    }

    #[test]
    fn reply_always_starts_group() {
        let mut b = msg(2, 1, T0 + 5);
        b.reply_to = Some(Id(1));
        assert!(starts_group(Some(&msg(1, 1, T0)), &b, 0));
    }

    #[test]
    fn day_boundary_breaks_group() {
        let midnight = (T0 / 86_400 + 1) * 86_400;
        let a = msg(1, 1, midnight - 60);
        let b = msg(2, 1, midnight + 60);
        assert!(starts_group(Some(&a), &b, 0));
        assert!(day_divider(Some(&a), &b, 0));
        assert!(!starts_group(Some(&a), &b, 7200));
    }

    #[test]
    fn out_of_order_timestamps_group_within_window() {
        assert!(!starts_group(Some(&msg(1, 1, T0)), &msg(2, 1, T0 - 10), 0));
        assert!(starts_group(Some(&msg(1, 1, T0)), &msg(2, 1, T0 - GROUP_WINDOW_SECS), 0));
    }

    #[test]
    fn flags_for_slice() {
        let v = vec![
            msg(1, 1, T0),
            msg(2, 1, T0 + 10),
            msg(3, 2, T0 + 20),
            msg(4, 2, T0 + 30),
            msg(5, 2, T0 + 1000),
        ];
        assert_eq!(group_flags(&v, 0), vec![true, false, true, false, true]);
    }
}
