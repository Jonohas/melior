import {
    type CommunityId,
    type Invite,
    type InvitePreview,
    type NewInvite,
    parseInvite,
    parseInvitePreview,
    parseInvites,
    parseJoinResult,
    toInviteBody,
} from '@/features/communities/domain';
import { fetchParsed, segment, send, withId, withJson } from './http';

const communityInvitesUrl = (communityId: CommunityId) => `/api/communities/${segment(communityId)}/invites`;
const inviteUrl = (code: string) => withId('/api/invites', code);

export const createInvite = (communityId: CommunityId, invite: NewInvite): Promise<Invite> =>
    fetchParsed(communityInvitesUrl(communityId), parseInvite, 'invite', withJson('POST', toInviteBody(invite)));

export const listInvites = (communityId: CommunityId): Promise<Invite[]> =>
    fetchParsed(communityInvitesUrl(communityId), parseInvites, 'invite list');

export const deleteInvite = async (code: string): Promise<void> => {
    await send(inviteUrl(code), { method: 'DELETE' });
};

/** What an invite leads to. 404 unknown. */
export const previewInvite = (code: string): Promise<InvitePreview> =>
    fetchParsed(inviteUrl(code), parseInvitePreview, 'invite preview');

/** Joins through the invite and returns the community id. 410 when it is no longer usable. */
export const joinInvite = (code: string): Promise<CommunityId> =>
    fetchParsed(inviteUrl(code), parseJoinResult, 'invite join', { method: 'POST' });
