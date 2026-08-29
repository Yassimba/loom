# pi-add-dir prototype

> Throwaway prototype: prove `/add-dir` can combine directory autocomplete, external context, and `pi-sandbox` permission prompts.

Try it without installing:

```bash
pi -e ./plugins/pi-add-dir-prototype
```

Then type `/add-dir ../`: sibling directories appear immediately, zoxide-frequent children sort first, and Tab accepts the highlighted segment and immediately opens its children. With no path, global zoxide favorites appear before local directories. The extension asks once for the access level and lifetime: read-only or read+write, for the session, project, or globally. The local `pi-sandbox` bridge passes the choice to `pi-sandbox` without displaying a second prompt.

Use `/dirs` to inspect the context directories added to the current session.

Prototype limits:

- Loads root `AGENTS.md` and `CLAUDE.md`; it does not dynamically register external skills yet.
- Session permissions remain in memory. Project and global permissions use `pi-sandbox`'s normal configuration files.
- Install `@yassimba/pi-sandbox-bridge-prototype` instead of loading `pi-sandbox` separately.
- The prototype installs a custom editor to chain directory completion; it does not compose with another custom-editor extension yet.
