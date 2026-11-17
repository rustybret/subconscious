import * as http from "node:http";
import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { randomUUID } from "node:crypto";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { SSEServerTransport } from "@modelcontextprotocol/sdk/server/sse.js";
import { z } from "zod";

import { authenticateRequest, sendUnauthorized, validateBindInterface } from "./auth.js";
import { VaultService } from "./claustrum.js";
import { AftBridge } from "./aft.js";

export interface ServerOptions {
  host?: string;
  port?: number;
  bearerToken?: string;
  envFile?: string;
  allowLoopback?: boolean;
  vaultService?: VaultService;
  aftBridge?: AftBridge;
  enableAft?: boolean;
}

function jsonSchemaToZodShape(schema: any): Record<string, z.ZodTypeAny> {
  if (!schema || typeof schema !== "object" || !schema.properties) {
    return {};
  }
  const shape: Record<string, z.ZodTypeAny> = {};
  const required = new Set(Array.isArray(schema.required) ? schema.required : []);

  for (const [key, prop] of Object.entries<any>(schema.properties)) {
    let field: z.ZodTypeAny;
    if (prop.type === "string") field = z.string();
    else if (prop.type === "number" || prop.type === "integer") field = z.number();
    else if (prop.type === "boolean") field = z.boolean();
    else if (prop.type === "array") field = z.array(z.any());
    else field = z.any();

    if (prop.description) field = field.describe(prop.description);
    if (!required.has(key)) field = field.optional();
    shape[key] = field;
  }
  return shape;
}

export async function loadBearerToken(envFile?: string): Promise<string> {
  if (process.env.VAULT_MCP_BEARER_TOKEN) {
    return process.env.VAULT_MCP_BEARER_TOKEN.trim();
  }

  const filePath = envFile || path.join(os.homedir(), ".config/cortexkit/vault-mcp.env");
  try {
    const raw = await fs.readFile(filePath, "utf-8");
    for (const line of raw.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const [k, ...v] = trimmed.split("=");
      if (k.trim() === "VAULT_MCP_BEARER_TOKEN") {
        return v.join("=").trim().replace(/^["']|["']$/g, "");
      }
    }
  } catch (err) {
    throw new Error(
      `Could not load bearer token: VAULT_MCP_BEARER_TOKEN not set and file "${filePath}" unreadable: ${String(err)}`
    );
  }

  throw new Error(`VAULT_MCP_BEARER_TOKEN not found in environment or "${filePath}"`);
}

export function createVaultMcpServer(vault: VaultService, aft?: AftBridge): McpServer {
  const server = new McpServer({
    name: "cortexkit-vault",
    version: "0.1.0",
  });

  // Tool 1: list_providers
  server.tool(
    "list_providers",
    "List active credential providers available in the CortexKit vault (non-secret metadata)",
    {},
    async () => {
      try {
        const providers = await vault.listProviders();
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(providers, null, 2),
            },
          ],
        };
      } catch (err) {
        return {
          isError: true,
          content: [{ type: "text", text: `Failed to list providers: ${String(err)}` }],
        };
      }
    }
  );

  // Tool 2: get_provider_token
  server.tool(
    "get_provider_token",
    "Retrieve an ephemeral access token or API key for a specified provider (e.g. 'anthropic', 'openai', 'google', 'deepseek')",
    {
      provider: z.string().describe("Provider name or alias (e.g. anthropic, openai, google, deepseek, cerebras, perplexity, tavily)"),
      minTtlSeconds: z.number().optional().default(300).describe("Minimum acceptable remaining token lifetime in seconds (default: 300)"),
    },
    async ({ provider, minTtlSeconds }) => {
      try {
        const result = await vault.getProviderToken(provider, minTtlSeconds);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch (err) {
        return {
          isError: true,
          content: [{ type: "text", text: `Error retrieving token for provider "${provider}": ${String(err)}` }],
        };
      }
    }
  );

  // Tool 3: call_api
  server.tool(
    "call_api",
    "Execute an authenticated HTTP API call through the primary node vault without exposing raw credentials to the client",
    {
      provider: z.string().describe("Target provider name (e.g. anthropic, openai, deepseek)"),
      endpoint: z.string().describe("API endpoint path (e.g. /v1/messages or /v1/chat/completions)"),
      method: z.enum(["GET", "POST", "PUT", "PATCH", "DELETE"]).optional().default("GET").describe("HTTP method"),
      payload: z.record(z.string(), z.any()).optional().describe("JSON request body for POST/PUT/PATCH"),
      headers: z.record(z.string(), z.string()).optional().describe("Optional custom headers"),
    },
    async ({ provider, endpoint, method, payload, headers }) => {
      try {
        const response = await vault.callApi(provider, endpoint, method, payload, headers);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(response, null, 2),
            },
          ],
        };
      } catch (err) {
        return {
          isError: true,
          content: [{ type: "text", text: `API call failed: ${String(err)}` }],
        };
      }
    }
  );
  // AFT Tools Bridge
  if (aft) {
    for (const tool of aft.getTools()) {
      const toolName = `aft_${tool.name}`;
      const shape = jsonSchemaToZodShape(tool.schema);
      server.tool(
        toolName,
        tool.description,
        shape,
        async (args: any) => {
          try {
            const reply: any = await aft.invokeTool(tool.name, args || {});
            return {
              isError: Boolean(reply.isError),
              content: reply.content || [{ type: "text", text: JSON.stringify(reply, null, 2) }],
            };
          } catch (err) {
            return {
              isError: true,
              content: [{ type: "text", text: `AFT tool "${tool.name}" failed: ${String(err)}` }],
            };
          }
        }
      );
    }
  }

  return server;
}

export async function startServer(options: ServerOptions = {}): Promise<{
  server: http.Server;
  host: string;
  port: number;
  vaultService: VaultService;
  aftBridge?: AftBridge;
  close: () => Promise<void>;
}> {
  const host = options.host || process.env.VAULT_MCP_BIND_HOST || "10.44.0.10";
  const port = options.port || Number(process.env.VAULT_MCP_BIND_PORT) || 8765;
  const allowLoopback = options.allowLoopback ?? (process.env.VAULT_MCP_ALLOW_LOOPBACK === "true");

  // Validate strict network binding
  validateBindInterface(host, allowLoopback);

  const bearerToken = options.bearerToken || (await loadBearerToken(options.envFile));
  if (!bearerToken || bearerToken.length < 16) {
    throw new Error("Invalid bearer token configuration: token must be at least 16 characters.");
  }

  const vault = options.vaultService || new VaultService();
  await vault.init();

  let aft: AftBridge | undefined = options.aftBridge;
  if (!aft && options.enableAft !== false) {
    try {
      aft = new AftBridge();
      await aft.init();
      console.log(`AFT bridge active: exposed ${aft.getTools().length} tools over SubC with preset: head.`);
    } catch (err) {
      console.warn(`AFT bridge initialization skipped: ${String(err)}`);
      aft = undefined;
    }
  }
  // Map of active Streamable HTTP sessions
  const streamableSessions = new Map<string, { transport: StreamableHTTPServerTransport; server: McpServer }>();

  // Map of active SSE sessions for /sse and /messages
  const sseTransports = new Map<string, SSEServerTransport>();

  const httpServer = http.createServer(async (req, res) => {
    const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);

    // Unauthenticated health endpoint
    if (url.pathname === "/health" && req.method === "GET") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "ok", host, version: "0.1.0" }));
      return;
    }

    // Authenticate all MCP routes
    if (!authenticateRequest(req, bearerToken)) {
      sendUnauthorized(res);
      return;
    }

    // Route 1: Streamable HTTP transport at /mcp
    if (url.pathname === "/mcp" || url.pathname.startsWith("/mcp/")) {
      const sessionId = req.headers["mcp-session-id"] as string | undefined;
      const session = sessionId ? streamableSessions.get(sessionId) : undefined;

      if (session) {
        await session.transport.handleRequest(req, res);
        return;
      }

      // New session
      let instanceServer: McpServer;
      const transport = new StreamableHTTPServerTransport({
        sessionIdGenerator: () => randomUUID(),
        onsessioninitialized: (sid) => {
          streamableSessions.set(sid, { transport, server: instanceServer });
        },
      });

      transport.onclose = () => {
        const sid = transport.sessionId;
        if (sid) streamableSessions.delete(sid);
      };

      instanceServer = createVaultMcpServer(vault, aft);
      await instanceServer.connect(transport);
      await transport.handleRequest(req, res);
      return;
    }

    // Route 2: Legacy SSE at /sse
    if (url.pathname === "/sse" && req.method === "GET") {
      const sseTransport = new SSEServerTransport("/messages", res);
      sseTransports.set(sseTransport.sessionId, sseTransport);
      const sseServer = createVaultMcpServer(vault, aft);
      await sseServer.connect(sseTransport);
      return;
    }
    // Route 3: Legacy SSE message dispatch at /messages
    if (url.pathname === "/messages" && req.method === "POST") {
      const sessionId = url.searchParams.get("sessionId");
      const transport = sessionId ? sseTransports.get(sessionId) : undefined;
      if (!transport) {
        res.writeHead(404, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "Session not found or expired" }));
        return;
      }
      await transport.handlePostMessage(req, res);
      return;
    }

    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "Not found" }));
  });

  await new Promise<void>((resolve, reject) => {
    httpServer.listen(port, host, () => resolve());
    httpServer.on("error", reject);
  });

  const actualAddress = httpServer.address();
  const actualPort = typeof actualAddress === "object" && actualAddress ? actualAddress.port : port;

  return {
    server: httpServer,
    host,
    port: actualPort,
    vaultService: vault,
    aftBridge: aft,
    close: async () => {
      await new Promise<void>((resolve) => httpServer.close(() => resolve()));
      await vault.close();
      if (aft) await aft.close();
    },
  };
}

// Direct execution entrypoint
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
    .then(({ host, port }) => {
      console.log(`Remote Vault MCP Server running strictly on http://${host}:${port}/mcp`);
    })
    .catch((err) => {
      console.error(`Fatal startup failure: ${String(err)}`);
      process.exit(1);
    });
}
