import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";

export const SANDBOX_GRANT_EVENT = "pi-sandbox:grant-path";

type Access = "read" | "write";
type Scope = "session" | "project" | "global";

interface SandboxGrantRequest {
  access: Access;
  path: string;
  scope: Scope;
  ctx: ExtensionCommandContext;
  accept(): void;
  resolve(): void;
  reject(error: unknown): void;
}

interface SandboxAllowCommand {
  handler(args: string, ctx: ExtensionCommandContext): Promise<void>;
}

function isGrantRequest(value: unknown): value is SandboxGrantRequest {
  if (typeof value !== "object" || value === null) return false;
  const request = value as Partial<SandboxGrantRequest>;
  return (
    (request.access === "read" || request.access === "write") &&
    (request.scope === "session" || request.scope === "project" || request.scope === "global") &&
    typeof request.path === "string" &&
    request.ctx !== undefined &&
    typeof request.accept === "function" &&
    typeof request.resolve === "function" &&
    typeof request.reject === "function"
  );
}

function permissionContext(
  ctx: ExtensionCommandContext,
  scope: Scope,
  path: string,
): ExtensionCommandContext {
  const ui = new Proxy(ctx.ui, {
    get(target, property, receiver) {
      if (property === "custom") {
        return async () => ({ action: scope, value: path });
      }
      if (property === "notify") return () => undefined;
      const value: unknown = Reflect.get(target, property, receiver);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  return new Proxy(ctx, {
    get(target, property, receiver) {
      return property === "ui" ? ui : Reflect.get(target, property, receiver);
    },
  });
}

export function registerSandboxBridge(
  pi: ExtensionAPI,
  sandboxExtension: (api: ExtensionAPI) => void,
): void {
  let allowCommand: SandboxAllowCommand | undefined;
  const wrappedPi = new Proxy(pi, {
    get(target, property, receiver) {
      if (property !== "registerCommand") return Reflect.get(target, property, receiver);
      return (name: string, command: SandboxAllowCommand) => {
        if (name === "sandbox-allow") allowCommand = command;
        return pi.registerCommand(name, command);
      };
    },
  });

  sandboxExtension(wrappedPi);

  pi.events.on(SANDBOX_GRANT_EVENT, (data) => {
    if (!isGrantRequest(data)) return;
    if (!allowCommand) {
      data.reject(new Error("pi-sandbox did not register /sandbox-allow"));
      return;
    }

    data.accept();
    void allowCommand
      .handler(`${data.access} ${data.path}`, permissionContext(data.ctx, data.scope, data.path))
      .then(data.resolve, data.reject);
  });
}

export default async function piSandboxBridge(pi: ExtensionAPI): Promise<void> {
  const { default: piSandbox } = await import("pi-sandbox/index.ts");
  registerSandboxBridge(pi, piSandbox);
}
