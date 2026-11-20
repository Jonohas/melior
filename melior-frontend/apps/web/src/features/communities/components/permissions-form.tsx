import type { ReactNode } from 'react';
import { useState } from 'react';
import { Button } from '@/components/ui';
import type { CommunityView, PermissionOverride } from '@/features/communities/domain';
import { useMembersQuery } from '@/features/communities/hooks';
import { FormError } from './form-error';
import { OverrideEditor } from './override-editor';

type PermissionsFormProps = {
    view: CommunityView;
    initial: PermissionOverride[];
    /** Above the editor, e.g. the sync switch. */
    header?: ReactNode;
    pending: boolean;
    error: string | null;
    onSave: (overrides: PermissionOverride[]) => void;
};

/** Edits a list of overrides locally; nothing is sent before Save. */
export const PermissionsForm = ({ view, initial, header, pending, error, onSave }: PermissionsFormProps) => {
    const members = useMembersQuery(view.id, true);
    const [draft, setDraft] = useState(initial);

    return (
        <div className="flex flex-col gap-4">
            {header}
            <OverrideEditor view={view} members={members.data} value={draft} onChange={setDraft} />
            <FormError message={error} />
            <div className="flex justify-end gap-2">
                <Button type="button" variant="outline" disabled={pending} onClick={() => setDraft(initial)}>
                    Reset
                </Button>
                <Button type="button" disabled={pending} onClick={() => onSave(draft)}>
                    Save permissions
                </Button>
            </div>
        </div>
    );
};
