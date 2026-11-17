import * as os from "os";
import type { IncomingMessage, ServerResponse } from "http";

export interface AuthConfig {
  expectedBearerToken: string;
  allowedBindHost: string;
}

/**
 * Validates that the requested bind host corresponds to an active local network interface
 * in the trusted WireGuard subnet (10.44.0.0/24), or localhost in test mode.
 * Fails closed if 0.0.0.0 or a non-WireGuard address is configured.
 */
export function validateBindInterface(host: string, allowLoopback = false): void {
  if (host === "0.0.0.0" || host === "::") {
    throw new Error(
      `Refusing to bind Remote Vault MCP server to wildcard interface "${host}". ` +
      `Must bind strictly to a trusted WireGuard interface (e.g. 10.44.0.10).`
    );
  }

  if (allowLoopback && (host === "127.0.0.1" || host === "localhost" || host === "::1")) {
    return;
  }

  const interfaces = os.networkInterfaces();
  let found = false;

  for (const [name, addrs] of Object.entries(interfaces)) {
    if (!addrs) continue;
    for (const addr of addrs) {
      if (addr.address === host) {
        // Enforce 10.44.0.0/24 subnet policy for WireGuard
        if (host.startsWith("10.44.0.")) {
          found = true;
          break;
        } else if (!allowLoopback) {
          throw new Error(
            `Bind host "${host}" on interface "${name}" is not in the trusted WireGuard 10.44.0.0/24 subnet.`
          );
        }
      }
    }
    if (found) break;
  }

  if (!found) {
    throw new Error(
      `Configured bind host "${host}" does not match any active local network interface on this machine.`
    );
  }
}

/**
 * Validates the HTTP Authorization: Bearer <token> header.
 * Returns true if authenticated, false otherwise.
 */
export function authenticateRequest(
  req: IncomingMessage,
  expectedToken: string
): boolean {
  if (!expectedToken || expectedToken.length < 16) {
    return false;
  }

  const authHeader = req.headers["authorization"];
  if (!authHeader) {
    return false;
  }

  const match = authHeader.match(/^Bearer\s+([A-Za-z0-9._~+/-]+=*)$/);
  if (!match) {
    return false;
  }

  const providedToken = match[1];

  // Constant-time comparison to prevent timing attacks
  if (providedToken.length !== expectedToken.length) {
    return false;
  }

  let mismatch = 0;
  for (let i = 0; i < expectedToken.length; i++) {
    mismatch |= providedToken.charCodeAt(i) ^ expectedToken.charCodeAt(i);
  }

  return mismatch === 0;
}

/**
 * Sends a standard 401 Unauthorized response with JSON body.
 */
export function sendUnauthorized(res: ServerResponse): void {
  res.writeHead(401, {
    "Content-Type": "application/json",
    "WWW-Authenticate": 'Bearer realm="RemoteVaultMCP"',
  });
  res.end(
    JSON.stringify({
      jsonrpc: "2.0",
      error: {
        code: -32000,
        message: "Unauthorized: valid cluster bearer token required in Authorization header",
      },
    })
  );
}

/**
 * Sanitizes log output to redact all bearer tokens, credentials, and sensitive headers.
 */
export function redactSecret(value: unknown): unknown {
  if (typeof value === "string") {
    if (value.startsWith("Bearer ") || value.startsWith("sk-") || value.startsWith("gho_")) {
      return `[REDACTED:len=${value.length}]`;
    }
    return value;
  }

  if (Array.isArray(value)) {
    return value.map(redactSecret);
  }

  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) {
      const lower = k.toLowerCase();
      if (
        lower.includes("token") ||
        lower.includes("secret") ||
        lower.includes("authorization") ||
        lower.includes("key") ||
        lower.includes("password")
      ) {
        out[k] = typeof v === "string" ? `[REDACTED:len=${v.length}]` : "[REDACTED]";
      } else {
        out[k] = redactSecret(v);
      }
    }
    return out;
  }

  return value;
}
