import { Crown, Users, UserX, Volume2, X } from 'lucide-react';
import { useState } from 'react';
import {
    Avatar,
    AvatarFallback,
    Badge,
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
    Skeleton,
} from '@/components/ui';
import {
    type CommunityView,
    canKick,
    colorToHex,
    groupMembers,
    type Member,
    type MemberGroup,
    nameColor,
    type RoleId,
    rolesByRank,
    usersInVoice,
} from '@/features/communities/domain';
import { useCommunityFeed, useMemberList, useMembersQuery, useSetMemberRoles } from '@/features/communities/hooks';
import { cn, initialsOf } from '@/lib/utils';
import { FormError } from './form-error';
import { assignableRoles, KickMemberDialog, RoleChecklist } from './member-actions';
import { RoleDot } from './roles-tab';

/** The channel header's button that shows or hides the member list, as Discord's. */
export const MemberListButton = () => {
    const { open, toggle } = useMemberList();
    const label = open ? 'Hide member list' : 'Show member list';
    return (
        <Button
            type="button"
            size="icon-sm"
            variant={open ? 'secondary' : 'ghost'}
            aria-pressed={open}
            aria-label={label}
            title={label}
            onClick={toggle}
        >
            <Users />
        </Button>
    );
};

type MemberCardProps = {
    view: CommunityView;
    member: Member;
    onSetRoles: (roleIds: RoleId[]) => void;
    onKick: () => void;
};

/** What a click on a member shows: name, roles, and the role and kick actions the caller may use. */
const MemberCard = ({ view, member, onSetRoles, onKick }: MemberCardProps) => {
    const roles = rolesByRank(view.roles).filter((role) => member.roleIds.includes(role.id));
    const mayAssign = assignableRoles(view).length > 0;
    return (
        <DropdownMenuContent side="left" align="start" className="w-60">
            <DropdownMenuLabel className="flex items-center gap-1.5 text-base">
                {member.username}
                {member.userId === view.ownerId && <Crown className="size-4 text-primary" aria-label="Owner" />}
            </DropdownMenuLabel>
            <div className="flex flex-wrap gap-1 px-2 pb-2">
                {roles.length === 0 && <span className="text-xs text-muted-foreground">No roles</span>}
                {roles.map((role) => (
                    <Badge key={role.id} variant="outline">
                        <RoleDot role={role} /> {role.name}
                    </Badge>
                ))}
            </div>
            {mayAssign && (
                <>
                    <DropdownMenuSeparator />
                    <DropdownMenuLabel>Roles</DropdownMenuLabel>
                    <RoleChecklist view={view} member={member} onSetRoles={onSetRoles} />
                </>
            )}
            {canKick(view, member) && (
                <>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem variant="destructive" onSelect={onKick}>
                        <UserX /> Kick {member.username}
                    </DropdownMenuItem>
                </>
            )}
        </DropdownMenuContent>
    );
};

type MemberRowProps = MemberCardProps & { inVoice: boolean };

const MemberRow = ({ view, member, inVoice, onSetRoles, onKick }: MemberRowProps) => {
    const color = nameColor(member, view.roles);
    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <button
                    type="button"
                    className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-left text-sm hover:bg-accent"
                >
                    <Avatar className="size-7">
                        <AvatarFallback className="bg-primary/15 text-[10px] text-primary">
                            {initialsOf(member.username)}
                        </AvatarFallback>
                    </Avatar>
                    <span
                        className={cn('min-w-0 flex-1 truncate', color === null && 'text-muted-foreground')}
                        style={color === null ? undefined : { color: colorToHex(color) }}
                    >
                        {member.username}
                    </span>
                    {member.userId === view.ownerId && (
                        <Crown className="size-3.5 shrink-0 text-primary" aria-label="Owner" />
                    )}
                    {inVoice && <Volume2 className="size-3.5 shrink-0 text-success" aria-label="In a voice channel" />}
                </button>
            </DropdownMenuTrigger>
            <MemberCard view={view} member={member} onSetRoles={onSetRoles} onKick={onKick} />
        </DropdownMenu>
    );
};

const groupTitle = (group: MemberGroup): string => group.role?.name ?? 'Members';

type MemberListPanelProps = { view: CommunityView; onClose: () => void };

/**
 * The server's members, grouped under their highest role, on the right of every channel page. Below the `wide`
 * breakpoint it lies over the channel instead of squeezing it, covering the header's toggle, so it carries its own
 * close button there.
 */
const MemberListPanel = ({ view, onClose }: MemberListPanelProps) => {
    const members = useMembersQuery(view.id, true);
    const { roster } = useCommunityFeed();
    const setRoles = useSetMemberRoles(view.id);
    const [kicking, setKicking] = useState<Member | null>(null);
    const inVoice = usersInVoice(roster);
    const groups = groupMembers(members.data ?? [], view.roles);

    return (
        <aside
            aria-label="Members"
            className="flex w-60 shrink-0 flex-col overflow-y-auto border-l border-border bg-card p-2 max-wide:absolute max-wide:inset-y-0 max-wide:right-0 max-wide:z-30 max-wide:shadow-lg"
        >
            <header className="flex items-center justify-between pl-2 wide:hidden">
                <h2 className="text-sm font-semibold">Members</h2>
                <Button type="button" size="icon-sm" variant="ghost" aria-label="Close member list" onClick={onClose}>
                    <X />
                </Button>
            </header>
            <FormError message={members.error?.message ?? setRoles.error?.message} />
            {members.isPending && <Skeleton className="h-40" />}
            {groups.map((group) => (
                <section key={group.role?.id ?? 'members'} aria-label={groupTitle(group)} className="pt-3">
                    <h2 className="px-2 pb-1 text-xs font-semibold tracking-wide text-muted-foreground uppercase">
                        {groupTitle(group)} — {group.members.length}
                    </h2>
                    <ul className="flex list-none flex-col gap-0.5">
                        {group.members.map((member) => (
                            <li key={member.userId}>
                                <MemberRow
                                    view={view}
                                    member={member}
                                    inVoice={inVoice.has(member.userId)}
                                    onSetRoles={(roleIds) => setRoles.mutate({ userId: member.userId, roleIds })}
                                    onKick={() => setKicking(member)}
                                />
                            </li>
                        ))}
                    </ul>
                </section>
            ))}
            <KickMemberDialog view={view} member={kicking} onClose={() => setKicking(null)} />
        </aside>
    );
};

/**
 * The member list while it is switched on (`MemberListButton`); nothing is loaded while it is off. Where it lies
 * over the channel (below the `wide` breakpoint), a tap beside it closes it.
 */
export const MemberList = ({ view }: { view: CommunityView }) => {
    const { open, toggle } = useMemberList();
    if (!open) return null;
    return (
        <>
            {/* The backdrop is for a tap only: the keyboard has the header's toggle and the panel's close button. */}
            <button
                type="button"
                tabIndex={-1}
                aria-hidden="true"
                className="absolute inset-0 z-20 bg-black/50 wide:hidden"
                onClick={toggle}
            />
            <MemberListPanel view={view} onClose={toggle} />
        </>
    );
};
