# tach fitness functions

Probed on **tach 0.35.2**. On another version (`tach --version`), probe every rule you use before
you trust this file.

## Baseline

```toml
source_roots = ["src"]
exact = true                          # a declared edge that the code does not use fails
forbid_circular_dependencies = true
layers_explicit_depends_on = true     # without it, exact reports downward edges as unused
ignore_type_checking_imports = false  # the default hides imports under `if TYPE_CHECKING:`
root_module = "forbid"                # all code lives in a declared module
layers = ["entry", "domain", "platform"]

[rules]
unused_ignore_directives = "error"
require_ignore_directive_reasons = "error"
unused_external_dependencies = "error"
local_imports = "error"               # imports inside functions count too

[[modules]]
path = "app.api"                      # facade and composition root
layer = "entry"
depends_on = ["app.billing", "app.ticketing"]

[[modules]]
path = "app.billing"
layer = "domain"
depends_on = ["app.billing.infra"]
depends_on_external = []              # domain logic imports no third-party package

[[modules]]
path = "app.billing.infra"            # billing's adapters
layer = "platform"
depends_on = []
depends_on_external = ["psycopg"]
visibility = ["app.billing"]

[[modules]]
path = "app.kernel"                   # shared kernel
utility = true
depends_on = []

[[interfaces]]
expose = ["open_invoice", "InvoiceId", "contract\\..*"]
from = ["app.billing"]
```

The gate runs both commands. `tach check` skips every `*_external` rule:

```bash
uv run tach check
uv run tach check-external
```

## Rule table

| Design rule | tach feature | Probe shows |
|---|---|---|
| A module uses only its declared edges | `depends_on` + `exact = true` | `Module 'X' cannot depend on 'Y'` |
| Calls go down | `layers` + `layer` | `Layer 'a' ('X') is lower than layer 'b' ('Y')` |
| A layer is the only way through | `{ name = "domain", closed = true }` | config probe: `is a closed layer between them` |
| No cycles | `forbid_circular_dependencies` | config probe: `Circular dependency detected` |
| All code belongs to a module | `root_module = "forbid"` | `Module '<root>' cannot depend on` |
| Only the interface is used | `[[interfaces]] expose` + `from` | `is not part of the public interface for` |
| Only DTOs cross a team seam | expose a `contract` subpackage: `"contract\\..*"` | same, for `X.model.Entity` |
| Only primitives cross a seam | `data_types = "primitive"` | `does not match the expected data type ('primitive')` |
| One privileged caller (one writer) | second `[[interfaces]]` with `visibility` + `exclusive = true` | `is not part of the public interface for` |
| Only listed modules may depend on it | module `visibility` | config probe: `does not match its visibility` |
| An edge that must never exist | `cannot_depend_on` (beats `depends_on`) | `Module 'X' cannot depend on 'Y'` |
| Shared kernel | `utility = true`, `depends_on = []` | kernel importing a domain: `cannot depend on` |
| Plugins never import each other | one glob module: `path = "app.plugins.*"` | `Module 'app.plugins.b' cannot depend on 'app.plugins.a'` |
| Domain logic is free of I/O libraries | `depends_on_external = []`; the driver only in `X.infra` | `does not declare a dependency on external package` |
| A library is banned in a module | `cannot_depend_on_external` | `cannot depend on external package` |
| An edge is scheduled for removal | `{ path = "Y", deprecated = true }` | `[WARN] ... is deprecated` (exit 0) |
| Every exception has a reason | `# tach-ignore(<reason>) <name>` | `Ignore directive is missing a reason` / `is unused` |
| A team owns its domain's rules | `tach.domain.toml` + a `CODEOWNERS` line for it | probe the rules inside it |

## Domain files

A `tach.domain.toml` makes its package the `[root]` module. Paths are relative to that package;
`//` starts an absolute path; `<domain_root>` names the package.

```toml
# src/app/ticketing/tach.domain.toml
[root]
layer = "domain"
depends_on = ["assign", "route"]
visibility = ["//app.api"]            # only the facade may depend on ticketing

[[modules]]
path = "assign"
layer = "domain"
depends_on = ["//app.billing"]

[[modules]]
path = "route"
layer = "domain"
depends_on = []
visibility = ["<domain_root>"]        # only ticketing itself may use route

[[interfaces]]
expose = ["open_ticket", "contract\\..*"]
from = ["<domain_root>"]
```

## Gotchas

1. Layers beat `depends_on`: a declared upward edge still fails.
2. A subpackage that is not a declared module (`billing/_rules.py`) falls under the parent's
   interface. A declared submodule (`billing.infra`) needs its own edge from every importer.
3. `visibility`, cycles and closed layers are checked against declared edges, so a planted import
   fails earlier, on the missing edge. Probe them in the config: back up `tach.toml`, add the
   forbidden edge to `depends_on`, run `tach check`, restore the backup. `visibility` lists exact
   module paths, so a sibling in the same domain must be listed too.
4. `expose` and `from` are regular expressions; in a TOML basic string, double the backslash.
5. `data_types = "primitive"` rejects dataclass parameters, returns and constants. For DTOs,
   expose a `contract` subpackage.
6. `exclusive = true` takes the default interface away from the listed consumers.
7. An `unchecked = true` module with `exact = true` has its edges reported as unused; give it no
   `depends_on`.

To shrink an interface, `tach report src/app/billing --usages` lists every outside use.

## Beyond imports: schema ownership

tach sees imports only. Enforce schema ownership in Postgres: one role per module, owning its
schema, and each module's adapter connects with its own role.

```sql
CREATE ROLE billing LOGIN;
CREATE SCHEMA billing AUTHORIZATION billing;
```

```python
import itertools

import psycopg
import pytest

MODULE_SCHEMAS = ["billing", "ticketing"]


@pytest.mark.parametrize(("role", "foreign_schema"), list(itertools.permutations(MODULE_SCHEMAS, 2)))
def test_module_role_cannot_read_foreign_schema(admin_dsn: str, role: str, foreign_schema: str) -> None:
    """Capability: data ownership. Seam: Postgres grants.

    Fails when a module role gets access to a schema that another module owns.
    """
    with psycopg.connect(admin_dsn, autocommit=True) as conn:
        conn.execute(f"SET ROLE {role}")
        with pytest.raises(psycopg.errors.InsufficientPrivilege):
            conn.execute(f"SELECT 1 FROM {foreign_schema}.any_table LIMIT 0")
```

Postgres checks schema access before it looks up the table, so `any_table` need not exist.
`admin_dsn` is the project's superuser fixture.
