import * as fs from "node:fs/promises";
import * as fsSync from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { ClaustrumClient, type ServedCredential } from "@cortexkit/claustrum-client";

export interface VaultConfig {
  connectionFile?: string;
  enrollmentTokenFile?: string;
}

export interface TokenResult {
  token: string;
  expiresInSeconds: number | null;
  provider: string;
  accountId?: string;
}

export interface ProviderInfo {
  id: string;
  friendlyName: string;
  state: string;
  providerIds: readonly string[] | string[];
}

export class VaultService {
  private client: ClaustrumClient | null = null;
  private enrollmentToken: string = "";
  private connectionFile: string;
  private enrollmentTokenFile: string;

  private static readonly ALIAS_MAP: Record<string, string> = {
    anthropic: "oauth:anthropic",
    openai: "chatgpt:openai",
    google: "oauth:google",
    deepseek: "apikey:deepseek",
    cerebras: "apikey:cerebras",
    perplexity: "apikey:perplexity",
    tavily: "apikey:tavily",
    openrouter: "apikey:openrouter",
    nvidia: "apikey:nvidia",
    moonshot: "apikey:moonshot",
    huggingface: "apikey:huggingface",
    "opencode-zen": "apikey:opencode-zen",
    exa: "apikey:exa",
  };

  private static readonly API_BASE_URLS: Record<string, string> = {
    "oauth:anthropic": "https://api.anthropic.com",
    "chatgpt:openai": "https://api.openai.com",
    "chatgpt:openai:bret-personal": "https://api.openai.com",
    "apikey:deepseek": "https://api.deepseek.com",
    "apikey:cerebras": "https://api.cerebras.ai",
    "apikey:perplexity": "https://api.perplexity.ai",
    "apikey:tavily": "https://api.tavily.com",
    "apikey:openrouter": "https://openrouter.ai/api",
    "apikey:nvidia": "https://integrate.api.nvidia.com",
    "apikey:moonshot": "https://api.moonshot.cn",
  };

  constructor(config: VaultConfig = {}) {
    const uid = typeof process.getuid === "function" ? process.getuid() : 501;
    const tempConnection = path.join(os.tmpdir(), `subc-${uid}.connection.json`);
    const configConnection = path.join(os.homedir(), ".config/cortexkit/subc.connection.json");

    this.connectionFile =
      config.connectionFile ||
      process.env.SUBC_CONNECTION_FILE ||
      (fsSync.existsSync(configConnection) ? configConnection : tempConnection);

    this.enrollmentTokenFile =
      config.enrollmentTokenFile ||
      process.env.VAULT_ENROLLMENT_TOKEN_FILE ||
      path.join(os.homedir(), ".config/cortexkit/remote-vault-mcp-enrollment.json");
  }

  public async init(): Promise<void> {
    if (process.env.VAULT_ENROLLMENT_TOKEN) {
      this.enrollmentToken = process.env.VAULT_ENROLLMENT_TOKEN.trim();
    } else {
      try {
        const raw = await fs.readFile(this.enrollmentTokenFile, "utf-8");
        const parsed = JSON.parse(raw);
        this.enrollmentToken = parsed.token;
      } catch (err) {
        throw new Error(
          `Failed to read Claustrum enrollment token from "${this.enrollmentTokenFile}": ${String(err)}`
        );
      }
    }

    if (!this.enrollmentToken || this.enrollmentToken.length !== 64) {
      throw new Error(`Invalid enrollment token: must be 64 hex characters.`);
    }

    // Connect to Claustrum daemon over SubC
    this.client = await ClaustrumClient.connect({
      connectionFile: this.connectionFile,
    });
  }

  public async close(): Promise<void> {
    if (this.client) {
      this.client.close();
      this.client = null;
    }
  }

  public resolveCredentialId(aliasOrId: string): string {
    const lower = aliasOrId.toLowerCase().trim();
    return VaultService.ALIAS_MAP[lower] || aliasOrId;
  }

  public async listProviders(): Promise<ProviderInfo[]> {
    if (!this.client) throw new Error("VaultService not initialized");

    const inventory = await this.client.listScoped(this.enrollmentToken);
    const reverseAliases: Record<string, string> = {};
    for (const [alias, id] of Object.entries(VaultService.ALIAS_MAP)) {
      reverseAliases[id] = alias;
    }

    return inventory.rows.map((row) => ({
      id: row.id,
      friendlyName: reverseAliases[row.id] || row.id,
      state: row.state,
      providerIds: row.providerIds || [],
    }));
  }

  public async getProviderToken(
    provider: string,
    minTtlSeconds: number = 300
  ): Promise<TokenResult> {
    if (!this.client) throw new Error("VaultService not initialized");

    const credentialId = this.resolveCredentialId(provider);
    const minTtlMs = minTtlSeconds > 0 ? minTtlSeconds * 1000 : undefined;

    let cred: ServedCredential;
    try {
      cred = await this.client.getScoped({
        credentialId,
        enrollmentToken: this.enrollmentToken,
        minTtlMs,
      });
    } catch (err) {
      throw new Error(`Failed to retrieve token for "${provider}" (${credentialId}): ${String(err)}`);
    }

    let expiresInSeconds: number | null = null;
    if (cred.expiresAtMs !== null && cred.expiresAtMs !== undefined) {
      const now = Date.now();
      expiresInSeconds = Math.max(0, Math.floor((cred.expiresAtMs - now) / 1000));
    }

    return {
      token: cred.material,
      expiresInSeconds,
      provider: credentialId,
      accountId: cred.accountId,
    };
  }

  public async callApi(
    provider: string,
    endpoint: string,
    method: string = "GET",
    payload?: unknown,
    customHeaders: Record<string, string> = {}
  ): Promise<{ status: number; data: unknown }> {
    const credentialId = this.resolveCredentialId(provider);
    const tokenResult = await this.getProviderToken(provider, 60);

    const baseUrl = VaultService.API_BASE_URLS[credentialId];
    if (!baseUrl) {
      throw new Error(`No proxy base URL configured for provider "${provider}" (${credentialId})`);
    }

    const cleanEndpoint = endpoint.startsWith("/") ? endpoint : `/${endpoint}`;
    const url = `${baseUrl}${cleanEndpoint}`;

    const headers: Record<string, string> = {
      ...customHeaders,
      "Content-Type": "application/json",
    };

    if (credentialId.startsWith("oauth:anthropic")) {
      headers["x-api-key"] = tokenResult.token;
      if (!headers["anthropic-version"]) {
        headers["anthropic-version"] = "2023-06-01";
      }
    } else {
      headers["Authorization"] = `Bearer ${tokenResult.token}`;
    }

    const reqInit: RequestInit = {
      method: method.toUpperCase(),
      headers,
    };

    if (payload && (method.toUpperCase() === "POST" || method.toUpperCase() === "PUT" || method.toUpperCase() === "PATCH")) {
      reqInit.body = typeof payload === "string" ? payload : JSON.stringify(payload);
    }

    const res = await fetch(url, reqInit);
    const text = await res.text();
    let data: unknown;
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }

    return {
      status: res.status,
      data,
    };
  }
}
