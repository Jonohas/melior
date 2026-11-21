//! The community API's request bodies, turned into domain values at the edge.

use super::community_dto::OverrideDto;
use super::support::{Failure, category_id, role_id};
use melior_domain::{
    CategoryChanges, Channel, ChannelChanges, ChannelDraft, ChannelKind, ChannelMedia,
    InviteSettings, MediaMode, Name, Override, Permissions, RoleChanges, RoleDraft, RoleId, Topic,
};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeSet;

/// More overrides or roles than this in one request are refused rather than stored.
pub(super) const MAX_ITEMS: usize = 250;

/// Tells a field that is `null` (`Some(None)`) from one that is missing (`None`).
// A patch field has three states: missing, null and a value.
#[allow(clippy::option_option)]
fn nullable<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn name(raw: &str) -> Result<Name, Failure> {
    Ok(Name::parse(raw)?)
}

fn overrides(list: Vec<OverrideDto>) -> Result<Vec<Override>, Failure> {
    if list.len() > MAX_ITEMS {
        return Err(Failure::Bad(format!("at most {MAX_ITEMS} overrides")));
    }
    list.into_iter().map(OverrideDto::into_override).collect()
}

fn media_mode(raw: &str) -> Result<MediaMode, Failure> {
    MediaMode::parse(raw).ok_or_else(|| Failure::Bad(format!("unknown media mode {raw:?}")))
}

/// `{ "name": ... }`.
#[derive(Debug, Deserialize)]
pub(super) struct NameDto {
    pub(super) name: String,
}

/// `{ "role_ids": [...] }`.
#[derive(Debug, Deserialize)]
pub(super) struct MemberRolesDto {
    role_ids: Vec<String>,
}

impl MemberRolesDto {
    pub(super) fn into_ids(self) -> Result<BTreeSet<RoleId>, Failure> {
        if self.role_ids.len() > MAX_ITEMS {
            return Err(Failure::Bad(format!("at most {MAX_ITEMS} roles")));
        }
        self.role_ids.iter().map(|id| role_id(id)).collect()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateRoleDto {
    name: String,
    #[serde(default)]
    color: Option<u32>,
    #[serde(default)]
    permissions: Vec<String>,
}

impl CreateRoleDto {
    pub(super) fn into_draft(self) -> Result<RoleDraft, Failure> {
        Ok(RoleDraft {
            name: name(&self.name)?,
            color: self.color,
            permissions: Permissions::from_names(&self.permissions)?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EditRoleDto {
    #[serde(default)]
    name: Option<String>,
    #[allow(clippy::option_option)] // missing, null or a value
    #[serde(default, deserialize_with = "nullable")]
    color: Option<Option<u32>>,
    #[serde(default)]
    permissions: Option<Vec<String>>,
    #[serde(default)]
    position: Option<u32>,
}

impl EditRoleDto {
    pub(super) fn into_changes(self) -> Result<RoleChanges, Failure> {
        Ok(RoleChanges {
            name: self.name.as_deref().map(name).transpose()?,
            color: self.color,
            permissions: self.permissions.map(Permissions::from_names).transpose()?,
            position: self.position,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EditCategoryDto {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    position: Option<u32>,
    #[serde(default)]
    overrides: Option<Vec<OverrideDto>>,
}

impl EditCategoryDto {
    pub(super) fn into_changes(self) -> Result<CategoryChanges, Failure> {
        Ok(CategoryChanges {
            name: self.name.as_deref().map(name).transpose()?,
            position: self.position,
            overrides: self.overrides.map(overrides).transpose()?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateChannelDto {
    name: String,
    kind: String,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    category_id: Option<String>,
    #[serde(default)]
    media_mode: Option<String>,
    #[serde(default)]
    user_limit: Option<u32>,
}

impl CreateChannelDto {
    pub(super) fn into_draft(self) -> Result<ChannelDraft, Failure> {
        let kind = ChannelKind::parse(&self.kind)
            .ok_or_else(|| Failure::Bad(format!("unknown channel kind {:?}", self.kind)))?;
        let mode = self.media_mode.as_deref().map(media_mode).transpose()?.unwrap_or_default();
        Ok(ChannelDraft {
            name: name(&self.name)?,
            kind,
            topic: Topic::parse(self.topic.as_deref().unwrap_or_default())?,
            category_id: self.category_id.as_deref().map(category_id).transpose()?,
            media: ChannelMedia { mode, user_limit: Channel::check_user_limit(self.user_limit)? },
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EditChannelDto {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    topic: Option<String>,
    #[allow(clippy::option_option)] // missing, null or a value
    #[serde(default, deserialize_with = "nullable")]
    category_id: Option<Option<String>>,
    #[serde(default)]
    position: Option<u32>,
    #[serde(default)]
    media_mode: Option<String>,
    #[allow(clippy::option_option)] // missing, null or a value
    #[serde(default, deserialize_with = "nullable")]
    user_limit: Option<Option<u32>>,
    #[serde(default)]
    overrides: Option<Vec<OverrideDto>>,
    #[serde(default)]
    synced: Option<bool>,
}

impl EditChannelDto {
    pub(super) fn into_changes(self) -> Result<ChannelChanges, Failure> {
        let category_id = match self.category_id {
            Some(Some(raw)) => Some(Some(category_id(&raw)?)),
            Some(None) => Some(None),
            None => None,
        };
        let user_limit = self.user_limit.map(Channel::check_user_limit).transpose()?;
        Ok(ChannelChanges {
            name: self.name.as_deref().map(name).transpose()?,
            topic: self.topic.as_deref().map(Topic::parse).transpose()?,
            category_id,
            position: self.position,
            media_mode: self.media_mode.as_deref().map(media_mode).transpose()?,
            user_limit,
            overrides: self.overrides.map(overrides).transpose()?,
            synced: self.synced,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateInviteDto {
    #[serde(default)]
    max_age_seconds: Option<u64>,
    #[serde(default)]
    max_uses: Option<u32>,
}

impl CreateInviteDto {
    pub(super) fn into_settings(self) -> Result<InviteSettings, Failure> {
        let settings =
            InviteSettings { max_age_seconds: self.max_age_seconds, max_uses: self.max_uses };
        Ok(settings.check()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melior_domain::{CategoryId, CommunityError, OverrideTarget, UserId};
    use serde_json::{Value, json};

    fn parse<T: serde::de::DeserializeOwned>(value: Value) -> T {
        serde_json::from_value(value).unwrap()
    }

    fn bad<T: std::fmt::Debug>(result: Result<T, Failure>) -> String {
        match result {
            Err(Failure::Bad(reason)) => reason,
            other => panic!("expected a 400, got {other:?}"),
        }
    }

    #[test]
    fn a_role_is_created_from_names_and_unknown_permissions_are_refused() {
        let draft = parse::<CreateRoleDto>(
            json!({ "name": " Mods ", "color": 255, "permissions": ["kick_members", "connect"] }),
        )
        .into_draft()
        .unwrap();
        assert_eq!(draft.name.as_str(), "Mods");
        assert_eq!(draft.color, Some(255));
        assert_eq!(draft.permissions, Permissions::KICK_MEMBERS.union(Permissions::CONNECT));

        let unknown = parse::<CreateRoleDto>(json!({ "name": "x", "permissions": ["fly"] }));
        assert_eq!(
            bad(unknown.into_draft()),
            CommunityError::UnknownPermission("fly".into()).to_string()
        );
    }

    #[test]
    fn a_null_colour_removes_it_and_a_missing_one_keeps_it() {
        let cleared = parse::<EditRoleDto>(json!({ "color": null })).into_changes().unwrap();
        assert_eq!(cleared.color, Some(None));
        let kept = parse::<EditRoleDto>(json!({ "position": 2 })).into_changes().unwrap();
        assert_eq!((kept.color, kept.position), (None, Some(2)));
    }

    #[test]
    fn a_channel_is_created_with_defaults_and_checked_settings() {
        let draft = parse::<CreateChannelDto>(json!({ "name": "Room", "kind": "voice" }))
            .into_draft()
            .unwrap();
        assert_eq!(draft.kind, ChannelKind::Voice);
        assert_eq!(draft.media, ChannelMedia::default());
        assert_eq!(draft.category_id, None);

        let full = parse::<CreateChannelDto>(json!({
            "name": "Room", "kind": "voice", "topic": "t", "media_mode": "peer_to_peer", "user_limit": 4
        }))
        .into_draft()
        .unwrap();
        assert_eq!(full.media, ChannelMedia { mode: MediaMode::PeerToPeer, user_limit: Some(4) });

        for body in [
            json!({ "name": "Room", "kind": "stage" }),
            json!({ "name": "Room", "kind": "voice", "media_mode": "p2p" }),
            json!({ "name": "Room", "kind": "voice", "user_limit": 1 }),
            json!({ "name": "Room", "kind": "voice", "category_id": "nope" }),
            json!({ "name": "", "kind": "text" }),
        ] {
            bad(parse::<CreateChannelDto>(body).into_draft());
        }
    }

    #[test]
    fn editing_a_channel_tells_null_from_missing() {
        let category = CategoryId::generate();
        let moved = parse::<EditChannelDto>(
            json!({ "category_id": category.as_str(), "user_limit": null }),
        )
        .into_changes()
        .unwrap();
        assert_eq!(moved.category_id, Some(Some(category)));
        assert_eq!(moved.user_limit, Some(None));

        let removed =
            parse::<EditChannelDto>(json!({ "category_id": null })).into_changes().unwrap();
        assert_eq!(removed.category_id, Some(None));
        assert_eq!(removed.user_limit, None);
        let untouched = parse::<EditChannelDto>(json!({})).into_changes().unwrap();
        assert_eq!(untouched, ChannelChanges::default());
    }

    #[test]
    fn overrides_name_a_role_or_a_member_and_known_permissions() {
        let role = RoleId::generate();
        let changes = parse::<EditChannelDto>(json!({
            "overrides": [
                { "target": { "kind": "role", "id": role.as_str() }, "allow": ["connect"], "deny": ["view_channel"] },
                { "target": { "kind": "member", "id": "sub-bob" }, "allow": [], "deny": [] }
            ],
            "synced": false
        }))
        .into_changes()
        .unwrap();
        let list = changes.overrides.unwrap();
        assert_eq!(list[0].target, OverrideTarget::Role(role));
        assert_eq!(
            (list[0].allow, list[0].deny),
            (Permissions::CONNECT, Permissions::VIEW_CHANNEL)
        );
        assert_eq!(list[1].target, OverrideTarget::Member(UserId::new("sub-bob")));

        for target in [json!({ "kind": "role", "id": "x" }), json!({ "kind": "member", "id": "" })]
        {
            let body = json!({ "overrides": [{ "target": target }] });
            bad(parse::<EditChannelDto>(body).into_changes());
        }
        assert!(
            serde_json::from_value::<EditChannelDto>(
                json!({ "overrides": [{ "target": { "kind": "team", "id": "x" } }] })
            )
            .is_err()
        );
    }

    #[test]
    fn member_roles_and_invites_are_checked() {
        let role = RoleId::generate();
        let ids =
            parse::<MemberRolesDto>(json!({ "role_ids": [role.as_str()] })).into_ids().unwrap();
        assert_eq!(ids, BTreeSet::from([role]));
        bad(parse::<MemberRolesDto>(json!({ "role_ids": ["nope"] })).into_ids());

        let settings = parse::<CreateInviteDto>(json!({ "max_age_seconds": 3600, "max_uses": 5 }))
            .into_settings()
            .unwrap();
        assert_eq!(settings, InviteSettings { max_age_seconds: Some(3600), max_uses: Some(5) });
        bad(parse::<CreateInviteDto>(json!({ "max_age_seconds": 5 })).into_settings());
        assert_eq!(
            parse::<CreateInviteDto>(json!({})).into_settings().unwrap(),
            InviteSettings::default()
        );
    }
}
