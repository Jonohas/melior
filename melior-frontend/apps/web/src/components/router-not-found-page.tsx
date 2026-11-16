import { Link } from '@tanstack/react-router';

export const RouterNotFoundPage = () => {
    return (
        <div className="flex flex-col items-center gap-4 p-8">
            <h1 className="text-heading3">Page not found</h1>
            <Link to="/" className="underline">
                Back to the lobby
            </Link>
        </div>
    );
};
