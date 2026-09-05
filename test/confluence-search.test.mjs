import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { execFile } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = fileURLToPath(new URL("..", import.meta.url));
const script = join(root, "skills", "confluence-export", "scripts", "search.py");
const run = promisify(execFile);
const directory = mkdtempSync(join(tmpdir(), "loom-confluence-search-"));
const configPath = join(directory, "app_data.json");
const token = "test-secret-token";
let server;
let site;
let receivedAuthorization;
let receivedUrl;

before(async () => {
  server = createServer((request, response) => {
    receivedAuthorization = request.headers.authorization;
    receivedUrl = new URL(request.url, site);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        results: [
          {
            id: "203458366",
            title: "General Integration Guidelines",
            space: { key: "DNA" },
            version: { when: "2026-06-02T13:31:01.835Z" },
            _links: { webui: "/spaces/DNA/pages/203458366/General" },
          },
        ],
      }),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  site = `http://127.0.0.1:${address.port}`;
  writeFileSync(
    configPath,
    JSON.stringify({
      auth: {
        confluence: {
          [site]: { username: "user@example.test", api_token: token },
        },
      },
    }),
  );
});

after(async () => {
  await new Promise((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
  rmSync(directory, { recursive: true, force: true });
});

test("Confluence search reuses cme credentials without printing its token", async () => {
  const result = await run(
    "python3",
    [script, 'integration "guidelines"', "--space", "DNA", "--limit", "3"],
    { encoding: "utf8", env: { ...process.env, CME_CONFIG_PATH: configPath } },
  );

  assert.equal(
    receivedAuthorization,
    `Basic ${Buffer.from(`user@example.test:${token}`).toString("base64")}`,
  );
  assert.equal(receivedUrl.pathname, "/wiki/rest/api/content/search");
  assert.equal(receivedUrl.searchParams.get("limit"), "3");
  assert.equal(
    receivedUrl.searchParams.get("cql"),
    'type=page AND text~"integration \\"guidelines\\"" AND space="DNA"',
  );
  assert.deepEqual(JSON.parse(result.stdout), [
    {
      id: "203458366",
      title: "General Integration Guidelines",
      space: "DNA",
      updated: "2026-06-02T13:31:01.835Z",
      url: `${site}/wiki/spaces/DNA/pages/203458366/General`,
    },
  ]);
  assert.equal(result.stdout.includes(token), false);
  assert.equal(result.stderr.includes(token), false);
});
