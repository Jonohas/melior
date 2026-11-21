//! A room run by its channel's plan (capacity, forced path) and its voice states.

use super::{
    ChannelId, ChannelMedia, DomainError, MediaKind, MediaMode, MediaPath, MeshPolicy, Room,
    RoomPlan, SessionId, Track, TrackId, VoiceState,
};

fn room(mode: MediaMode, user_limit: Option<u8>, mesh: usize) -> Room {
    let policy = MeshPolicy::new(mesh);
    let plan = RoomPlan::resolve(ChannelMedia { mode, user_limit }, policy);
    Room::with_plan(ChannelId::parse("general").unwrap(), policy, plan)
}

fn join(room: &mut Room) -> SessionId {
    let id = SessionId::generate();
    room.join(id.clone(), "someone").unwrap();
    id
}

fn join_ready(room: &mut Room) -> SessionId {
    let id = join(room);
    room.client_offer_answered(&id).unwrap();
    room.media_connected(&id).unwrap();
    id
}

fn report_all(room: &mut Room, ids: &[&SessionId]) {
    for (index, a) in ids.iter().enumerate() {
        for b in &ids[index + 1..] {
            let epoch = room.epoch(a, b).unwrap();
            room.route_live(a, b, epoch);
            room.route_live(b, a, epoch);
        }
    }
}

fn video() -> Track {
    Track::new(TrackId::generate(), MediaKind::Video)
}

#[allow(clippy::fn_params_excessive_bools)]
fn claim(muted: bool, camera: bool, screen: bool, speaking: bool) -> VoiceState {
    VoiceState { muted, camera, screen, speaking }
}

#[test]
fn a_room_refuses_a_join_once_it_holds_its_capacity() {
    let mut room = room(MediaMode::Server, Some(2), 4);
    join(&mut room);
    let second = join(&mut room);

    assert_eq!(room.join(SessionId::generate(), "late"), Err(DomainError::ChannelFull));

    room.leave(&second).unwrap();
    assert!(room.join(SessionId::generate(), "now").is_ok(), "a seat came free");
}

#[test]
fn a_room_without_a_limit_admits_anyone() {
    let mut room = room(MediaMode::Auto, None, 4);
    for _ in 0..12 {
        join(&mut room);
    }
    assert_eq!(room.len(), 12);
}

#[test]
fn a_server_channel_stays_forwarded_however_small() {
    let mut room = room(MediaMode::Server, None, 4);
    join(&mut room);
    join(&mut room);
    assert_eq!(room.target(), MediaPath::Forwarded);
}

#[test]
fn a_peer_to_peer_channel_stays_direct_up_to_its_capacity() {
    let mut room = room(MediaMode::PeerToPeer, None, 4);
    for _ in 0..4 {
        join(&mut room);
        assert_eq!(room.target(), MediaPath::Direct);
    }
    assert_eq!(room.join(SessionId::generate(), "fifth"), Err(DomainError::ChannelFull));
}

#[test]
fn an_auto_channel_still_switches_with_its_size() {
    let mut room = room(MediaMode::Auto, None, 2);
    join(&mut room);
    join(&mut room);
    assert_eq!(room.target(), MediaPath::Direct);
    join(&mut room);
    assert_eq!(room.target(), MediaPath::Forwarded);
}

#[test]
fn a_changed_plan_applies_its_capacity_from_the_next_join() {
    let mut room = room(MediaMode::Auto, None, 4);
    join(&mut room);
    join(&mut room);

    let policy = MeshPolicy::new(4);
    let media = ChannelMedia { mode: MediaMode::Server, user_limit: Some(2) };
    room.apply_plan(RoomPlan::resolve(media, policy));

    assert_eq!(room.len(), 2, "nobody is removed");
    assert_eq!(room.join(SessionId::generate(), "third"), Err(DomainError::ChannelFull));
}

#[test]
fn the_roster_carries_every_voice_state_and_everything_is_off_on_join() {
    let mut room = room(MediaMode::Auto, None, 0);
    let alice = join(&mut room);
    assert_eq!(room.roster()[0].voice, VoiceState::default());

    room.set_voice(&alice, claim(true, true, false, false)).unwrap();

    assert_eq!(room.roster()[0].voice, claim(true, true, false, false));
}

#[test]
fn set_voice_reports_a_change_only_once() {
    let mut room = room(MediaMode::Auto, None, 0);
    let alice = join(&mut room);

    let first = room.set_voice(&alice, claim(false, true, false, true)).unwrap();
    let again = room.set_voice(&alice, claim(false, true, false, true)).unwrap();

    assert_eq!(first, Some(claim(false, true, false, true)));
    assert_eq!(again, None);
}

#[test]
fn a_muted_participant_is_never_speaking() {
    let mut room = room(MediaMode::Auto, None, 0);
    let alice = join(&mut room);

    let state = room.set_voice(&alice, claim(true, false, false, true)).unwrap();

    assert_eq!(state, Some(claim(true, false, false, false)));
    assert_eq!(room.set_voice(&alice, claim(true, false, false, true)).unwrap(), None);
}

#[test]
fn a_screen_claim_needs_a_screen_track_on_a_forwarded_room() {
    let mut room = room(MediaMode::Server, None, 4);
    let alice = join_ready(&mut room);
    join_ready(&mut room);

    let refused = room.set_voice(&alice, claim(false, false, true, false)).unwrap();
    assert_eq!(refused, None, "no screen is recorded, so nothing changed");

    room.publish(&alice, video()).unwrap();
    room.publish(&alice, video()).unwrap();
    let accepted = room.set_voice(&alice, claim(false, true, true, false)).unwrap();
    assert_eq!(accepted, Some(claim(false, true, true, false)));
}

#[test]
fn a_screen_claimed_before_its_track_arrives_is_shown_once_the_track_is_published() {
    let mut room = room(MediaMode::Server, None, 4);
    let alice = join_ready(&mut room);
    join_ready(&mut room);
    room.publish(&alice, video()).unwrap();

    assert_eq!(
        room.set_voice(&alice, claim(false, true, true, false)).unwrap(),
        Some(claim(false, true, false, false))
    );
    let screen = video();
    room.publish(&alice, screen.clone()).unwrap();

    assert_eq!(room.reconcile_voice(&alice).unwrap(), Some(claim(false, true, true, false)));
    assert_eq!(room.reconcile_voice(&alice).unwrap(), None, "reported once");

    room.unpublish(&alice, &screen.id);
    assert_eq!(
        room.reconcile_voice(&alice).unwrap(),
        Some(claim(false, true, false, false)),
        "a screen that stopped is no longer shown, whatever the claim says"
    );
}

#[test]
fn a_screen_claim_is_accepted_when_every_link_of_the_participant_is_direct() {
    let mut room = room(MediaMode::PeerToPeer, None, 4);
    let alice = join_ready(&mut room);
    let bob = join_ready(&mut room);
    report_all(&mut room, &[&alice, &bob]);

    let state = room.set_voice(&alice, claim(false, false, true, false)).unwrap();

    assert_eq!(state, Some(claim(false, false, true, false)));
}

#[test]
fn a_participant_alone_cannot_claim_a_screen_it_does_not_send() {
    let mut room = room(MediaMode::PeerToPeer, None, 4);
    let alice = join_ready(&mut room);

    assert_eq!(room.set_voice(&alice, claim(false, false, true, false)).unwrap(), None);
}

#[test]
fn an_unknown_participant_has_no_voice() {
    let mut room = room(MediaMode::Auto, None, 0);
    let stranger = SessionId::generate();
    assert_eq!(
        room.set_voice(&stranger, VoiceState::default()),
        Err(DomainError::UnknownParticipant(stranger.clone()))
    );
    assert_eq!(room.voice(&stranger), Err(DomainError::UnknownParticipant(stranger)));
}
