//! Ported from calldiff's callstack-diff.test.ts — expected outputs are verbatim.

use crate::common::{expect_callstack, expect_callstack_depth, expect_callstack_in};

#[test]
fn refactors_calls_into_a_helper_preserves_if_else_branch_labels() {
    expect_callstack(
        r#"
      export class PiService {
        static createAgentSession(options: { sessionId?: string }) {
    -     AuthStorage.create();
    -     new ModelRegistry();
    -     createCodingTools();
    +     const services = PiService.getServices();
    +     services.boot();
          if (!options.sessionId) {
            SessionManager.create();
          } else {
            SessionManager.open(options.sessionId);
          }
        }
    +
    +   static getServices() {
    +     SettingsManager.create();
    +     AuthStorage.create();
    +     new ModelRegistry();
    +     createCodingTools();
    +     return { boot() {} };
    +   }
      }

      class AuthStorage {
        static create() {}
      }

      class ModelRegistry {
        constructor() {}
      }

      class SessionManager {
        static create() {}
        static open(_id: string) {}
      }
    +
    + class SettingsManager {
    +   static create() {}
    + }

      function createCodingTools() {}
    "#,
        "PiService.createAgentSession",
        r#"
      PiService.createAgentSession(options)
    - ├─ AuthStorage.create()
    - ├─ new ModelRegistry()
    - ├─ createCodingTools()
    + ├─ PiService.getServices()
    + │  ├─ SettingsManager.create()
    + │  ├─ AuthStorage.create()
    + │  ├─ new ModelRegistry()
    + │  └─ createCodingTools()
    + ├─ services.boot()
      ├─ if (!options.sessionId)
         └─ SessionManager.create()
      └─ else
         └─ SessionManager.open(_id)
  "#,
    );
}

#[test]
fn adds_and_removes_free_function_calls() {
    expect_callstack(
        r#"
      export function boot() {
        loadConfig();
    +   migrate();
        connect();
      }

      function loadConfig() {}
    + function migrate() {}
      function connect() {}
    "#,
        "boot",
        r#"
      boot()
      ├─ loadConfig()
    + ├─ migrate()
      └─ connect()
  "#,
    );
}

#[test]
fn shows_class_method_labels_for_this_method_calls() {
    expect_callstack(
        r#"
      export class Runner {
        start() {
          this.prepare();
    +     this.validate();
          this.run();
        }
        prepare() {}
    +   validate() {}
        run() {}
      }
    "#,
        "Runner.start",
        r#"
      Runner.start()
      ├─ Runner.prepare()
    + ├─ Runner.validate()
      └─ Runner.run()
  "#,
    );
}

#[test]
fn labels_else_if_chains_from_source_text() {
    expect_callstack(
        r#"
      export function handle(status: string) {
        if (status === "a") {
          doA();
        } else if (status === "b") {
          doB();
    +     doExtra();
        } else {
          doOther();
        }
      }

      function doA() {}
      function doB() {}
    + function doExtra() {}
      function doOther() {}
    "#,
        "handle",
        r#"
      handle(status)
      ├─ if (status === "a")
         └─ doA()
      ├─ else if (status === "b")
         ├─ doB()
    +    └─ doExtra()
      └─ else
         └─ doOther()
  "#,
    );
}

#[test]
fn marks_a_fully_removed_callee_subtree() {
    expect_callstack(
        r#"
      export function main() {
    -   setup();
        work();
      }
    -
    - function setup() {
    -   initDb();
    - }
    -
    - function initDb() {}
      function work() {}
    "#,
        "main",
        r#"
      main()
    - ├─ setup()
    - │  └─ initDb()
      └─ work()
  "#,
    );
}

#[test]
fn resolves_optional_chaining_as_a_normal_call() {
    expect_callstack(
        r#"
      export function boot(svc?: { start(): void }) {
        svc?.start();
    +   foo?.bar();
      }
    "#,
        "boot",
        r#"
      boot(svc)
      ├─ svc.start()
    + └─ foo.bar()
  "#,
    );
}

#[test]
fn indexes_and_expands_private_methods() {
    expect_callstack(
        r#"
      export class Vault {
        open() {
          this.#unlock();
        }
        #unlock() {
          prep();
    +     audit();
        }
      }
      function prep() {}
    + function audit() {}
    "#,
        "Vault.open",
        r#"
      Vault.open()
      └─ Vault.#unlock()
         ├─ prep()
    +    └─ audit()
  "#,
    );
}

#[test]
fn follows_class_field_arrow_functions() {
    expect_callstack(
        r#"
      export class Runner {
        start() {
          this.helper();
        }
        helper = () => {
          work();
    +     extra();
        };
      }
      function work() {}
    + function extra() {}
    "#,
        "Runner.start",
        r#"
      Runner.start()
      └─ Runner.helper()
         ├─ work()
    +    └─ extra()
  "#,
    );
}

#[test]
fn does_not_attribute_nested_function_bodies_to_the_caller() {
    expect_callstack(
        r#"
      export function outer() {
        function inner() {
          hidden();
        }
        const f = () => {
          alsoHidden();
        };
        visible();
    +   alsoVisible();
      }
      function hidden() {}
      function alsoHidden() {}
      function visible() {}
    + function alsoVisible() {}
    "#,
        "outer",
        r#"
      outer()
      ├─ visible()
    + └─ alsoVisible()
  "#,
    );
}

#[test]
fn treats_tagged_templates_as_calls() {
    expect_callstack(
        r#"
      export function boot() {
        css`color: red`;
    +   html`<div/>`;
        work();
      }
      function css(_s: TemplateStringsArray) {}
    + function html(_s: TemplateStringsArray) {}
      function work() {}
    "#,
        "boot",
        r#"
      boot()
      ├─ css(_s)
    + ├─ html(_s)
      └─ work()
  "#,
    );
}

#[test]
fn extracts_methods_on_abstract_classes() {
    expect_callstack(
        r#"
      export abstract class Service {
        abstract prep(): void;
        start() {
          this.prep();
    +     finish();
        }
      }
    + function finish() {}
    "#,
        "Service.start",
        r#"
      Service.start()
      ├─ Service.prep()
    + └─ finish()
  "#,
    );
}

#[test]
fn expands_new_class_through_the_constructor() {
    expect_callstack(
        r#"
      export function make() {
        new Thing();
      }
      class Thing {
        constructor() {
          init();
    +     ready();
        }
      }
      function init() {}
    + function ready() {}
    "#,
        "make",
        r#"
      make()
      └─ new Thing()
         ├─ init()
    +    └─ ready()
  "#,
    );
}

#[test]
fn follows_const_arrow_function_declarations() {
    expect_callstack(
        r#"
      export const boot = () => {
        load();
    +   migrate();
      };
      function load() {}
    + function migrate() {}
    "#,
        "boot",
        r#"
      boot()
      ├─ load()
    + └─ migrate()
  "#,
    );
}

#[test]
fn names_anonymous_default_exports_as_default() {
    expect_callstack(
        r#"
      export default function () {
        work();
    +   extra();
      }
      function work() {}
    + function extra() {}
    "#,
        "default",
        r#"
      default()
      ├─ work()
    + └─ extra()
  "#,
    );
}

#[test]
fn extracts_generator_function_bodies() {
    expect_callstack(
        r#"
      export function* gen() {
        yield work();
    +   yield extra();
        done();
      }
      function work() { return 1; }
    + function extra() { return 2; }
      function done() {}
    "#,
        "gen",
        r#"
      gen()
      ├─ work()
    + ├─ extra()
      └─ done()
  "#,
    );
}

#[test]
fn indexes_getters_and_walks_their_bodies() {
    expect_callstack(
        r#"
      export class Config {
        get value() {
          load();
    +     validate();
          return 1;
        }
      }
      function load() {}
    + function validate() {}
    "#,
        "Config.value",
        r#"
      Config.value()
      ├─ load()
    + └─ validate()
  "#,
    );
}

#[test]
fn labels_super_method_as_class_method_without_linking_base() {
    // super.setup() is keyed as Child.setup (current class), so Base.setup is not expanded.
    expect_callstack(
        r#"
      class Base {
        setup() {
          prep();
        }
      }
      export class Child extends Base {
        start() {
          super.setup();
    +     work();
        }
      }
      function prep() {}
    + function work() {}
    "#,
        "Child.start",
        r#"
      Child.start()
      ├─ Child.setup()
    + └─ work()
  "#,
    );
}

#[test]
fn collects_calls_inside_try_catch_finally_and_loops() {
    expect_callstack(
        r#"
      export function boot(items: string[]) {
        try {
          open();
        } catch {
          recover();
        } finally {
          close();
        }
        for (const item of items) {
          visit(item);
        }
    +   while (pending()) {
    +     flush();
    +   }
      }
      function open() {}
      function recover() {}
      function close() {}
      function visit(_item: string) {}
    + function pending() { return false; }
    + function flush() {}
    "#,
        "boot",
        r#"
      boot(items)
      ├─ open()
      ├─ recover()
      ├─ close()
      ├─ visit(_item)
    + ├─ pending()
    + └─ flush()
  "#,
    );
}

#[test]
fn ignores_computed_member_calls() {
    expect_callstack(
        r#"
      export function run(obj: Record<string, Function>, key: string) {
        obj[key]();
        obj.known();
    +   obj.other();
      }
    "#,
        "run",
        r#"
      run(obj, key)
      ├─ obj.known()
    + └─ obj.other()
  "#,
    );
}

#[test]
fn parses_tsx_files_and_tracks_calls_in_component_bodies() {
    // JSX tags are not call_expressions; only explicit calls in the body count.
    expect_callstack_in(
        r#"
      export function App() {
        setup();
    +   track();
        return <Button onClick={handle} />;
      }
      function setup() {}
    + function track() {}
      function handle() {
        click();
      }
      function click() {}
      function Button(_props: { onClick(): void }) {
        return null;
      }
    "#,
        "App",
        "app.tsx",
        r#"
      App()
      ├─ setup()
    + └─ track()
  "#,
    );
}

#[test]
fn marks_recursive_cycles_with_a_turnstile() {
    expect_callstack(
        r#"
      export function a() {
        b();
      }
      function b() {
        a();
    +   c();
      }
    + function c() {}
    "#,
        "a",
        r#"
      a()
      └─ b()
         ├─ a() ⇄
    +    └─ c()
  "#,
    );
}

#[test]
fn truncates_expansion_at_max_depth() {
    // Deeper edits under c() are hidden once maxDepth stops expanding it.
    expect_callstack_depth(
        r#"
      export function a() {
        b();
    +   extra();
      }
      function b() {
        c();
      }
      function c() {
        d();
    +   e();
      }
      function d() {}
    + function e() {}
    + function extra() {}
    "#,
        "a",
        2,
        r#"
      a()
      ├─ b()
      │  └─ c()
    + └─ extra()
  "#,
    );
}

#[test]
fn lcs_aligns_reordered_sibling_calls() {
    expect_callstack(
        r#"
      export function boot() {
    -   first();
        second();
    +   first();
      }
      function first() {}
      function second() {}
    "#,
        "boot",
        r#"
      boot()
    - ├─ first()
      ├─ second()
    + └─ first()
  "#,
    );
}

#[test]
fn shows_a_newly_introduced_callee_subtree_as_added() {
    expect_callstack(
        r#"
      export function main() {
    +   boot();
        work();
      }
    +
    + function boot() {
    +   setup();
    + }
    + function setup() {}
      function work() {}
    "#,
        "main",
        r#"
      main()
    + ├─ boot()
    + │  └─ setup()
      └─ work()
  "#,
    );
}
