import { ArrowDown, ArrowUp } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui';
import {
    abilitiesOf,
    type CommunityView,
    canEditRole,
    colorToHex,
    grantablePermissions,
    isEveryoneRole,
    moveRole,
    type Role,
    rolesByRank,
} from '@/features/communities/domain';
import { useCreateRole, useDeleteRole, useMoveRole, useUpdateRole } from '@/features/communities/hooks';
import { ConfirmDialog } from './confirm-dialog';
import { FormError } from './form-error';
import { NameForm } from './name-form';
import { RoleEditor } from './role-editor';

export const RoleDot = ({ role }: { role: Pick<Role, 'color'> }) => (
    <span
        className="size-3 shrink-0 rounded-full bg-muted-foreground"
        style={role.color === null ? undefined : { backgroundColor: colorToHex(role.color) }}
        aria-hidden="true"
    />
);

type RoleListItemProps = {
    view: CommunityView;
    role: Role;
    selected: boolean;
    movable: boolean;
    onSelect: () => void;
    onMove: (direction: 'up' | 'down') => void;
};

const RoleListItem = ({ view, role, selected, movable, onSelect, onMove }: RoleListItemProps) => (
    <li className="flex items-center gap-1">
        <Button
            type="button"
            variant={selected ? 'secondary' : 'ghost'}
            size="sm"
            className="flex-1 justify-start"
            onClick={onSelect}
        >
            <RoleDot role={role} />
            <span className="truncate">{role.name}</span>
        </Button>
        {movable && !isEveryoneRole(view, role) && (
            <>
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Move ${role.name} up`}
                    onClick={() => onMove('up')}
                >
                    <ArrowUp />
                </Button>
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={`Move ${role.name} down`}
                    onClick={() => onMove('down')}
                >
                    <ArrowDown />
                </Button>
            </>
        )}
    </li>
);

/** The roles, highest first: create, reorder, edit and delete those below your rank. */
export const RolesTab = ({ view }: { view: CommunityView }) => {
    const create = useCreateRole(view.id);
    const update = useUpdateRole(view.id);
    const move = useMoveRole(view.id);
    const remove = useDeleteRole(view.id);
    const roles = rolesByRank(view.roles);
    const [selectedId, setSelectedId] = useState(roles[0]?.id ?? null);
    const [confirmDelete, setConfirmDelete] = useState(false);
    const selected = roles.find((role) => role.id === selectedId) ?? null;
    const canManage = abilitiesOf(view.me).manageRoles;

    return (
        <div className="grid gap-4 sm:grid-cols-[14rem_1fr]">
            <div className="flex flex-col gap-3">
                <ul className="flex list-none flex-col gap-0.5">
                    {roles.map((role) => (
                        <RoleListItem
                            key={role.id}
                            view={view}
                            role={role}
                            selected={role.id === selectedId}
                            movable={canEditRole(view.me, role)}
                            onSelect={() => setSelectedId(role.id)}
                            onMove={(direction) => move.mutate(moveRole(view.roles, role.id, direction))}
                        />
                    ))}
                </ul>
                <FormError message={move.error?.message} />
                {canManage && (
                    <NameForm
                        key={view.roles.length}
                        label="New role"
                        placeholder="Moderators"
                        submitLabel="Create"
                        pending={create.isPending}
                        error={create.error?.message ?? null}
                        onSubmit={(name) => create.mutate({ name, permissions: [] })}
                    />
                )}
            </div>
            {selected && (
                <div className="min-w-0">
                    <RoleEditor
                        key={`${selected.id}:${selected.position}:${selected.permissions.join()}`}
                        role={selected}
                        everyone={isEveryoneRole(view, selected)}
                        editable={canEditRole(view.me, selected)}
                        grantable={grantablePermissions(view.me)}
                        pending={update.isPending}
                        error={update.error?.message ?? null}
                        onSave={(change) => update.mutate({ roleId: selected.id, change })}
                        onDelete={() => setConfirmDelete(true)}
                    />
                    <ConfirmDialog
                        open={confirmDelete}
                        onOpenChange={setConfirmDelete}
                        title={`Delete ${selected.name}?`}
                        confirmLabel="Delete role"
                        pending={remove.isPending}
                        error={remove.error?.message ?? null}
                        onConfirm={() =>
                            remove.mutate(selected.id, {
                                onSuccess: () => {
                                    setConfirmDelete(false);
                                    setSelectedId(roles[0]?.id ?? null);
                                },
                            })
                        }
                    >
                        Members with this role lose it and its permissions.
                    </ConfirmDialog>
                </div>
            )}
        </div>
    );
};
