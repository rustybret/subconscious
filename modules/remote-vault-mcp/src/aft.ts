import * as os from "node:os";
import * as path from "node:path";
import * as fsSync from "node:fs";
import { SubcClient, type RouteHandle } from "@cortexkit/subc-client";

export interface AftToolDefinition {
  name: string;
  description: string;
  schema: Record<string, unknown>;
}

export interface AftBridgeConfig {
  connectionFile?: string;
  projectRoot?: string;
}

export class AftBridge {
  private subc: SubcClient | null = null;
  private route: RouteHandle | null = null;
  private connectionFile: string;
  private projectRoot: string;
  private tools: AftToolDefinition[] = [];

  constructor(config: AftBridgeConfig = {}) {
    const uid = typeof process.getuid === "function" ? process.getuid() : 501;
    const tempConnection = path.join(os.tmpdir(), `subc-${uid}.connection.json`);
    const configConnection = path.join(os.homedir(), ".config/cortexkit/subc.connection.json");

    this.connectionFile =
      config.connectionFile ||
      process.env.SUBC_CONNECTION_FILE ||
      (fsSync.existsSync(configConnection) ? configConnection : tempConnection);

    this.projectRoot = config.projectRoot || "/Volumes/Topper2TB/Git/subconscious";
  }

  public async init(): Promise<void> {
    this.subc = await SubcClient.connect({
      connectionFile: this.connectionFile,
    });

    // Discover tools from catalog
    const catalog = await this.subc.catalogList();
    const modules: any[] = (catalog as any).modules || catalog;
    const aftModule = modules.find((m: any) => m.module_id === "aft");
    const toolProviderRole = aftModule?.roles?.find((r: any) => r.role === "tool_provider");

    if (toolProviderRole && Array.isArray(toolProviderRole.tools)) {
      this.tools = toolProviderRole.tools.map((t: any) => ({
        name: t.name,
        description: t.description || `AFT tool: ${t.name}`,
        schema: t.schema || { type: "object", properties: {} },
      }));
    }

    // Open route to AFT
    const identity = {
      harness: "opencode",
      session: "remote-vault-mcp-aft",
      project_root: this.projectRoot,
    };

    const target = {
      kind: "tool_provider" as const,
      module_id: "aft",
    };

    this.route = await this.subc.routeOpen(target, identity);
  }

  public getTools(): AftToolDefinition[] {
    return this.tools;
  }

  public async invokeTool(name: string, args: Record<string, unknown>): Promise<any> {
    if (!this.subc || !this.route) {
      throw new Error("AftBridge not initialized or route not established");
    }

    const payload = {
      name,
      arguments: args,
      preset: "head", // Injects required catalog preset to satisfy AFT 0.64+ invariants
    };

    return await this.subc.request(this.route, payload);
  }

  public async close(): Promise<void> {
    if (this.subc) {
      this.subc.close();
      this.subc = null;
      this.route = null;
    }
  }
}
