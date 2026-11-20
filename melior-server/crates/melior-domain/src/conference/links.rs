//! Links and routes: what the server keeps for each pair of participants.

use super::{MediaPath, SessionId};
use std::collections::{BTreeMap, BTreeSet};

/// The state of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Media goes through the SFU only.
    Forwarded,
    /// Moving to direct: both paths carry media.
    ToDirect,
    /// Media goes browser to browser only.
    Direct,
    /// Moving back to the SFU: both paths carry media.
    ToForwarded,
}

impl Route {
    /// Whether a direct connection between the two exists in this state.
    #[must_use]
    pub fn has_direct_link(self) -> bool {
        match self {
            Self::Forwarded => false,
            Self::ToDirect | Self::Direct | Self::ToForwarded => true,
        }
    }
}

/// An unordered pair of participants. The lower session id comes first, which is
/// also the side that makes the offer on the direct connection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pair {
    low: SessionId,
    high: SessionId,
}

impl Pair {
    #[must_use]
    pub fn new(a: &SessionId, b: &SessionId) -> Self {
        if a <= b {
            Self { low: a.clone(), high: b.clone() }
        } else {
            Self { low: b.clone(), high: a.clone() }
        }
    }

    #[must_use]
    pub fn low(&self) -> &SessionId {
        &self.low
    }

    #[must_use]
    pub fn high(&self) -> &SessionId {
        &self.high
    }

    #[must_use]
    pub fn contains(&self, id: &SessionId) -> bool {
        self.low == *id || self.high == *id
    }

    /// Both sides, each with the other one: `(who to tell, about whom)`.
    #[must_use]
    pub fn facing(&self) -> [(&SessionId, &SessionId); 2] {
        [(&self.low, &self.high), (&self.high, &self.low)]
    }
}

/// What a change to the links means for the clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// Both sides are told to play over `path`.
    Moving { pair: Pair, path: MediaPath, epoch: u64 },
    /// A `ToDirect` timeout has to be armed for this epoch.
    TimeoutArmed { pair: Pair, epoch: u64 },
    /// Both sides are told to close the direct connection.
    Closed { pair: Pair },
}

#[derive(Debug)]
struct Link {
    route: Route,
    epoch: u64,
    reported_low: bool,
    reported_high: bool,
}

impl Link {
    fn new() -> Self {
        Self { route: Route::Forwarded, epoch: 0, reported_low: false, reported_high: false }
    }

    /// Every transition increments the epoch and clears the reports.
    fn move_to(&mut self, route: Route) {
        self.route = route;
        self.epoch = self.epoch.saturating_add(1);
        self.reported_low = false;
        self.reported_high = false;
    }

    fn is_moving(&self) -> bool {
        matches!(self.route, Route::ToDirect | Route::ToForwarded)
    }

    fn both_reported(&self) -> bool {
        self.reported_low && self.reported_high
    }
}

/// Every link of a room, one per unordered pair of participants.
#[derive(Debug, Default)]
pub struct Links {
    links: BTreeMap<Pair, Link>,
}

impl Links {
    #[must_use]
    pub fn route(&self, a: &SessionId, b: &SessionId) -> Option<Route> {
        self.links.get(&Pair::new(a, b)).map(|link| link.route)
    }

    #[must_use]
    pub fn epoch(&self, a: &SessionId, b: &SessionId) -> Option<u64> {
        self.links.get(&Pair::new(a, b)).map(|link| link.epoch)
    }

    /// Creates the link between two participants, in `Forwarded` at epoch 0.
    pub fn add(&mut self, a: &SessionId, b: &SessionId) {
        self.links.insert(Pair::new(a, b), Link::new());
    }

    /// Removes every link of a participant. Returns the peers whose link had a direct
    /// connection, which have to be told to close it.
    pub fn remove_participant(&mut self, id: &SessionId) -> Vec<SessionId> {
        let mut with_direct_link = Vec::new();
        self.links.retain(|pair, link| {
            if !pair.contains(id) {
                return true;
            }
            if link.route.has_direct_link() {
                let peer = if pair.low() == id { pair.high() } else { pair.low() };
                with_direct_link.push(peer.clone());
            }
            false
        });
        with_direct_link
    }

    /// Moves every link whose route does not match the room's target one step towards it.
    pub fn apply_target(&mut self, target: MediaPath) -> Vec<LinkEvent> {
        let mut events = Vec::new();
        for (pair, link) in &mut self.links {
            match (target, link.route) {
                (MediaPath::Direct, Route::Forwarded | Route::ToForwarded) => {
                    link.move_to(Route::ToDirect);
                    events.push(moving(pair, MediaPath::Direct, link.epoch));
                    events.push(LinkEvent::TimeoutArmed { pair: pair.clone(), epoch: link.epoch });
                }
                (MediaPath::Forwarded, Route::ToDirect | Route::Direct) => {
                    link.move_to(Route::ToForwarded);
                    events.push(moving(pair, MediaPath::Forwarded, link.epoch));
                }
                (MediaPath::Direct, Route::ToDirect | Route::Direct)
                | (MediaPath::Forwarded, Route::Forwarded | Route::ToForwarded) => {}
            }
        }
        events
    }

    /// A participant reports it plays `peer`'s media over the path of route `epoch`.
    /// A report for another epoch, for a link that is not moving or from someone who is
    /// not on the link is ignored.
    pub fn report(&mut self, reporter: &SessionId, peer: &SessionId, epoch: u64) -> Vec<LinkEvent> {
        let pair = Pair::new(reporter, peer);
        let Some(link) = self.links.get_mut(&pair) else {
            return Vec::new();
        };
        if link.epoch != epoch || !link.is_moving() || reporter == peer {
            return Vec::new();
        }
        if pair.low() == reporter {
            link.reported_low = true;
        } else {
            link.reported_high = true;
        }
        if !link.both_reported() {
            return Vec::new();
        }
        match link.route {
            Route::ToDirect => {
                link.move_to(Route::Direct);
                Vec::new()
            }
            Route::ToForwarded => {
                link.move_to(Route::Forwarded);
                vec![LinkEvent::Closed { pair }]
            }
            Route::Forwarded | Route::Direct => Vec::new(),
        }
    }

    /// Whether the `ToDirect` timeout armed for `epoch` has found the link still waiting.
    #[must_use]
    pub fn timed_out(&self, pair: &Pair, epoch: u64) -> bool {
        self.links
            .get(pair)
            .is_some_and(|link| link.epoch == epoch && link.route == Route::ToDirect)
    }

    /// The participants whose uplink is paused: they have links and all are `Direct`.
    #[must_use]
    pub fn paused_uplinks(&self) -> BTreeSet<SessionId> {
        let mut counts: BTreeMap<&SessionId, (usize, usize)> = BTreeMap::new();
        for (pair, link) in &self.links {
            for id in [pair.low(), pair.high()] {
                let (total, direct) = counts.entry(id).or_default();
                *total += 1;
                *direct += usize::from(link.route == Route::Direct);
            }
        }
        counts
            .into_iter()
            .filter(|(_, (total, direct))| total == direct)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

fn moving(pair: &Pair, path: MediaPath, epoch: u64) -> LinkEvent {
    LinkEvent::Moving { pair: pair.clone(), path, epoch }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        links: Links,
        a: SessionId,
        b: SessionId,
    }

    fn ids() -> (SessionId, SessionId) {
        let (x, y) = (SessionId::generate(), SessionId::generate());
        if x < y { (x, y) } else { (y, x) }
    }

    fn fixture() -> Fixture {
        let (a, b) = ids();
        let mut links = Links::default();
        links.add(&a, &b);
        Fixture { links, a, b }
    }

    impl Fixture {
        fn route(&self) -> Route {
            self.links.route(&self.a, &self.b).unwrap()
        }

        fn epoch(&self) -> u64 {
            self.links.epoch(&self.a, &self.b).unwrap()
        }

        fn pair(&self) -> Pair {
            Pair::new(&self.a, &self.b)
        }

        fn report_both(&mut self) -> Vec<LinkEvent> {
            let epoch = self.epoch();
            let mut events = self.links.report(&self.a, &self.b, epoch);
            events.extend(self.links.report(&self.b, &self.a, epoch));
            events
        }

        fn start_direct(&mut self) {
            self.links.apply_target(MediaPath::Direct);
        }

        fn establish_direct(&mut self) {
            self.start_direct();
            self.report_both();
        }
    }

    #[test]
    fn a_new_link_starts_forwarded_at_epoch_zero_whatever_the_target() {
        let f = fixture();
        assert_eq!(f.route(), Route::Forwarded);
        assert_eq!(f.epoch(), 0);
    }

    #[test]
    fn forwarded_moves_to_direct_with_a_route_and_a_timeout() {
        let mut f = fixture();
        let events = f.links.apply_target(MediaPath::Direct);

        assert_eq!(f.route(), Route::ToDirect);
        assert_eq!(f.epoch(), 1);
        assert_eq!(
            events,
            vec![
                LinkEvent::Moving { pair: f.pair(), path: MediaPath::Direct, epoch: 1 },
                LinkEvent::TimeoutArmed { pair: f.pair(), epoch: 1 },
            ]
        );
    }

    #[test]
    fn to_direct_becomes_direct_when_both_sides_reported_this_epoch() {
        let mut f = fixture();
        f.start_direct();

        assert!(f.links.report(&f.a, &f.b, 1).is_empty());
        assert_eq!(f.route(), Route::ToDirect, "one side is not enough");

        assert!(f.links.report(&f.b, &f.a, 1).is_empty());
        assert_eq!(f.route(), Route::Direct);
        assert_eq!(f.epoch(), 2, "every transition increments the epoch");
    }

    #[test]
    fn reporting_twice_from_one_side_does_not_complete_the_move() {
        let mut f = fixture();
        f.start_direct();
        f.links.report(&f.a, &f.b, 1);
        f.links.report(&f.a, &f.b, 1);
        assert_eq!(f.route(), Route::ToDirect);
    }

    #[test]
    fn to_direct_goes_back_to_forwarded_when_the_target_flips() {
        let mut f = fixture();
        f.start_direct();
        let events = f.links.apply_target(MediaPath::Forwarded);

        assert_eq!(f.route(), Route::ToForwarded);
        assert_eq!(f.epoch(), 2);
        assert_eq!(
            events,
            vec![LinkEvent::Moving { pair: f.pair(), path: MediaPath::Forwarded, epoch: 2 }]
        );
    }

    #[test]
    fn direct_moves_to_forwarded_when_the_target_flips() {
        let mut f = fixture();
        f.establish_direct();
        let events = f.links.apply_target(MediaPath::Forwarded);

        assert_eq!(f.route(), Route::ToForwarded);
        assert_eq!(f.epoch(), 3);
        assert_eq!(
            events,
            vec![LinkEvent::Moving { pair: f.pair(), path: MediaPath::Forwarded, epoch: 3 }]
        );
    }

    #[test]
    fn to_forwarded_closes_the_link_when_both_sides_reported() {
        let mut f = fixture();
        f.establish_direct();
        f.links.apply_target(MediaPath::Forwarded);

        let events = f.report_both();

        assert_eq!(f.route(), Route::Forwarded);
        assert_eq!(f.epoch(), 4);
        assert_eq!(events, vec![LinkEvent::Closed { pair: f.pair() }]);
    }

    #[test]
    fn to_forwarded_moves_back_to_direct_when_the_target_flips_again() {
        let mut f = fixture();
        f.establish_direct();
        f.links.apply_target(MediaPath::Forwarded);
        let events = f.links.apply_target(MediaPath::Direct);

        assert_eq!(f.route(), Route::ToDirect);
        assert_eq!(f.epoch(), 4);
        assert_eq!(
            events,
            vec![
                LinkEvent::Moving { pair: f.pair(), path: MediaPath::Direct, epoch: 4 },
                LinkEvent::TimeoutArmed { pair: f.pair(), epoch: 4 },
            ]
        );
    }

    #[test]
    fn a_target_that_matches_the_route_changes_nothing() {
        let mut f = fixture();
        assert!(f.links.apply_target(MediaPath::Forwarded).is_empty());
        f.start_direct();
        assert!(f.links.apply_target(MediaPath::Direct).is_empty());
        f.report_both();
        assert!(f.links.apply_target(MediaPath::Direct).is_empty());
        assert_eq!(f.route(), Route::Direct);
        assert_eq!(f.epoch(), 2);
    }

    #[test]
    fn reports_for_another_epoch_or_from_strangers_are_ignored() {
        let mut f = fixture();
        f.start_direct();
        let stranger = SessionId::generate();

        assert!(f.links.report(&f.a, &f.b, 0).is_empty());
        assert!(f.links.report(&f.a, &f.b, 2).is_empty());
        assert!(f.links.report(&stranger, &f.b, 1).is_empty());
        assert!(f.links.report(&f.a, &stranger, 1).is_empty());
        assert!(f.links.report(&f.a, &f.a, 1).is_empty());
        assert!(f.links.report(&f.b, &f.b, 1).is_empty());
        f.links.report(&f.b, &f.a, 1);
        assert_eq!(f.route(), Route::ToDirect, "none of them counted");
    }

    #[test]
    fn a_stale_report_after_a_transition_is_ignored() {
        let mut f = fixture();
        f.start_direct();
        f.links.apply_target(MediaPath::Forwarded); // epoch 2
        f.links.report(&f.a, &f.b, 1);
        f.links.report(&f.b, &f.a, 1);
        assert_eq!(f.route(), Route::ToForwarded);
    }

    #[test]
    fn reports_on_settled_links_are_ignored() {
        let mut f = fixture();
        assert!(f.links.report(&f.a, &f.b, 0).is_empty());
        f.establish_direct();
        let epoch = f.epoch();
        assert!(f.links.report(&f.a, &f.b, epoch).is_empty());
        assert!(f.links.report(&f.b, &f.a, epoch).is_empty());
        assert_eq!(f.route(), Route::Direct);
    }

    #[test]
    fn only_a_to_direct_link_of_the_current_epoch_times_out() {
        let mut f = fixture();
        assert!(!f.links.timed_out(&f.pair(), 0), "forwarded");
        f.start_direct();
        assert!(!f.links.timed_out(&f.pair(), 0), "stale epoch");
        assert!(f.links.timed_out(&f.pair(), 1));
        f.report_both();
        assert!(!f.links.timed_out(&f.pair(), 1), "no longer to direct");
        let gone = Pair::new(&SessionId::generate(), &SessionId::generate());
        assert!(!f.links.timed_out(&gone, 1), "vanished link");
    }

    #[test]
    fn removing_a_participant_names_the_peers_that_had_a_direct_connection() {
        let (a, b) = ids();
        let c = SessionId::generate();
        let mut links = Links::default();
        links.add(&a, &b);
        links.apply_target(MediaPath::Direct); // a-b is ToDirect
        links.add(&a, &c); // a-c is Forwarded
        links.add(&b, &c);

        assert_eq!(links.remove_participant(&a), vec![b.clone()]);
        assert_eq!(links.route(&a, &b), None);
        assert_eq!(links.route(&a, &c), None);
        assert_eq!(links.route(&b, &c), Some(Route::Forwarded));
    }

    #[test]
    fn an_uplink_is_paused_only_when_it_has_links_and_all_are_direct() {
        let (a, b) = ids();
        let c = SessionId::generate();
        let mut links = Links::default();
        assert!(links.paused_uplinks().is_empty(), "nobody, nothing paused");

        links.add(&a, &b);
        links.add(&a, &c);
        assert!(links.paused_uplinks().is_empty(), "all forwarded");

        links.apply_target(MediaPath::Direct);
        links.report(&a, &b, 1);
        links.report(&b, &a, 1);
        assert_eq!(links.paused_uplinks(), BTreeSet::from([b.clone()]), "only b has all direct");

        links.report(&a, &c, 1);
        links.report(&c, &a, 1);
        assert_eq!(links.paused_uplinks(), BTreeSet::from([a, b, c]));
    }
}
