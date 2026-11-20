//! In-memory implementations of the community ports, for tests and for running a
//! single instance without a database or Redis. Nothing survives a restart.

use super::ports::{
    CommunityEvent, CommunityEvents, CommunityRepository, CommunityStoreError, EventsError,
    Redemption, VoiceRoster,
};
use crate::conference::VoiceEntry;
use async_trait::async_trait;
use melior_domain::{
    Category, CategoryId, Channel, ChannelId, Community, CommunityId, CommunityState, Invite,
    InviteCode, Member, OverrideTarget, Role, RoleId, SessionId, UserId,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
struct Stored {
    communities: BTreeMap<CommunityId, CommunityState>,
    members: BTreeMap<CommunityId, BTreeMap<UserId, Member>>,
    invites: BTreeMap<InviteCode, Invite>,
}

impl Stored {
    fn state_mut(&mut self, id: &CommunityId) -> Result<&mut CommunityState, CommunityStoreError> {
        self.communities
            .get_mut(id)
            .ok_or_else(|| CommunityStoreError(format!("unknown community {id}")))
    }
}

/// A [`CommunityRepository`] in memory.
#[derive(Default)]
pub struct InMemoryCommunityRepository {
    stored: Mutex<Stored>,
}

fn replace<T>(items: &mut Vec<T>, item: T, same: impl Fn(&T) -> bool) {
    match items.iter_mut().find(|existing| same(existing)) {
        Some(existing) => *existing = item,
        None => items.push(item),
    }
}

#[async_trait]
impl CommunityRepository for InMemoryCommunityRepository {
    async fn create(
        &self,
        state: &CommunityState,
        owner: &Member,
    ) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        stored.communities.insert(state.id().clone(), state.clone());
        let members = BTreeMap::from([(owner.user_id.clone(), owner.clone())]);
        stored.members.insert(state.id().clone(), members);
        Ok(())
    }

    async fn load(&self, id: &CommunityId) -> Result<Option<CommunityState>, CommunityStoreError> {
        Ok(lock(&self.stored).communities.get(id).cloned())
    }

    async fn load_by_channel(
        &self,
        channel_id: &ChannelId,
    ) -> Result<Option<CommunityState>, CommunityStoreError> {
        let stored = lock(&self.stored);
        let found = stored.communities.values().find(|state| state.channel(channel_id).is_some());
        Ok(found.cloned())
    }

    async fn list_for(&self, user: &UserId) -> Result<Vec<Community>, CommunityStoreError> {
        let stored = lock(&self.stored);
        let mut communities: Vec<Community> = stored
            .members
            .iter()
            .filter(|(_, members)| members.contains_key(user))
            .filter_map(|(id, _)| stored.communities.get(id))
            .map(|state| state.community.clone())
            .collect();
        communities.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
        Ok(communities)
    }

    async fn rename(&self, community: &Community) -> Result<(), CommunityStoreError> {
        lock(&self.stored).state_mut(&community.id)?.community = community.clone();
        Ok(())
    }

    async fn delete(&self, id: &CommunityId) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        stored.communities.remove(id);
        stored.members.remove(id);
        stored.invites.retain(|_, invite| invite.community_id != *id);
        Ok(())
    }

    async fn member(
        &self,
        id: &CommunityId,
        user: &UserId,
    ) -> Result<Option<Member>, CommunityStoreError> {
        Ok(lock(&self.stored).members.get(id).and_then(|members| members.get(user)).cloned())
    }

    async fn members(&self, id: &CommunityId) -> Result<Vec<Member>, CommunityStoreError> {
        let stored = lock(&self.stored);
        Ok(stored.members.get(id).map(|m| m.values().cloned().collect()).unwrap_or_default())
    }

    async fn member_count(&self, id: &CommunityId) -> Result<u64, CommunityStoreError> {
        let count = lock(&self.stored).members.get(id).map_or(0, BTreeMap::len);
        Ok(u64::try_from(count).unwrap_or(u64::MAX))
    }

    async fn remove_member(
        &self,
        id: &CommunityId,
        user: &UserId,
    ) -> Result<(), CommunityStoreError> {
        if let Some(members) = lock(&self.stored).members.get_mut(id) {
            members.remove(user);
        }
        Ok(())
    }

    async fn set_member_roles(
        &self,
        id: &CommunityId,
        member: &Member,
    ) -> Result<(), CommunityStoreError> {
        if let Some(members) = lock(&self.stored).members.get_mut(id) {
            members.insert(member.user_id.clone(), member.clone());
        }
        Ok(())
    }

    async fn save_role(&self, id: &CommunityId, role: &Role) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        replace(&mut stored.state_mut(id)?.roles, role.clone(), |r| r.id == role.id);
        Ok(())
    }

    async fn delete_role(
        &self,
        id: &CommunityId,
        role: &RoleId,
    ) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        let state = stored.state_mut(id)?;
        state.roles.retain(|r| r.id != *role);
        let names_role =
            |entry: &melior_domain::Override| entry.target == OverrideTarget::Role(role.clone());
        for category in &mut state.categories {
            category.overrides.retain(|entry| !names_role(entry));
        }
        for channel in &mut state.channels {
            channel.overrides.retain(|entry| !names_role(entry));
        }
        for member in stored.members.get_mut(id).into_iter().flat_map(BTreeMap::values_mut) {
            member.role_ids.remove(role);
        }
        Ok(())
    }

    async fn save_category(
        &self,
        id: &CommunityId,
        category: &Category,
    ) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        replace(&mut stored.state_mut(id)?.categories, category.clone(), |c| c.id == category.id);
        Ok(())
    }

    async fn delete_category(
        &self,
        id: &CommunityId,
        category: &CategoryId,
    ) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        let state = stored.state_mut(id)?;
        state.categories.retain(|c| c.id != *category);
        for channel in &mut state.channels {
            if channel.category_id.as_ref() == Some(category) {
                channel.category_id = None;
            }
        }
        Ok(())
    }

    async fn save_channel(
        &self,
        id: &CommunityId,
        channel: &Channel,
    ) -> Result<(), CommunityStoreError> {
        let mut stored = lock(&self.stored);
        replace(&mut stored.state_mut(id)?.channels, channel.clone(), |c| c.id == channel.id);
        Ok(())
    }

    async fn delete_channel(&self, channel: &ChannelId) -> Result<(), CommunityStoreError> {
        for state in lock(&self.stored).communities.values_mut() {
            state.channels.retain(|c| c.id != *channel);
        }
        Ok(())
    }

    async fn save_invite(&self, invite: &Invite) -> Result<(), CommunityStoreError> {
        lock(&self.stored).invites.insert(invite.code.clone(), invite.clone());
        Ok(())
    }

    async fn invite(&self, code: &InviteCode) -> Result<Option<Invite>, CommunityStoreError> {
        Ok(lock(&self.stored).invites.get(code).cloned())
    }

    async fn invites(&self, id: &CommunityId) -> Result<Vec<Invite>, CommunityStoreError> {
        let stored = lock(&self.stored);
        Ok(stored.invites.values().filter(|i| i.community_id == *id).cloned().collect())
    }

    async fn delete_invite(&self, code: &InviteCode) -> Result<(), CommunityStoreError> {
        lock(&self.stored).invites.remove(code);
        Ok(())
    }

    async fn redeem_invite(
        &self,
        code: &InviteCode,
        member: &Member,
        now_ms: i64,
    ) -> Result<Redemption, CommunityStoreError> {
        let mut stored = lock(&self.stored);
        let Some(invite) = stored.invites.get(code).cloned() else {
            return Ok(Redemption::Unknown);
        };
        let community = invite.community_id.clone();
        let members = stored.members.entry(community.clone()).or_default();
        if members.contains_key(&member.user_id) {
            return Ok(Redemption::AlreadyMember(community));
        }
        if !invite.is_usable(now_ms) {
            return Ok(Redemption::NotUsable);
        }
        members.insert(member.user_id.clone(), member.clone());
        if let Some(stored_invite) = stored.invites.get_mut(code) {
            stored_invite.uses += 1;
        }
        Ok(Redemption::Joined(community))
    }
}

/// How many events a slow subscriber may fall behind before it misses some.
const EVENT_BUFFER: usize = 256;

/// [`CommunityEvents`] within one process, on tokio broadcast channels.
#[derive(Default)]
pub struct InMemoryCommunityEvents {
    channels: Mutex<HashMap<CommunityId, broadcast::Sender<CommunityEvent>>>,
}

impl InMemoryCommunityEvents {
    /// Hands an event to the local subscribers of a community. Adapters that carry
    /// events between instances call this for what they receive.
    pub fn deliver(&self, id: &CommunityId, event: CommunityEvent) {
        let mut channels = lock(&self.channels);
        let Some(sender) = channels.get(id) else {
            return;
        };
        if sender.send(event).is_err() {
            channels.remove(id);
        }
    }
}

#[async_trait]
impl CommunityEvents for InMemoryCommunityEvents {
    async fn publish(&self, id: &CommunityId, event: CommunityEvent) -> Result<(), EventsError> {
        self.deliver(id, event);
        Ok(())
    }

    fn subscribe(&self, id: &CommunityId) -> broadcast::Receiver<CommunityEvent> {
        lock(&self.channels)
            .entry(id.clone())
            .or_insert_with(|| broadcast::channel(EVENT_BUFFER).0)
            .subscribe()
    }
}

/// A community's live entries, each with the moment it expires.
type Expiring = HashMap<SessionId, (VoiceEntry, Instant)>;

/// A [`VoiceRoster`] in memory, with expiry.
#[derive(Default)]
pub struct InMemoryVoiceRoster {
    entries: Mutex<HashMap<CommunityId, Expiring>>,
}

#[async_trait]
impl VoiceRoster for InMemoryVoiceRoster {
    async fn put(
        &self,
        id: &CommunityId,
        entry: &VoiceEntry,
        ttl: Duration,
    ) -> Result<(), CommunityStoreError> {
        let expires = Instant::now() + ttl;
        let mut entries = lock(&self.entries);
        entries
            .entry(id.clone())
            .or_default()
            .insert(entry.session_id.clone(), (entry.clone(), expires));
        Ok(())
    }

    async fn remove(
        &self,
        id: &CommunityId,
        session: &SessionId,
    ) -> Result<(), CommunityStoreError> {
        if let Some(sessions) = lock(&self.entries).get_mut(id) {
            sessions.remove(session);
        }
        Ok(())
    }

    async fn list(&self, id: &CommunityId) -> Result<Vec<VoiceEntry>, CommunityStoreError> {
        let now = Instant::now();
        let mut entries = lock(&self.entries);
        let Some(sessions) = entries.get_mut(id) else {
            return Ok(Vec::new());
        };
        sessions.retain(|_, (_, expires)| *expires > now);
        let mut live: Vec<VoiceEntry> = sessions.values().map(|(entry, _)| entry.clone()).collect();
        live.sort_by(|a, b| (&a.channel_id, &a.session_id).cmp(&(&b.channel_id, &b.session_id)));
        Ok(live)
    }
}
