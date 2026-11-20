import { DropdownMenuCheckboxItem } from '@/components/ui';
import {
    type CommunityView,
    canAssignRole,
    type Member,
    type RoleId,
    rolesByRank,
} from '@/features/communities/domain';
import { useKickMember } from '@/features/communities/hooks';
import { ConfirmDialog } from './confirm-dialog';

const toggled = (roleIds: RoleId[], roleId: RoleId, on: boolean): RoleId[] =>
    on ? [...roleIds, roleId] : roleIds.filter((id) => id !== roleId);

/** The roles the caller may give or take, below their rank. */
export const assignableRoles = (view: CommunityView) =>
    rolesByRank(view.roles).filter((role) => canAssignRole(view, role));

type RoleChecklistProps = {
    view: CommunityView;
    member: Member;
    onSetRoles: (roleIds: RoleId[]) => void;
};

/** One checkbox per assignable role, for a dropdown menu: checking gives the role, unchecking takes it. */
export const RoleChecklist = ({ view, member, onSetRoles }: RoleChecklistProps) =>
    assignableRoles(view).map((role) => (
        <DropdownMenuCheckboxItem
            key={role.id}
            checked={member.roleIds.includes(role.id)}
            onCheckedChange={(on) => onSetRoles(toggled(member.roleIds, role.id, on))}
        >
            {role.name}
        </DropdownMenuCheckboxItem>
    ));

type KickMemberDialogProps = {
    view: CommunityView;
    /** The member to kick; null keeps the dialog closed. */
    member: Member | null;
    onClose: () => void;
};

/** Asks before kicking a member, and kicks them. */
export const KickMemberDialog = ({ view, member, onClose }: KickMemberDialogProps) => {
    const kick = useKickMember(view.id);
    return (
        <ConfirmDialog
            open={member !== null}
            onOpenChange={(open) => {
                if (!open) onClose();
            }}
            title={`Kick ${member?.username ?? ''}?`}
            confirmLabel="Kick"
            pending={kick.isPending}
            error={kick.error?.message ?? null}
            onConfirm={() => {
                if (member) kick.mutate(member.userId, { onSuccess: onClose });
            }}
        >
            They leave the server at once and can only come back with a new invite.
        </ConfirmDialog>
    );
};
