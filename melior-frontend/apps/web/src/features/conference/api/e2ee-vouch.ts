import {
    checkClaims,
    type DeviceTrust,
    parseCertificate,
    parseVouchConfig,
    thumbprintInput,
    toBase64Url,
    type VouchConfig,
} from '@/features/conference/domain';
import { sharedDeviceStore } from './device-store';
import { type DeviceKey, sha256 } from './e2ee-crypto';
import { finishRenewal, renewCertificate, storedCertificate } from './e2ee-vouch-login';

/**
 * Keycloak vouches for device keys (see `domain/e2ee-certificate.ts`). This module checks a peer's token against
 * the realm's signing keys, fetched from the realm directly, never through the Melior server. This browser's own
 * token comes from a full-page round trip to the realm (`e2ee-vouch-login.ts`).
 */
export type Voucher = {
    /** Whether a realm is configured: then a device the realm does not vouch for gets no media key. */
    vouching: boolean;
    /** This browser's token, or null when it has none that is still valid (no realm, no round trip yet). */
    ownCertificate: () => Promise<string | null>;
    /**
     * Starts the round trip to the realm when this browser's token is missing or nearly expired, coming back to
     * `returnPath`. Returns true when the page is navigating away, so the caller stops.
     */
    renewIfNeeded: (returnPath: string) => Promise<boolean>;
    /** How far the realm vouches that `device` belongs to `username`. */
    verify: (certificate: string | null, device: Uint8Array, username: string) => Promise<DeviceTrust>;
};

/** Where the app's origin serves the realm settings; nginx writes it from the environment. */
const CONFIG_PATH = '/e2ee.json';

/** The `cnf.jkt` a token bound to `device` carries. */
export const deviceThumbprint = async (device: Uint8Array): Promise<string> =>
    toBase64Url(await sha256(thumbprintInput(device)));

export const loadVouchConfig = async (): Promise<VouchConfig | null> => {
    try {
        const response = await fetch(CONFIG_PATH, { cache: 'no-store' });
        return response.ok ? parseVouchConfig(await response.json()) : null;
    } catch {
        return null;
    }
};

type Jwk = JsonWebKey & { kid?: string; alg?: string };

const VERIFY_ALGORITHMS: Record<
    string,
    { import: RsaHashedImportParams | EcKeyImportParams; verify: AlgorithmIdentifier | EcdsaParams }
> = {
    RS256: {
        import: { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' },
        verify: 'RSASSA-PKCS1-v1_5',
    },
    ES256: { import: { name: 'ECDSA', namedCurve: 'P-256' }, verify: { name: 'ECDSA', hash: 'SHA-256' } },
};

/** The realm's published signing keys, fetched once per page load. */
const createKeySet = (config: VouchConfig) => {
    let keys: Promise<Jwk[]> | null = null;
    const load = async (): Promise<Jwk[]> => {
        const response = await fetch(`${config.issuer}/protocol/openid-connect/certs`);
        const body = (await response.json()) as { keys?: Jwk[] };
        return body.keys ?? [];
    };
    return {
        find: async (kid: string): Promise<Jwk | undefined> => {
            keys ??= load().catch(() => {
                keys = null;
                return [];
            });
            return (await keys).find((key) => key.kid === kid);
        },
    };
};

/** Whether the realm signed `certificate`. */
const signedByRealm = async (
    certificate: NonNullable<ReturnType<typeof parseCertificate>>,
    keySet: ReturnType<typeof createKeySet>,
): Promise<boolean> => {
    const algorithm = VERIFY_ALGORITHMS[certificate.header.alg];
    const jwk = await keySet.find(certificate.header.kid);
    if (!algorithm || !jwk) return false;
    try {
        const key = await crypto.subtle.importKey('jwk', jwk, algorithm.import, false, ['verify']);
        return await crypto.subtle.verify(algorithm.verify, key, certificate.signature, certificate.signedPart);
    } catch {
        return false;
    }
};

const nowSeconds = () => Math.floor(Date.now() / 1000);

const createVerify =
    (config: VouchConfig, keySet: ReturnType<typeof createKeySet>): Voucher['verify'] =>
    async (raw, device, username) => {
        if (raw === null) return 'unverified';
        const certificate = parseCertificate(raw);
        if (!certificate || !(await signedByRealm(certificate, keySet))) return 'invalid';
        const result = checkClaims(certificate.claims, config, await deviceThumbprint(device), nowSeconds());
        if ('problem' in result) return 'invalid';
        return result.username === username ? 'verified' : 'invalid';
    };

const NO_REALM: Voucher = {
    vouching: false,
    ownCertificate: async () => null,
    renewIfNeeded: async () => false,
    verify: async () => 'unchecked',
};

/** The voucher of this page: the configured realm, or none (every device then counts as unverified). */
export const createVoucher = async (device: DeviceKey): Promise<Voucher> => {
    const config = await loadVouchConfig();
    if (!config) return NO_REALM;
    const thumbprint = await deviceThumbprint(device.publicRaw);
    return {
        vouching: true,
        ownCertificate: async () => storedCertificate(thumbprint),
        renewIfNeeded: async (returnPath) => renewCertificate(config, thumbprint, returnPath),
        verify: createVerify(config, createKeySet(config)),
    };
};

let shared: Promise<Voucher> | null = null;

/** The page's one voucher, so the realm's keys and this browser's token are fetched once per page load. */
export const sharedVoucher = (device: DeviceKey): Promise<Voucher> => {
    shared ??= createVoucher(device);
    return shared;
};

/**
 * The end of the realm round trip, on `/e2ee-callback`: keeps this device's token and returns where to go back to.
 */
export const completeDeviceVouching = (search: string): Promise<string> => {
    // Once per page load: the callback's effect may run twice, and the round trip can be finished only once.
    completion ??= (async () => {
        const device = await (await sharedDeviceStore()).deviceKey();
        return finishRenewal(await loadVouchConfig(), device, await deviceThumbprint(device.publicRaw), search);
    })();
    return completion;
};

let completion: Promise<string> | null = null;

/**
 * Before a call starts: renews this device's token with the realm round trip if it is missing or nearly expired.
 * Returns true when the page is navigating away; it comes back to `returnPath`, which opens the call again.
 */
export const renewDeviceCertificate = async (returnPath: string): Promise<boolean> => {
    const device = await (await sharedDeviceStore()).deviceKey();
    return (await sharedVoucher(device)).renewIfNeeded(returnPath);
};
