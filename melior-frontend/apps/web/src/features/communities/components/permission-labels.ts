import type { Permission } from '@/features/communities/domain';

/** How each permission is named and explained in the settings. */
export const PERMISSION_LABELS: Record<Permission, { label: string; hint: string }> = {
    view_channel: { label: 'View channels', hint: 'See the channel and who is in it.' },
    connect: { label: 'Connect', hint: 'Join voice channels.' },
    send_messages: { label: 'Send messages', hint: 'Write in text channels and in a call’s chat.' },
    read_history: { label: 'Read message history', hint: 'See messages sent before joining.' },
    create_invite: { label: 'Create invites', hint: 'Invite people to the server.' },
    manage_channels: { label: 'Manage channels', hint: 'Create, edit and delete channels and categories.' },
    manage_roles: { label: 'Manage roles', hint: 'Create and edit roles below their own and assign them.' },
    kick_members: { label: 'Kick members', hint: 'Remove members ranked below them.' },
    administrator: { label: 'Administrator', hint: 'Every permission, in every channel. Grant with care.' },
};
