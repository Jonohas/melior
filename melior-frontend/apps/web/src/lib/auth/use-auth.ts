import { useRouteContext } from '@tanstack/react-router';

/** The signed-in identity from the `_authenticated` layout's route context. */
export const useAuth = () => useRouteContext({ from: '/_authenticated' }).auth;
