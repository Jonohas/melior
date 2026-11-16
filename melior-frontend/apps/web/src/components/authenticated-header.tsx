type AuthenticatedHeaderProps = {
    username: string;
};

export const AuthenticatedHeader = ({ username }: AuthenticatedHeaderProps) => {
    return (
        <header className="flex items-center justify-end gap-4 px-4 py-2 border-b border-(--border-color)">
            <span>{username}</span>
            {/* A plain anchor: sign-out is a full-page navigation through the BFF, not a client route. */}
            <a href="/signout" className="underline">
                Sign out
            </a>
        </header>
    );
};
