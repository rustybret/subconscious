# Remote Vault MCP over WireGuard: Operator Runbook

Status: **Implemented, deployed, and live-verified.**

This service exposes CortexKit's credential vault (`ck-claustrum`) over private WireGuard mesh networks to in-cluster machines (e.g. `altos-worker-01`, `altos-worker-02`, cloud pods). It allows headless CLI agents (`agy`) and OpenCode instances to consume fresh, centrally-refreshed access tokens or proxy API requests without holding OAuth refresh tokens, preventing OAuth token-reuse collisions.

---

## 1. Architectural Summary

- **Primary Node Custody**: The primary host (`10.44.0.10` / Mac Nimbus) is the **sole custody home** and refresher for all OAuth accounts (Anthropic, OpenAI, Google).
- **Transport**: MCP Streamable HTTP (`POST /mcp`) with backwards-compatible SSE fallback (`GET /sse`), strictly bound to the WireGuard interface IP `10.44.0.10:8765`. Wildcard binds (`0.0.0.0`) are refused at startup.
- **Authentication**: High-entropy 256-bit bearer secret (`Authorization: Bearer <token>`). Unauthenticated or malformed requests receive `401 Unauthorized`.
- **WireGuard Encryption**: Layer-3 kernel encryption over `10.44.0.0/24` ensures zero plaintext exposure on local or remote networks.

```
┌────────────────────────────────────────────────────────┐
│  Primary Node (10.44.0.10)                             │
│                                                        │
│  ck-claustrum (sole custody home & OAuth refresher)    │
│         │ SubC channel 0 (local IPC)                   │
│         ▼                                              │
│  com.cortexkit.vault-mcp (PID 58452, port 8765)        │
└───────────────────────┬────────────────────────────────┘
                        │ WireGuard (10.44.0.0/24)
                        │ HTTP JSON-RPC + Bearer Auth
        ┌───────────────┴───────────────┐
        ▼                               ▼
┌────────────────────────────┐   ┌────────────────────────────┐
│ Cluster Worker 1           │   │ Cluster Worker 2           │
│ agy (.agents/mcp_config)   │   │ OpenCode (opencode.json)   │
└────────────────────────────┘   └────────────────────────────┘
```

---

## 2. Tool Surface

The server exposes three tools:

| Tool | Signature | Purpose |
| :--- | :--- | :--- |
| **`list_providers`** | `list_providers()` | Non-secret inventory of active providers (`anthropic`, `openai`, `deepseek`, etc.). |
| **`get_provider_token`** | `get_provider_token(provider, minTtlSeconds?)` | **Direct Token Dispenser**: Returns short-lived access token and remaining lifetime in seconds. |
| **`call_api`** | `call_api(provider, endpoint, method, payload?, headers?)` | **Zero-Leak Proxy**: Executes authenticated request directly from the primary node. Client never sees the token. |

---

## 3. Worker Configuration

### For `agy` (Antigravity / Gemini CLI)
On any in-cluster worker machine, add to `~/.gemini/config/mcp_config.json` or workspace `.agents/mcp_config.json`:

```json
{
  "mcpServers": {
    "vault": {
      "type": "http",
      "url": "http://10.44.0.10:8765/mcp",
      "headers": {
        "Authorization": "Bearer <VAULT_MCP_BEARER_TOKEN>"
      }
    }
  }
}
```

Or configure via CLI:
```bash
agy mcp add --header "Authorization: Bearer <VAULT_MCP_BEARER_TOKEN>" --type http vault http://10.44.0.10:8765/mcp
```

### For OpenCode
In worker `~/.config/opencode/opencode.json`:
```json
{
  "mcp": {
    "vault": {
      "type": "remote",
      "url": "http://10.44.0.10:8765/mcp",
      "headers": {
        "Authorization": "Bearer <VAULT_MCP_BEARER_TOKEN>"
      }
    }
  }
}
```

---

## 4. Primary Node Configuration & Management

- **Environment File**: `~/.config/cortexkit/vault-mcp.env` (file mode `0600`):
  ```bash
  VAULT_MCP_BIND_HOST=10.44.0.10
  VAULT_MCP_BIND_PORT=8765
  VAULT_MCP_BEARER_TOKEN=<64-hex-bearer-token>
  VAULT_ENROLLMENT_TOKEN_FILE=/Users/brethoffman/.config/cortexkit/remote-vault-mcp-enrollment.json
  SUBC_CONNECTION_FILE=/var/folders/9f/hp9ydfwn7wxcm48907jlpm3c0000gn/T/subc-501.connection.json
  ```
- **LaunchAgent Plist**: `~/Library/LaunchAgents/com.cortexkit.vault-mcp.plist`
- **Logs**:
  - `tail -f ~/.local/state/vault-mcp.log`
  - `tail -f ~/.local/state/vault-mcp.err`
- **Service Control**:
  ```bash
  # Restart service
  launchctl kickstart -k gui/501/com.cortexkit.vault-mcp
  
  # Status
  launchctl list | grep vault-mcp
  ```

---

## 5. Claustrum Permissions & Enrollment

The server runs under its own enrolled consumer principal `enrolled:remote-vault-mcp` with scoped category grants:
- `llm-provider`: `read`, `list` (reaches 14 credentials)
- `web-search`: `read`, `list` (reaches 5 credentials)
- `anthropic-native`: `read`, `list` (reaches `oauth:anthropic`)
- `openai-native`: `read`, `list` (reaches `chatgpt:openai`, `chatgpt:openai:bret-personal`)
- `google-native`: `read`, `list` (reaches `oauth:google`, `oauth:google:bantam`)

Token material is minted locally by Claustrum and never exposes raw refresh tokens or master keys.
