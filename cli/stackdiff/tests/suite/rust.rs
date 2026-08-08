use crate::common::expect_callstack_in;

#[test]
fn impl_methods_resolve_self_calls() {
    expect_callstack_in(
        r#"
      struct Runner;

      impl Runner {
          pub fn start(&self) {
              self.prepare();
    +         self.validate();
              self.run();
          }

          fn prepare(&self) {}
    +     fn validate(&self) {}
          fn run(&self) {}
      }
    "#,
        "Runner.start",
        "runner.rs",
        r#"
      Runner.start(self)
      ├─ Runner.prepare(self)
    + ├─ Runner.validate(self)
      └─ Runner.run(self)
  "#,
    );
}
