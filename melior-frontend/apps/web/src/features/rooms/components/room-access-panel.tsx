import { type FormEvent, useState } from 'react';
import { addMember, type Room, type RoomSettings, removeMember } from '@/features/rooms/domain';

type RoomAccessPanelProps = {
    room: Room;
    saving: boolean;
    /** Error from the last save (for example the server rejecting a username). */
    saveError: string | null;
    onSave: (settings: RoomSettings) => void;
};

/** The owner's (or an admin's) controls: invite-only toggle and the list of invited usernames. */
export const RoomAccessPanel = ({ room, saving, saveError, onSave }: RoomAccessPanelProps) => {
    const [input, setInput] = useState('');
    const [inputError, setInputError] = useState<string | null>(null);

    const handleAdd = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const result = addMember(room.members, input);
        if (!result.ok) {
            setInputError(result.error);
            return;
        }
        setInputError(null);
        setInput('');
        onSave({ restricted: room.restricted, members: result.members });
    };

    return (
        <section className="flex flex-col gap-2 rounded border border-(--border-color) p-3" aria-label="Room access">
            <h2 className="text-heading4">Room access</h2>
            <label className="flex items-center gap-2">
                <input
                    type="checkbox"
                    checked={room.restricted}
                    disabled={saving}
                    onChange={(event) => onSave({ restricted: event.target.checked, members: room.members })}
                />
                Invite only
            </label>
            <ul className="flex flex-col gap-1">
                {room.members.map((member) => (
                    <li key={member} className="flex items-center gap-2">
                        <span>{member}</span>
                        <button
                            type="button"
                            className="underline"
                            disabled={saving}
                            onClick={() =>
                                onSave({ restricted: room.restricted, members: removeMember(room.members, member) })
                            }
                        >
                            Remove
                        </button>
                    </li>
                ))}
                {room.members.length === 0 && <li>No one is invited.</li>}
            </ul>
            <form onSubmit={handleAdd} className="flex gap-2">
                <input
                    type="text"
                    aria-label="Username to invite"
                    value={input}
                    placeholder="username"
                    onChange={(event) => setInput(event.target.value)}
                    aria-invalid={inputError !== null}
                    className="rounded border border-(--border-color) bg-(--input-background-color) px-3 py-1"
                />
                <button type="submit" disabled={saving} className="underline">
                    Invite
                </button>
            </form>
            {(inputError ?? saveError) && (
                <p className="text-red-500" role="alert">
                    {inputError ?? saveError}
                </p>
            )}
        </section>
    );
};
