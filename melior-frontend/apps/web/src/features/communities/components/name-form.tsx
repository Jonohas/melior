import { type FormEvent, useId, useState } from 'react';
import { Button, Input, Label } from '@/components/ui';
import { checkName } from '@/features/communities/domain';
import { FormError } from './form-error';

type NameFormProps = {
    label: string;
    initial?: string;
    placeholder?: string;
    submitLabel: string;
    pending: boolean;
    /** The server's refusal of the last attempt. */
    error: string | null;
    onSubmit: (name: string) => void;
};

/** One name field with the shared name rules (1 to 100 characters), for servers, categories and roles. */
export const NameForm = ({
    label,
    initial = '',
    placeholder,
    submitLabel,
    pending,
    error,
    onSubmit,
}: NameFormProps) => {
    const id = useId();
    const [value, setValue] = useState(initial);
    const [invalid, setInvalid] = useState<string | null>(null);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const checked = checkName(value);
        setInvalid(checked.ok ? null : checked.error);
        if (checked.ok) onSubmit(checked.value);
    };

    return (
        <form onSubmit={handleSubmit} className="flex flex-col gap-2">
            <Label htmlFor={id}>{label}</Label>
            <div className="flex gap-2">
                <Input
                    id={id}
                    value={value}
                    placeholder={placeholder}
                    onChange={(event) => setValue(event.target.value)}
                    aria-invalid={invalid !== null}
                />
                <Button type="submit" disabled={pending}>
                    {submitLabel}
                </Button>
            </div>
            <FormError message={invalid ?? error} />
        </form>
    );
};
