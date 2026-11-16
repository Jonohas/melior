export const NoAccessPage = () => {
    return (
        <div className="flex flex-col items-center gap-4 p-8">
            <h1 className="text-heading3">No access</h1>
            <p>You are signed in, but this account has no Melior role (member or admin). Ask an administrator.</p>
            <a href="/signout" className="underline">
                Sign out
            </a>
        </div>
    );
};
