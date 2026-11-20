import { Check, Minus, Plus, Trash2, X } from 'lucide-react';
import { useId, useState } from 'react';
import { Button, Label, NativeSelect } from '@/components/ui';
import {
    addTarget,
    CHANNEL_PERMISSIONS,
    type CommunityView,
    choiceOf,
    type Member,
    type OverrideChoice,
    type OverrideTarget,
    type PermissionOverride,
    removeTarget,
    rolesByRank,
    sameTarget,
    setChoice,
} from '@/features/communities/domain';
import { cn } from '@/lib/utils';
import { PERMISSION_LABELS } from './permission-labels';

type OverrideEditorProps = {
    view: CommunityView;
    /** For member names; may still be loading. */
    members: Member[] | undefined;
    value: PermissionOverride[];
    onChange: (next: PermissionOverride[]) => void;
};

const CHOICES: { choice: OverrideChoice; label: string; icon: typeof Check; active: string }[] = [
    { choice: 'deny', label: 'Deny', icon: X, active: 'bg-destructive text-white' },
    { choice: 'inherit', label: 'Inherit', icon: Minus, active: 'bg-secondary text-foreground' },
    { choice: 'allow', label: 'Allow', icon: Check, active: 'bg-success text-primary-foreground' },
];

const targetKey = (target: OverrideTarget): string => `${target.kind}:${target.id}`;

const targetName = (view: CommunityView, members: Member[] | undefined, target: OverrideTarget): string => {
    if (target.kind === 'role') return view.roles.find((role) => role.id === target.id)?.name ?? 'Deleted role';
    return members?.find((member) => member.userId === target.id)?.username ?? 'Member';
};

/** Every role and member that has no override yet, roles first. */
const addableTargets = (view: CommunityView, members: Member[] | undefined, value: PermissionOverride[]) =>
    [
        ...rolesByRank(view.roles).map((role): OverrideTarget => ({ kind: 'role', id: role.id })),
        ...(members ?? []).map((member): OverrideTarget => ({ kind: 'member', id: member.userId })),
    ].filter((target) => !value.some((entry) => sameTarget(entry.target, target)));

type ChoiceButtonsProps = { label: string; value: OverrideChoice; onChange: (choice: OverrideChoice) => void };

const ChoiceButtons = ({ label, value, onChange }: ChoiceButtonsProps) => (
    <div role="radiogroup" aria-label={label} className="flex overflow-hidden rounded-md border border-border">
        {CHOICES.map(({ choice, label: choiceLabel, icon: Icon, active }) => (
            <button
                key={choice}
                type="button"
                role="radio"
                aria-checked={value === choice}
                aria-label={choiceLabel}
                title={choiceLabel}
                onClick={() => onChange(choice)}
                className={cn(
                    'grid size-8 cursor-pointer place-items-center text-muted-foreground hover:bg-accent',
                    value === choice && active,
                )}
            >
                <Icon className="size-4" />
            </button>
        ))}
    </div>
);

/** Allow, inherit or deny each channel permission, per role or member. */
export const OverrideEditor = ({ view, members, value, onChange }: OverrideEditorProps) => {
    const addId = useId();
    const [selected, setSelected] = useState<OverrideTarget | null>(value[0]?.target ?? null);
    const current = selected ? value.find((entry) => sameTarget(entry.target, selected)) : undefined;
    const addable = addableTargets(view, members, value);

    const add = (key: string) => {
        const target = addable.find((entry) => targetKey(entry) === key);
        if (!target) return;
        onChange(addTarget(value, target));
        setSelected(target);
    };

    return (
        <div className="grid gap-4 sm:grid-cols-[12rem_1fr]">
            <div className="flex flex-col gap-2">
                <ul className="flex list-none flex-col gap-0.5">
                    {value.map((entry) => (
                        <li key={targetKey(entry.target)} className="flex items-center gap-1">
                            <Button
                                type="button"
                                variant={selected && sameTarget(selected, entry.target) ? 'secondary' : 'ghost'}
                                size="sm"
                                className="flex-1 justify-start truncate"
                                onClick={() => setSelected(entry.target)}
                            >
                                {entry.target.kind === 'role' ? '' : '@'}
                                {targetName(view, members, entry.target)}
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="icon-sm"
                                aria-label={`Remove the override of ${targetName(view, members, entry.target)}`}
                                onClick={() => onChange(removeTarget(value, entry.target))}
                            >
                                <Trash2 />
                            </Button>
                        </li>
                    ))}
                </ul>
                <Label htmlFor={addId} className="gap-1 text-xs text-muted-foreground">
                    <Plus className="size-3" /> Add a role or member
                </Label>
                <NativeSelect id={addId} value="" onChange={(event) => add(event.target.value)}>
                    <option value="">Choose…</option>
                    {addable.map((target) => (
                        <option key={targetKey(target)} value={targetKey(target)}>
                            {target.kind === 'role' ? 'Role: ' : 'Member: '}
                            {targetName(view, members, target)}
                        </option>
                    ))}
                </NativeSelect>
            </div>
            {selected && current ? (
                <ul className="flex list-none flex-col divide-y divide-border">
                    {CHANNEL_PERMISSIONS.map((permission) => (
                        <li key={permission} className="flex items-center justify-between gap-3 py-2">
                            <span className="flex flex-col">
                                <span className="text-sm font-medium">{PERMISSION_LABELS[permission].label}</span>
                                <span className="text-xs text-muted-foreground">
                                    {PERMISSION_LABELS[permission].hint}
                                </span>
                            </span>
                            <ChoiceButtons
                                label={PERMISSION_LABELS[permission].label}
                                value={choiceOf(current, permission)}
                                onChange={(choice) => onChange(setChoice(value, selected, permission, choice))}
                            />
                        </li>
                    ))}
                </ul>
            ) : (
                <p className="text-sm text-muted-foreground">
                    No overrides: every role keeps its server-wide permissions here. Add a role or member to change
                    that.
                </p>
            )}
        </div>
    );
};
