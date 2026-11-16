export { type AuthClient, auth, createAuthClient } from './auth-client';
export { type AuthContext, loadAuthenticatedContext, recheckSession, type SessionCheck } from './authenticated-context';
export { LOGIN_COMPLETED_COOKIE } from './bff-cookies';
export { type CurrentIdentity, getCurrentIdentity, parseIdentity } from './current-identity';
export {
    ForbiddenError,
    type LoginGateReason,
    type LoginGateResult,
    refusalMessage,
    SessionRejectedError,
    UnauthorizedError,
} from './errors';
export { beginLoginRedirect, buildLoginHref, redirectToLogin } from './login-gate';
export { useAuth } from './use-auth';
