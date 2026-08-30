import {
  calls,
  defineActors,
  defineAnchors,
  defineStores,
} from "virtual:progressive-review-authoring";

export const actors = defineActors({
  shell: { label: "Bootstrap shell", softwareMapPath: "loom.bootstrap" },
  mise: { label: "mise", softwareMapPath: "loom.mise" },
  cli: { label: "Loom CLI", softwareMapPath: "loom.cli" },
  wizard: { label: "Catalog + wizard", softwareMapPath: "loom.cli.wizard" },
  executor: { label: "Plan executor", softwareMapPath: "loom.cli.executor" },
  files: { label: "Local files", softwareMapPath: "loom.files" },
});

export const anchors = defineAnchors({
  installMise: {
    title: "Install mise",
    peek: { file: "install.sh", fromLine: 23, toLine: 30 },
    softwareMapPath: "loom.bootstrap.installMise",
  },
  syncCore: {
    title: "Refresh core selection",
    peek: { file: "install.sh", fromLine: 33, toLine: 43 },
    softwareMapPath: "loom.bootstrap.syncCore",
  },
  installPins: {
    title: "Install exact pins",
    peek: { file: "install.sh", fromLine: 72, toLine: 76 },
  },
  persistShell: {
    title: "Persist shell activation",
    peek: { file: "install.sh", fromLine: 78, toLine: 90 },
  },
  parseCli: {
    title: "Parse setup command",
    peek: { file: "cli/loom/src/main.rs", fromLine: 11, toLine: 26 },
  },
  select: {
    title: "Resolve selectors",
    peek: { file: "cli/loom/src/app.rs", fromLine: 74, toLine: 90 },
  },
  dependencies: {
    title: "Expand skill dependencies",
    peek: { file: "cli/loom/src/app.rs", fromLine: 101, toLine: 115 },
  },
  catalog: {
    title: "Load embedded catalog",
    peek: { file: "cli/loom/src/catalog.rs", fromLine: 73, toLine: 82 },
  },
  node: {
    title: "Detect Node prerequisite",
    peek: { file: "cli/loom/src/install.rs", fromLine: 23, toLine: 35 },
  },
});

export const bootstrapMessages = [
  {
    from: actors.shell,
    to: actors.mise,
    label: "Install the bootstrap manager",
    anchor: anchors.installMise,
  },
  {
    from: actors.shell,
    to: actors.files,
    label: "Refresh core tool pins",
    anchor: anchors.syncCore,
  },
  {
    from: actors.shell,
    to: actors.mise,
    label: "Install exact selection",
    anchor: anchors.installPins,
  },
  {
    from: actors.shell,
    to: actors.files,
    label: "Activate future shells",
    anchor: anchors.persistShell,
  },
  { from: actors.shell, to: actors.cli, label: "Run guided setup", anchor: anchors.parseCli },
  {
    from: actors.cli,
    to: actors.wizard,
    label: "Load catalog and choose resources",
    anchor: anchors.catalog,
  },
  {
    from: actors.wizard,
    to: actors.executor,
    label: "Expand dependencies and build plan",
    anchor: anchors.dependencies,
  },
];

const asyncSelection = calls(anchors.parseCli, anchors.select, "guided setup dispatch");
export const baseStack = [anchors.parseCli, asyncSelection, anchors.dependencies];
export const headStack = [anchors.parseCli, asyncSelection, anchors.dependencies];

export const stores = defineStores({
  miseSelection: {
    kind: "document",
    label: "mise selection",
    dataStoreKind: "fileStore",
    softwareMapPath: "loom.files.miseSelection",
    documents: {
      loomToml: {
        label: "~/.config/mise/conf.d/loom.toml",
        key: "tools",
        schema: {
          selection: {
            core: { node: { type: "string", example: "24.7.0" }, loom: { type: "string" } },
            optional: { type: "array" },
          },
        },
      },
    },
  },
  shellProfile: {
    kind: "document",
    label: "shell profile",
    dataStoreKind: "fileStore",
    softwareMapPath: "loom.files.shellProfile",
    documents: {
      activation: {
        label: "mise activation line",
        schema: { shell: { type: "string" }, command: { type: "string" } },
      },
    },
  },
  installLedger: {
    kind: "relational",
    label: "install ledger",
    dataStoreKind: "database",
    softwareMapPath: "loom.ledger",
    tables: {
      runs: {
        label: "runs",
        schema: {
          id: { type: "text", pk: true, example: "run_01J8W7J4" },
          startedAt: { type: "timestamp", example: "2026-08-30T16:52:39Z" },
          status: { type: "text", example: "planned" },
          selection: {
            type: "json",
            schema: {
              scope: { type: "text", example: "global" },
              resources: { type: "array", example: ["code-diagram", "pi-openai-fast"] },
            },
          },
        },
      },
      installedResources: {
        label: "installed_resources",
        schema: {
          runId: {
            type: "text",
            fk: {
              table: "runs",
              field: "id",
              cardinality: "many-to-one",
              onDelete: "cascade",
            },
          },
          resource: { type: "text", pk: true, example: "code-diagram" },
          manager: { type: "text", example: "skills" },
          status: { type: "text", example: "installed" },
        },
      },
    },
  },
  skillTrees: {
    kind: "document",
    label: "agent skill trees",
    dataStoreKind: "fileStore",
    softwareMapPath: "loom.files.skillTrees",
    documents: {
      skills: {
        label: "SKILL.md destinations",
        key: "agent/scope/name",
        schema: {
          agent: { type: "string" },
          scope: { type: "string" },
          skill: {
            schema: { name: { type: "string" }, markdown: { type: "string" } },
            type: "object",
          },
        },
      },
    },
  },
});
