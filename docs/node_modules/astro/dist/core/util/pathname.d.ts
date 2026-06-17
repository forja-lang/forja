/**
 * Error thrown when multi-level URL encoding is detected in a pathname.
 * This is a distinct error type so callers can handle it specifically
 * (e.g., returning a 400 response) rather than falling back to partial decoding.
 *
 * @deprecated No longer thrown internally — multi-level encoding is now
 * decoded iteratively instead of rejected. Kept for backwards compatibility
 * in case third-party code references the class.
 */
export declare class MultiLevelEncodingError extends Error {
    constructor();
}
/**
 * Decodes a pathname iteratively until stable, collapsing all levels of
 * percent-encoding into a single canonical form. This prevents
 * double/triple encoding from bypassing middleware authorization checks
 * (CVE-2025-66202) — instead of rejecting multi-level encoding, we
 * fully resolve it so middleware always sees the true decoded path.
 *
 * @param pathname - The pathname to decode
 * @returns The fully decoded pathname
 * @throws Error if the pathname contains invalid URL encoding that
 *   cannot be decoded at all (e.g., a bare `%` not followed by hex digits)
 */
export declare function validateAndDecodePathname(pathname: string): string;
