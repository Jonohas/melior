import { type FormEvent, useId, useState } from 'react';
import { Button, Input, Label, Switch } from '@/components/ui';
import {
    checkName,
    colorToHex,
    PERMISSIONS,
    type Permission,
    parseHexColor,
    type Role,
    type RoleChange,
    togglePermission,
} from '@/features/communities/domain';
import { FormError } from './form-error';
import { PERMISSION_LABELS } from './permission-labels';

type RoleEditorProps = {
    role: Role;
    /** `@everyone`: its name and existence are fixed. */
    everyone: boolean;
    /** Below the caller's rank, with `manage_roles`. */
    editable: boolean;
    /** What the caller may grant; the rest cannot be switched on. */
    grantable: readonly Permission[];
    pending: boolean;
    error: string | null;
    onSave: (change: RoleChange) => void;
    onDelete: () => void;
};

const DEFAULT_COLOR = '#99aab5';

type PermissionSwitchesProps = {
    value: Permission[];
    editable: boolean;
    grantable: readonly Permission[];
    onChange: (next: Permission[]) => void;
};

const PermissionSwitches = ({ value, editable, grantable, onChange }: PermissionSwitchesProps) => (
    <ul className="flex list-none flex-col divide-y divide-border">
        {PERMISSIONS.map((permission) => (
            <li key={permission} className="flex items-center justify-between gap-3 py-2">
                <span className="flex flex-col">
                    <span className="text-sm font-medium">{PERMISSION_LABELS[permission].label}</span>
                    <span className="text-xs text-muted-foreground">{PERMISSION_LABELS[permission].hint}</span>
                </span>
                <Switch
                    aria-label={PERMISSION_LABELS[permission].label}
                    checked={value.includes(permission)}
                    disabled={!editable || !grantable.includes(permission)}
                    onCheckedChange={(on) => onChange(togglePermission(value, permission, on))}
                />
            </li>
        ))}
    </ul>
);

/** One role's name, colour and permissions. Changes are sent on Save. */
export const RoleEditor = ({
    role,
    everyone,
    editable,
    grantable,
    pending,
    error,
    onSave,
    onDelete,
}: RoleEditorProps) => {
    const nameId = useId();
    const colorId = useId();
    const [name, setName] = useState(role.name);
    const [color, setColor] = useState(role.color === null ? null : colorToHex(role.color));
    const [permissions, setPermissions] = useState(role.permissions);
    const [invalid, setInvalid] = useState<string | null>(null);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const checked = checkName(name);
        setInvalid(checked.ok ? null : checked.error);
        if (!checked.ok) return;
        const nameChange = everyone ? {} : { name: checked.value };
        onSave({ ...nameChange, color: color === null ? null : parseHexColor(color), permissions });
    };

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <div className="flex gap-3">
                <div className="flex flex-1 flex-col gap-1.5">
                    <Label htmlFor={nameId}>Role name</Label>
                    <Input
                        id={nameId}
                        value={name}
                        disabled={!editable || everyone}
                        onChange={(event) => setName(event.target.value)}
                    />
                </div>
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor={colorId}>Colour</Label>
                    <Input
                        id={colorId}
                        type="color"
                        className="w-16 p-1"
                        value={color ?? DEFAULT_COLOR}
                        disabled={!editable}
                        onChange={(event) => setColor(event.target.value)}
                    />
                </div>
            </div>
            <PermissionSwitches
                value={permissions}
                editable={editable}
                grantable={grantable}
                onChange={setPermissions}
            />
            <FormError message={invalid ?? error} />
            {editable && (
                <div className="flex justify-between gap-2">
                    {!everyone && (
                        <Button type="button" variant="destructive" onClick={onDelete}>
                            Delete role
                        </Button>
                    )}
                    <Button type="submit" className="ml-auto" disabled={pending}>
                        Save role
                    </Button>
                </div>
            )}
        </form>
    );
};
