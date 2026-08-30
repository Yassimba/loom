import { defineSoftwareMap } from "@dev.fast/progressive-review/software-map-model";

export default defineSoftwareMap({
  systems: {
    loom: {
      label: "Loom installer",
      description: "Bootstrap scripts and a catalog-driven Rust CLI",
      containers: {
        bootstrap: {
          label: "Bootstrap scripts",
          components: {
            installMise: {
              label: "Install mise",
              codeElements: {
                shell: {
                  label: "mise bootstrap",
                  sourceRanges: [{ file: "install.sh", fromLine: 23, toLine: 30 }],
                },
              },
            },
            syncCore: {
              label: "Sync core pins",
              codeElements: {
                shell: {
                  label: "manifest selection",
                  sourceRanges: [{ file: "install.sh", fromLine: 33, toLine: 43 }],
                },
              },
            },
          },
        },
        cli: {
          label: "Rust CLI",
          components: {
            catalog: {
              label: "Embedded catalog",
              codeElements: {
                load: {
                  label: "Catalog::embedded",
                  sourceRanges: [{ file: "cli/loom/src/catalog.rs", fromLine: 73, toLine: 82 }],
                },
              },
            },
            wizard: { label: "Selection wizard" },
            executor: {
              label: "Planner + executor",
              codeElements: {
                plan: {
                  label: "build plan",
                  sourceRanges: [{ file: "cli/loom/src/app.rs", fromLine: 101, toLine: 115 }],
                },
              },
            },
          },
        },
        mise: { label: "mise runtime" },
      },
      dataStores: {
        files: {
          label: "Persisted local files",
          kind: "fileStore",
          components: {
            miseSelection: { label: "loom.toml selection" },
            shellProfile: { label: "shell activation" },
            skillTrees: { label: "agent skill trees" },
          },
        },
        ledger: {
          label: "Install ledger",
          kind: "database",
          tables: {
            runs: {
              label: "runs",
              schema: {
                id: { type: "text", pk: true, example: "run_01J8W7J4" },
                startedAt: { type: "timestamp" },
                status: { type: "text", example: "planned" },
                selection: { type: "json" },
              },
            },
            installedResources: {
              label: "installed_resources",
              schema: {
                runId: { type: "text", fk: "runs.id" },
                resource: { type: "text", pk: true },
                manager: { type: "text" },
                status: { type: "text", example: "installed" },
              },
            },
          },
        },
      },
      relationships: [
        { kind: "semantic", from: "bootstrap", to: "mise", label: "installs pins through" },
        { kind: "call", from: "bootstrap", to: "cli" },

        { kind: "semantic", from: "cli.wizard", to: "cli.executor", label: "builds selection" },
        {
          kind: "semantic",
          from: "cli.executor",
          to: "files",
          label: "persists configuration and skills",
        },
        {
          kind: "semantic",
          from: "cli.executor",
          to: "ledger",
          label: "records each result",
        },
      ],
    },
  },
  relationships: [
    {
      kind: "semantic",
      from: "loom.cli.catalog.load",
      to: "loom.cli.wizard",
      label: "offers resources",
      sourceRanges: [{ fromLine: 73, toLine: 82 }],
    },
  ],
});
