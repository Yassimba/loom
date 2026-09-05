---
name: datadog
description: Use Datadog through the pup CLI. Use for logs, traces, APM, services, monitors, metrics, dashboards, SLOs, incidents, synthetics, security, infrastructure, authentication, documentation, or Live Debugger probes.
---

# Datadog

Use `pup` to read and change Datadog. Check the installed command before you run it. The examples in this skill can become outdated.

## Before you start

1. Confirm the Datadog site, environment, service, and time range. Ask when a missing value could select the wrong data.
2. Run `pup auth status`. If the session expired, run `pup auth refresh`. Run `pup auth login` only if refresh fails.
3. Read the file that matches the task. Read more than one file only when the task covers more than one area.

| Task | Read |
| --- | --- |
| Services, traces, latency, errors, sampling, or APM settings | [references/apm.md](references/apm.md) |
| Runtime values or Live Debugger probes | [references/debugger.md](references/debugger.md) |
| Log search, pipelines, archives, exclusions, or log costs | [references/logs.md](references/logs.md) |
| Monitors, alerts, muting, downtimes, or SLO alerts | [references/monitors.md](references/monitors.md) |
| Product behavior, limits, setup, or supported features | [references/docs.md](references/docs.md) |
| Authentication, metrics, dashboards, SLOs, incidents, synthetics, security, infrastructure, or any other Pup command | [references/cli.md](references/cli.md) |

For a command you do not know, run `pup --help` and `pup <group> --help`. Keep searches small by setting a time range, service, environment, and result limit.

## Safety

- You may run read commands after the scope is clear.
- Before you change Datadog, show the exact target and change. Wait for approval.
- Mark a monitor for deletion by default. Delete it only when the user approves permanent deletion.
- Keep Live Debugger probes small and temporary. Capture named values, set a TTL, and delete the probe when you finish.
- Keep credentials out of commands, output, logs, and files. Use Pup OAuth or existing `DD_API_KEY`, `DD_APP_KEY`, and `DD_SITE` environment variables.
- Use Datadog documentation for product limits. Use `pup --help` for command syntax.

## Report the result

State the site and scope, commands run, findings, changes made, and any cleanup that remains.
