//! The mesh rules as the room applies them: target, links, subscriptions, uplinks.

use super::{
    ChannelId, Effects, LinkClose, MediaKind, MediaPath, MeshPolicy, Pair, Room, Route,
    RouteNotice, SessionId, Track, TrackId,
};

fn mesh_room(limit: usize) -> Room {
    Room::with_mesh(ChannelId::parse("general").unwrap(), MeshPolicy::new(limit))
}

fn video() -> Track {
    Track::new(TrackId::generate(), MediaKind::Video)
}

/// Joins a participant that has finished connecting.
fn join_ready(room: &mut Room) -> (SessionId, Effects) {
    let id = SessionId::generate();
    let joined = room.join(id.clone(), "someone").unwrap();
    room.client_offer_answered(&id).unwrap();
    room.media_connected(&id).unwrap();
    (id, joined.effects)
}

/// Both sides of every pair report the link's current epoch.
fn report_all(room: &mut Room, ids: &[&SessionId]) -> Vec<Effects> {
    let mut all = Vec::new();
    for (index, a) in ids.iter().enumerate() {
        for b in &ids[index + 1..] {
            let epoch = room.epoch(a, b).unwrap();
            all.push(room.route_live(a, b, epoch));
            all.push(room.route_live(b, a, epoch));
        }
    }
    all
}

fn route_of(room: &Room, a: &SessionId, b: &SessionId) -> Route {
    room.route(a, b).unwrap()
}

fn merged(effects: Vec<Effects>) -> Effects {
    let mut total = Effects::default();
    for e in effects {
        total.subscribe.extend(e.subscribe);
        total.unsubscribe.extend(e.unsubscribe);
        total.routes.extend(e.routes);
        total.link_closes.extend(e.link_closes);
        total.uplinks.extend(e.uplinks);
        total.timeouts.extend(e.timeouts);
    }
    total
}

fn route_notice(to: &SessionId, peer: &SessionId, path: MediaPath, epoch: u64) -> RouteNotice {
    RouteNotice { to: to.clone(), peer: peer.clone(), path, epoch }
}

#[test]
fn the_second_participant_starts_the_move_to_direct() {
    let mut room = mesh_room(4);
    let (a, first) = join_ready(&mut room);
    assert!(first.is_empty(), "alone: nothing to do, uplink stays active");

    let (b, effects) = join_ready(&mut room);

    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);
    assert_eq!(room.epoch(&a, &b), Some(1));
    assert_eq!(room.target(), MediaPath::Direct);
    let mut routes = effects.routes.clone();
    routes.sort_by(|x, y| x.to.cmp(&y.to));
    let mut expected = vec![
        route_notice(&a, &b, MediaPath::Direct, 1),
        route_notice(&b, &a, MediaPath::Direct, 1),
    ];
    expected.sort_by(|x, y| x.to.cmp(&y.to));
    assert_eq!(routes, expected);
    assert_eq!(effects.timeouts.len(), 1);
    assert_eq!(effects.timeouts[0].pair, Pair::new(&a, &b));
    assert_eq!(effects.timeouts[0].epoch, 1);
    assert!(effects.uplinks.is_empty(), "both paths carry media, so uplinks stay active");
}

#[test]
fn reaching_direct_unsubscribes_both_sides_and_pauses_both_uplinks() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (ta, tb) = (video(), video());
    assert_eq!(room.publish(&a, ta.clone()).unwrap(), vec![b.clone()]);
    assert_eq!(room.publish(&b, tb.clone()).unwrap(), vec![a.clone()]);

    let effects = merged(report_all(&mut room, &[&a, &b]));

    assert_eq!(route_of(&room, &a, &b), Route::Direct);
    let mut unsubscribe = effects.unsubscribe.clone();
    unsubscribe.sort();
    let mut expected = vec![(a.clone(), vec![tb.id.clone()]), (b.clone(), vec![ta.id.clone()])];
    expected.sort();
    assert_eq!(unsubscribe, expected);
    assert!(room.subscriptions(&a).unwrap().is_empty());
    assert!(room.subscriptions(&b).unwrap().is_empty());
    let mut uplinks = effects.uplinks.clone();
    uplinks.sort();
    let mut expected = vec![(a, false), (b, false)];
    expected.sort();
    assert_eq!(uplinks, expected);
}

#[test]
fn a_direct_link_gets_no_sfu_tracks_and_a_forwarded_one_does() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    report_all(&mut room, &[&a, &b]);
    let (c, _) = join_ready(&mut room); // a third participant forwards the room

    let track = video();
    let mut receivers = room.publish(&a, track.clone()).unwrap();
    receivers.sort();
    let mut expected = vec![b.clone(), c.clone()];
    expected.sort();
    assert_eq!(receivers, expected, "a-b is moving back, so b receives it again");

    let mut mesh = mesh_room(4);
    let (a, _) = join_ready(&mut mesh);
    let (b, _) = join_ready(&mut mesh);
    report_all(&mut mesh, &[&a, &b]);
    assert!(mesh.publish(&a, video()).unwrap().is_empty(), "Direct: nothing through the SFU");
}

#[test]
fn a_participant_that_connects_late_skips_direct_publishers() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let b = SessionId::generate();
    room.join(b.clone(), "b").unwrap();
    room.client_offer_answered(&b).unwrap();
    report_all(&mut room, &[&a, &b]);
    room.publish(&a, video()).unwrap();

    assert!(room.media_connected(&b).unwrap().is_empty());
    assert!(room.subscriptions(&b).unwrap().is_empty());
}

#[test]
fn a_fifth_join_forwards_the_room_and_resubscribes_direct_pairs() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (ta, tb) = (video(), video());
    room.publish(&a, ta.clone()).unwrap();
    room.publish(&b, tb.clone()).unwrap();
    report_all(&mut room, &[&a, &b]);

    let (c, effects) = join_ready(&mut room);

    assert_eq!(room.target(), MediaPath::Forwarded);
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);
    assert_eq!(route_of(&room, &a, &c), Route::Forwarded);
    assert_eq!(room.epoch(&a, &c), Some(0), "a new link starts at epoch 0 whatever the target");
    let epoch = room.epoch(&a, &b).unwrap();
    assert!(effects.routes.contains(&route_notice(&a, &b, MediaPath::Forwarded, epoch)));
    assert!(effects.routes.contains(&route_notice(&b, &a, MediaPath::Forwarded, epoch)));
    let mut subscribe: Vec<_> =
        effects.subscribe.iter().map(|(id, tracks)| (id.clone(), tracks.len())).collect();
    subscribe.sort();
    let mut expected = vec![(a.clone(), 1), (b.clone(), 1)];
    expected.sort();
    assert_eq!(subscribe, expected);
    let mut uplinks = effects.uplinks.clone();
    uplinks.sort();
    let mut expected = vec![(a, true), (b, true)];
    expected.sort();
    assert_eq!(uplinks, expected, "uplinks resume with the SFU path");
}

#[test]
fn to_forwarded_closes_the_link_once_both_sides_reported() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room);
    // a-b was ToDirect when c joined; it is ToForwarded now.
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);

    let effects = merged(report_all(&mut room, &[&a, &b]));

    assert_eq!(route_of(&room, &a, &b), Route::Forwarded);
    let mut closes = effects.link_closes.clone();
    closes.sort_by(|x, y| x.to.cmp(&y.to));
    let mut expected = vec![
        LinkClose { to: a.clone(), peer: b.clone() },
        LinkClose { to: b.clone(), peer: a.clone() },
    ];
    expected.sort_by(|x, y| x.to.cmp(&y.to));
    assert_eq!(closes, expected);
    assert_eq!(route_of(&room, &a, &c), Route::Forwarded);
}

#[test]
fn to_forwarded_moves_back_to_direct_when_the_room_shrinks() {
    let mut room = mesh_room(3);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room);
    report_all(&mut room, &[&a, &b, &c]);
    let (d, _) = join_ready(&mut room);
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);

    room.leave(&c).unwrap();
    assert_eq!(room.target(), MediaPath::Forwarded, "at the limit nothing changes");
    let departure = room.leave(&d).unwrap();

    assert_eq!(room.target(), MediaPath::Direct);
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);
    let epoch = room.epoch(&a, &b).unwrap();
    assert!(departure.effects.routes.contains(&route_notice(&a, &b, MediaPath::Direct, epoch)));
    assert!(
        departure.effects.timeouts.iter().any(|t| t.pair == Pair::new(&a, &b) && t.epoch == epoch)
    );
    assert!(departure.effects.subscribe.is_empty(), "SFU subscriptions stay until Direct");
    assert!(departure.effects.unsubscribe.is_empty());
}

#[test]
fn to_direct_turns_back_when_the_target_flips_to_forwarded() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);

    let (_c, effects) = join_ready(&mut room);

    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);
    assert!(effects.routes.contains(&route_notice(&a, &b, MediaPath::Forwarded, 2)));
}

#[test]
fn the_timeout_pins_the_room_to_forwarded_without_touching_subscriptions() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    room.publish(&a, video()).unwrap();
    let pair = Pair::new(&a, &b);

    assert_eq!(room.link_timeout(&pair, 0), Effects::default(), "stale epoch");
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);

    let effects = room.link_timeout(&pair, 1);

    assert!(room.is_pinned());
    assert_eq!(room.target(), MediaPath::Forwarded);
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);
    assert!(effects.routes.contains(&route_notice(&a, &b, MediaPath::Forwarded, 2)));
    assert!(effects.subscribe.is_empty() && effects.unsubscribe.is_empty());

    assert_eq!(room.link_timeout(&pair, 1), Effects::default(), "fires only once");
}

#[test]
fn the_timeout_of_a_vanished_or_settled_link_does_nothing() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let pair = Pair::new(&a, &b);
    report_all(&mut room, &[&a, &b]);
    assert_eq!(room.link_timeout(&pair, 1), Effects::default());
    assert!(!room.is_pinned());

    room.leave(&b).unwrap();
    assert_eq!(room.link_timeout(&pair, 2), Effects::default());
}

#[test]
fn the_pin_is_cleared_by_the_next_join_or_leave() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    room.link_timeout(&Pair::new(&a, &b), 1);
    assert!(room.is_pinned());

    let (_c, effects) = join_ready(&mut room);

    assert!(!room.is_pinned());
    assert_eq!(room.target(), MediaPath::Direct, "3 participants are below the limit of 4");
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);
    assert!(effects.timeouts.iter().any(|t| t.pair == Pair::new(&a, &b)));
}

#[test]
fn leaving_closes_direct_links_of_the_others_but_not_forwarded_ones() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room); // a-c, b-c are Forwarded; a-b is ToForwarded

    let departure = room.leave(&a).unwrap();
    assert_eq!(departure.remaining.len(), 2);
    assert_eq!(departure.effects.link_closes, vec![LinkClose { to: b.clone(), peer: a.clone() }]);
    assert_eq!(room.route(&b, &c), Some(Route::Forwarded));
    assert_eq!(room.route(&a, &b), None);
}

#[test]
fn leaving_unsubscribes_the_leavers_tracks_and_reevaluates_uplinks() {
    let mut room = mesh_room(3);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room);
    report_all(&mut room, &[&a, &b, &c]);
    let (d, _) = join_ready(&mut room);
    let track = video();
    room.publish(&d, track.clone()).unwrap();
    report_all(&mut room, &[&a, &b, &c]); // a-b, a-c, b-c are Forwarded again
    assert_eq!(room.subscriptions(&a).unwrap(), vec![track.id.clone()]);

    let departure = room.leave(&d).unwrap();

    let mut unsubscribed: Vec<_> =
        departure.effects.unsubscribe.iter().map(|(id, _)| id.clone()).collect();
    unsubscribed.sort();
    let mut expected = vec![a, b, c];
    expected.sort();
    assert_eq!(unsubscribed, expected);
    assert!(departure.effects.uplinks.is_empty(), "nobody was paused");
}

#[test]
fn an_uplink_is_active_while_alone_or_with_any_non_direct_link() {
    let mut room = mesh_room(4);
    let (a, alone) = join_ready(&mut room);
    assert!(alone.uplinks.is_empty());
    let (b, _) = join_ready(&mut room);
    let (c, third) = join_ready(&mut room);
    assert!(third.uplinks.is_empty());

    // a-b established; c has only moving links.
    let epoch = room.epoch(&a, &b).unwrap();
    room.route_live(&a, &b, epoch);
    let effects = room.route_live(&b, &a, epoch);
    assert!(effects.uplinks.is_empty(), "a and b still have a moving link to c");

    let effects = merged(report_all(&mut room, &[&a, &c]));
    assert_eq!(effects.uplinks, vec![(a, false)], "a is all direct, c still has b-c moving");

    let mut effects = merged(report_all(&mut room, &[&b, &c])).uplinks;
    effects.sort();
    let mut expected = vec![(b, false), (c, false)];
    expected.sort();
    assert_eq!(effects, expected);
}

#[test]
fn relaying_is_allowed_only_while_a_direct_link_exists() {
    let mut room = mesh_room(2);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let stranger = SessionId::generate();
    assert!(room.may_relay(&a, &b), "ToDirect");
    assert!(!room.may_relay(&a, &stranger));
    assert!(!room.may_relay(&stranger, &a));

    report_all(&mut room, &[&a, &b]);
    assert!(room.may_relay(&a, &b), "Direct");

    let (_c, _) = join_ready(&mut room);
    assert!(room.may_relay(&b, &a), "ToForwarded");

    report_all(&mut room, &[&a, &b]);
    assert!(!room.may_relay(&a, &b), "Forwarded: the direct link is closed");
}

#[test]
fn keys_are_relayed_between_any_two_participants_on_every_path() {
    let mut room = mesh_room(0);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room);
    let stranger = SessionId::generate();
    // A room without mesh has no direct link, and keys still go.
    assert!(!room.may_relay(&a, &b));
    assert!(room.may_relay_keys(&a, &b));
    assert!(room.may_relay_keys(&c, &a));
    assert!(!room.may_relay_keys(&a, &a), "not to oneself");
    assert!(!room.may_relay_keys(&a, &stranger));
    assert!(!room.may_relay_keys(&stranger, &a));

    room.leave(&b).unwrap();
    assert!(!room.may_relay_keys(&a, &b), "not to someone who left");
}

#[test]
fn reports_from_outside_or_for_a_stale_epoch_do_nothing() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let stranger = SessionId::generate();

    assert_eq!(room.route_live(&stranger, &a, 1), Effects::default());
    assert_eq!(room.route_live(&a, &stranger, 1), Effects::default());
    assert_eq!(room.route_live(&a, &b, 7), Effects::default());
    room.route_live(&a, &b, 1);
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);
}

#[test]
fn with_mesh_off_nothing_ever_leaves_forwarded() {
    let mut room = Room::new(ChannelId::parse("general").unwrap());
    let mut all = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..4 {
        let (id, effects) = join_ready(&mut room);
        ids.push(id);
        all.push(effects);
    }
    let track = video();
    assert_eq!(room.publish(&ids[0], track).unwrap().len(), 3);
    for id in &ids[1..] {
        all.push(room.leave(id).unwrap().effects);
    }

    assert!(all.iter().all(Effects::is_empty));
    assert_eq!(room.target(), MediaPath::Forwarded);
}

#[test]
fn a_failed_direct_link_pins_the_room_and_returns_its_subscriptions() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    let (c, _) = join_ready(&mut room);
    let track = video();
    room.publish(&a, track.clone()).unwrap();
    report_all(&mut room, &[&a, &b, &c]);
    assert!(room.subscriptions(&b).unwrap().is_empty());

    let effects = room.link_failed(&b, &a);

    assert!(room.is_pinned());
    assert_eq!(room.target(), MediaPath::Forwarded);
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);
    assert_eq!(route_of(&room, &b, &c), Route::ToForwarded, "every link follows the target");
    let mut subscribed: Vec<_> = effects.subscribe.iter().map(|(id, _)| id.clone()).collect();
    subscribed.sort();
    let mut expected = vec![b.clone(), c.clone()];
    expected.sort();
    assert_eq!(subscribed, expected);
    assert_eq!(room.subscriptions(&b).unwrap(), vec![track.id]);
}

#[test]
fn a_link_failure_report_is_ignored_without_a_direct_connection() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let stranger = SessionId::generate();
    assert_eq!(room.link_failed(&a, &stranger), Effects::default());
    assert_eq!(room.link_failed(&stranger, &a), Effects::default());

    let mut off = Room::new(ChannelId::parse("general").unwrap());
    let (x, _) = join_ready(&mut off);
    let (y, _) = join_ready(&mut off);
    assert_eq!(off.link_failed(&x, &y), Effects::default(), "Forwarded link");
    assert!(!off.is_pinned());
}

#[test]
fn a_failure_reported_during_to_direct_or_to_forwarded_pins_too() {
    let mut room = mesh_room(4);
    let (a, _) = join_ready(&mut room);
    let (b, _) = join_ready(&mut room);
    assert_eq!(route_of(&room, &a, &b), Route::ToDirect);
    room.link_failed(&a, &b);
    assert_eq!(route_of(&room, &a, &b), Route::ToForwarded);
    assert!(room.is_pinned());
    let again = room.link_failed(&b, &a);
    assert!(again.routes.is_empty(), "already moving back; nothing new to say");
}
