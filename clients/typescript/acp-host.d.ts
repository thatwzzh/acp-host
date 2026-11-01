export declare class AcpHost {
  constructor(options?: { binary?: string; cwd?: string });
  start(): this;
  ask(text: string, options?: {
    agent?: string;
    cwd?: string;
    onPermission?: (event: { request: string; title: string; options: unknown }) => Promise<string>;
  }): AsyncGenerator<string, void, unknown>;
  close(): void;
}
