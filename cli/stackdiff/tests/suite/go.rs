//! Ported from calldiff's go.test.ts — expected outputs are verbatim.

use crate::common::expect_callstack_in;

#[test]
fn refactors_calls_into_a_helper_with_if_else() {
    expect_callstack_in(
        r#"
      package pi

      func CreateAgentSession(options Options) {
    -   AuthStorageCreate()
    -   CreateCodingTools()
    +   services := GetServices()
    +   services.Boot()
        if options.SessionID == "" {
          SessionManagerCreate()
        } else {
          SessionManagerOpen(options.SessionID)
        }
      }

    + func GetServices() Services {
    +   AuthStorageCreate()
    +   CreateCodingTools()
    +   return Services{}
    + }

      func AuthStorageCreate() {}
      func CreateCodingTools() {}
      func SessionManagerCreate() {}
      func SessionManagerOpen(id string) {}

      type Options struct{ SessionID string }
    + type Services struct{}
    + func (s Services) Boot() {}
    "#,
        "CreateAgentSession",
        "pi.go",
        r#"
      CreateAgentSession(options)
    - ├─ AuthStorageCreate()
    - ├─ CreateCodingTools()
    + ├─ GetServices()
    + │  ├─ AuthStorageCreate()
    + │  └─ CreateCodingTools()
    + ├─ services.Boot()
      ├─ if options.SessionID == ""
         └─ SessionManagerCreate()
      └─ else
         └─ SessionManagerOpen(id)
  "#,
    );
}

#[test]
fn receiver_methods_resolve_to_type_method() {
    expect_callstack_in(
        r#"
      package runner

      type Runner struct{}

      func (r *Runner) Start() {
        r.Prepare()
    +   r.Validate()
        r.Run()
      }

      func (r *Runner) Prepare() {}
    + func (r *Runner) Validate() {}
      func (r *Runner) Run() {}
    "#,
        "Runner.Start",
        "runner.go",
        r#"
      Runner.Start()
      ├─ Runner.Prepare()
    + ├─ Runner.Validate()
      └─ Runner.Run()
  "#,
    );
}
