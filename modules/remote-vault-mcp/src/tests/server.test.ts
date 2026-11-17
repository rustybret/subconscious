import { describe, expect, it, beforeAll, afterAll } from "bun:test";
import { validateBindInterface, authenticateRequest, redactSecret } from "../auth.js";
import { startServer } from "../server.js";

describe("Remote Vault MCP Auth & Binding Unit Tests", () => {
  it("rejects 0.0.0.0 and wildcard binds strictly", () => {
    expect(() => validateBindInterface("0.0.0.0", false)).toThrow(/Refusing to bind/);
    expect(() => validateBindInterface("::", false)).toThrow(/Refusing to bind/);
    expect(() => validateBindInterface("0.0.0.0", true)).toThrow(/Refusing to bind/);
  });

  it("accepts loopback only when explicitly permitted", () => {
    expect(() => validateBindInterface("127.0.0.1", true)).not.toThrow();
    expect(() => validateBindInterface("localhost", true)).not.toThrow();
  });

  it("authenticates valid bearer token and rejects invalid ones", () => {
    const validToken = "test-cluster-secret-0123456789abcdef";

    // Valid
    const mockReqValid = {
      headers: { authorization: `Bearer ${validToken}` },
    } as any;
    expect(authenticateRequest(mockReqValid, validToken)).toBe(true);

    // Missing header
    const mockReqMissing = { headers: {} } as any;
    expect(authenticateRequest(mockReqMissing, validToken)).toBe(false);

    // Wrong token
    const mockReqWrong = {
      headers: { authorization: "Bearer wrong-secret-0123456789abcdef" },
    } as any;
    expect(authenticateRequest(mockReqWrong, validToken)).toBe(false);

    // Malformed format
    const mockReqMalformed = {
      headers: { authorization: `Token ${validToken}` },
    } as any;
    expect(authenticateRequest(mockReqMalformed, validToken)).toBe(false);
  });

  it("redacts sensitive keys and values in logs", () => {
    const input = {
      provider: "anthropic",
      apiKey: "sk-ant-api03-abcdef1234567890",
      user: {
        authorization: "Bearer secret-bearer-token",
        email: "test@example.com",
      },
    };

    const redacted = redactSecret(input) as any;
    expect(redacted.provider).toBe("anthropic");
    expect(redacted.apiKey).toContain("[REDACTED");
    expect(redacted.user.authorization).toContain("[REDACTED");
    expect(redacted.user.email).toBe("test@example.com");
  });
});

describe("Remote Vault MCP Server Integration Tests", () => {
  let serverInstance: Awaited<ReturnType<typeof startServer>> | null = null;
  const testBearer = "integration-test-secret-0123456789abcdef";
  let baseUrl = "";
  let activeSessionId = "";
  beforeAll(async () => {
    // Start test server on loopback with ephemeral port
    serverInstance = await startServer({
      host: "127.0.0.1",
      port: 0,
      bearerToken: testBearer,
      allowLoopback: true,
    });
    baseUrl = `http://127.0.0.1:${serverInstance.port}`;
  });

  afterAll(async () => {
    if (serverInstance) {
      await serverInstance.close();
    }
  });

  it("serves unauthenticated /health endpoint", async () => {
    const res = await fetch(`${baseUrl}/health`);
    expect(res.status).toBe(200);
    const body = (await res.json()) as any;
    expect(body.status).toBe("ok");
    expect(body.host).toBe("127.0.0.1");
  });

  it("rejects unauthenticated requests to /mcp with 401", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "tools/list",
        params: {},
      }),
    });
    expect(res.status).toBe(401);
  });

  it("rejects wrong bearer token with 401", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: "Bearer invalid-token-1234567890abcdef",
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "tools/list",
        params: {},
      }),
    });
    expect(res.status).toBe(401);
  });

  it("accepts valid bearer token and handles initialize over /mcp", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          protocolVersion: "2024-11-05",
          capabilities: {},
          clientInfo: { name: "test-client", version: "1.0.0" },
        },
      }),
    });
    expect(res.status).toBe(200);
    activeSessionId = res.headers.get("mcp-session-id") || "";
    expect(activeSessionId).not.toBe("");
    const text = await res.text();
    expect(text).toContain("cortexkit-vault");
  });

  it("advertises all three tools on tools/list", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "tools/list",
        params: {},
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("list_providers");
    expect(text).toContain("get_provider_token");
    expect(text).toContain("call_api");
  });

  it("executes list_providers and returns active vault providers", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 3,
        method: "tools/call",
        params: {
          name: "list_providers",
          arguments: {},
        },
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("deepseek");
    expect(text).toContain("active");
  });

  it("executes get_provider_token and retrieves active token material", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 4,
        method: "tools/call",
        params: {
          name: "get_provider_token",
          arguments: {
            provider: "deepseek",
          },
        },
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("apikey:deepseek");
    expect(text).toContain("token");
  });

  it("advertises AFT tools on tools/list", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 5,
        method: "tools/list",
        params: {},
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("aft_read");
    expect(text).toContain("aft_status");
    expect(text).toContain("aft_glob");
    expect(text).toContain("aft_search");
  });

  it("executes aft_status over SubC with preset: head", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 6,
        method: "tools/call",
        params: {
          name: "aft_status",
          arguments: {},
        },
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("AFT");
    expect(text).not.toContain("invalid_request");
  });

  it("executes aft_read on README.md over SubC with preset: head", async () => {
    const res = await fetch(`${baseUrl}/mcp`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${testBearer}`,
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        "mcp-session-id": activeSessionId,
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 7,
        method: "tools/call",
        params: {
          name: "aft_read",
          arguments: {
            path: "README.md",
            limit: 3,
          },
        },
      }),
    });
    expect(res.status).toBe(200);
    const text = await res.text();
    expect(text).toContain("subconscious");
    expect(text).not.toContain("invalid_request");
  });
});
