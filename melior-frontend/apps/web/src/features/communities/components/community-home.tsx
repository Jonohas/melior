import { Link, useNavigate } from '@tanstack/react-router';
import { Info, Server } from 'lucide-react';
import { type FormEvent, useId, useState } from 'react';
import {
    Alert,
    AlertDescription,
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Input,
    Label,
    Skeleton,
} from '@/components/ui';
import { describeRemoved, inviteCodeFromInput, type RemovedReason } from '@/features/communities/domain';
import { useCommunitiesQuery, useCreateCommunity } from '@/features/communities/hooks';
import { initialsOf } from '@/lib/utils';
import { FormError } from './form-error';
import { NameForm } from './name-form';

const CreateServerCard = () => {
    const navigate = useNavigate();
    const create = useCreateCommunity();
    const onCreate = (name: string) =>
        create.mutate(name, {
            onSuccess: (view) => void navigate({ to: '/c/$communityId', params: { communityId: view.id } }),
        });
    return (
        <Card>
            <CardHeader>
                <CardTitle>Create a server</CardTitle>
                <CardDescription>You own it. It starts with a text and a voice channel.</CardDescription>
            </CardHeader>
            <CardContent>
                <NameForm
                    label="Server name"
                    placeholder="My server"
                    submitLabel="Create"
                    pending={create.isPending}
                    error={create.error?.message ?? null}
                    onSubmit={onCreate}
                />
            </CardContent>
        </Card>
    );
};

const JoinServerCard = () => {
    const id = useId();
    const navigate = useNavigate();
    const [value, setValue] = useState('');
    const [invalid, setInvalid] = useState(false);

    const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        const code = inviteCodeFromInput(value);
        setInvalid(code === null);
        if (code) void navigate({ to: '/invite/$code', params: { code } });
    };

    return (
        <Card>
            <CardHeader>
                <CardTitle>Join a server</CardTitle>
                <CardDescription>Paste an invite link or code someone gave you.</CardDescription>
            </CardHeader>
            <CardContent>
                <form onSubmit={handleSubmit} className="flex flex-col gap-2">
                    <Label htmlFor={id}>Invite link or code</Label>
                    <div className="flex gap-2">
                        <Input
                            id={id}
                            value={value}
                            placeholder="aB3dE5fG7h"
                            onChange={(event) => setValue(event.target.value)}
                            aria-invalid={invalid}
                        />
                        <Button type="submit">Join</Button>
                    </div>
                    <FormError message={invalid ? 'An invite code is 10 letters and digits.' : null} />
                </form>
            </CardContent>
        </Card>
    );
};

const ServerCards = () => {
    const communities = useCommunitiesQuery();
    if (communities.isPending) return <Skeleton className="h-24" />;
    if (communities.error) return <FormError message={communities.error.message} />;
    if (communities.data.length === 0) {
        return <p className="text-sm text-muted-foreground">You are not in any server yet.</p>;
    }
    return (
        <ul className="grid list-none gap-3 sm:grid-cols-2 xl:grid-cols-3">
            {communities.data.map((community) => (
                <li key={community.id}>
                    <Link
                        to="/c/$communityId"
                        params={{ communityId: community.id }}
                        className="flex items-center gap-3 rounded-lg border border-border bg-card p-4 hover:bg-accent"
                    >
                        <span className="grid size-10 place-items-center rounded-2xl bg-primary/15 font-semibold text-primary">
                            {initialsOf(community.name)}
                        </span>
                        <span className="flex min-w-0 flex-col">
                            <span className="truncate font-medium">{community.name}</span>
                            <span className="text-xs text-muted-foreground">Owner: {community.ownerUsername}</span>
                        </span>
                    </Link>
                </li>
            ))}
        </ul>
    );
};

/** Home: the servers you are in, and the ways to get into another one. */
export const CommunityHome = ({ removed }: { removed?: RemovedReason }) => (
    <main className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-5xl flex-col gap-6 p-4 sm:p-6">
            {removed && (
                <Alert role="status">
                    <Info />
                    <AlertDescription>{describeRemoved(removed)}</AlertDescription>
                </Alert>
            )}
            <section className="flex flex-col gap-3" aria-label="Your servers">
                <h2 className="flex items-center gap-2 text-lg font-semibold">
                    <Server className="size-5" aria-hidden="true" /> Your servers
                </h2>
                <ServerCards />
            </section>
            <div className="grid gap-4 lg:grid-cols-2">
                <CreateServerCard />
                <JoinServerCard />
            </div>
        </div>
    </main>
);
