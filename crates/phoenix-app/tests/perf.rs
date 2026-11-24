use phoenix_app::{app::ChannelMessages, model::{Id, Message}};

fn msgs(ids: &[u64]) -> ChannelMessages {
    ChannelMessages {
        msgs: ids.iter().map(|&i| Message { id: Id(i), ..Default::default() }).collect(),
        ..Default::default()
    }
}

#[test]
fn index_of_sorted_history() {
    let e = msgs(&(1..=5000).map(|i| i * 7).collect::<Vec<_>>());
    assert_eq!(e.index_of(Id(7)), Some(0));
    assert_eq!(e.index_of(Id(35_000)), Some(4999));
    assert_eq!(e.index_of(Id(8)), None);
}

#[test]
fn index_of_out_of_order_tail() {
    let e = msgs(&[10, 20, 30, 5, 40]);
    assert_eq!(e.index_of(Id(5)), Some(3));
    assert_eq!(e.index_of(Id(40)), Some(4));
    assert_eq!(e.index_of(Id(20)), Some(1));
}
