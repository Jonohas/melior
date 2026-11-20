import { Plus, UserX } from 'lucide-react';
import { useState } from 'react';
import {
    Avatar,
    AvatarFallback,
    Badge,
    Button,
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuLabel,
    DropdownMenuTrigger,
    Skeleton,
} from '@/components/ui';
import { type CommunityView, canKick, type Member, type RoleId, rolesByRank } from '@/features/communities/domain';
import { useMembersQuery, useSetMemberRoles } from '@/features/communities/hooks';
import { initialsOf } from '@/lib/utils';
import { FormError } from './form-error';
import { assignableRoles, KickMemberDialog, RoleChecklist } from './member-actions';
import { RoleDot } from './roles-tab';

type MemberRowProps = {
    view: CommunityView;
    member: Member;
    onSetRoles: (roleIds: RoleId[]) => void;
    onKick: () => void;
};

const MemberRow = ({ view, member, onSetRoles, onKick }: MemberRowProps) => {
    const roles = rolesByRank(view.roles).filter((role) => member.roleIds.includes(role.id));
    const assignable = assignableRoles(view);
    return (
        <li className="flex flex-wrap items-center gap-2 py-2">
            <Avatar className="size-8">
                <AvatarFallback className="bg-primary/15 text-xs text-primary">
                    {initialsOf(member.username)}
                </AvatarFallback>
            </Avatar>
            <span className="text-sm font-medium">
                {member.username}
                {member.userId === view.ownerId && <span className="ml-1 text-xs text-muted-foreground">(owner)</span>}
            </span>
            <span className="flex flex-1 flex-wrap gap-1">
                {roles.map((role) => (
                    <Badge key={role.id} variant="outline">
                        <RoleDot role={role} /> {role.name}
                    </Badge>
                ))}
            </span>
            {assignable.length > 0 && (
                <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                        <Button type="button" variant="ghost" size="icon-sm" aria-label={`Roles of ${member.username}`}>
                            <Plus />
                        </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                        <DropdownMenuLabel>Roles</DropdownMenuLabel>
                        <RoleChecklist view={view} member={member} onSetRoles={onSetRoles} />
                    </DropdownMenuContent>
                </DropdownMenu>
            )}
            {canKick(view, member) && (
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Kick ${member.username}`}
                    onClick={onKick}
                >
                    <UserX className="text-destructive" />
                </Button>
            )}
        </li>
    );
};

/** The members with their roles: assign roles below your rank, kick members below it. */
export const MembersTab = ({ view }: { view: CommunityView }) => {
    const members = useMembersQuery(view.id, true);
    const setRoles = useSetMemberRoles(view.id);
    const [kicking, setKicking] = useState<Member | null>(null);

    if (members.isPending) return <Skeleton className="h-40" />;
    return (
        <div className="flex flex-col gap-2">
            <FormError message={members.error?.message ?? setRoles.error?.message} />
            <ul className="flex max-h-[50vh] list-none flex-col divide-y divide-border overflow-y-auto">
                {(members.data ?? []).map((member) => (
                    <MemberRow
                        key={member.userId}
                        view={view}
                        member={member}
                        onSetRoles={(roleIds) => setRoles.mutate({ userId: member.userId, roleIds })}
                        onKick={() => setKicking(member)}
                    />
                ))}
            </ul>
            <KickMemberDialog view={view} member={kicking} onClose={() => setKicking(null)} />
        </div>
    );
};
