//! Ported from calldiff's python.test.ts — expected outputs are verbatim.

use crate::common::expect_callstack_in;
use stackdiff::extract::{build_index, extract_functions};
use stackdiff::infer::infer_entries;

#[test]
fn refactors_calls_into_a_helper_with_if_else() {
    expect_callstack_in(
        r#"
      class PiService:
          @staticmethod
          def create_agent_session(options):
    -         AuthStorage.create()
    -         create_coding_tools()
    +         services = PiService.get_services()
    +         services.boot()
              if not options.session_id:
                  SessionManager.create()
              else:
                  SessionManager.open(options.session_id)

    +     @staticmethod
    +     def get_services():
    +         AuthStorage.create()
    +         create_coding_tools()
    +         return services

      class AuthStorage:
          @staticmethod
          def create():
              pass

      class SessionManager:
          @staticmethod
          def create():
              pass

          @staticmethod
          def open(_id):
              pass

      def create_coding_tools():
          pass
    +
    + services = None
    "#,
        "PiService.create_agent_session",
        "pi.py",
        r#"
      PiService.create_agent_session(options)
    - ├─ AuthStorage.create()
    - ├─ create_coding_tools()
    + ├─ PiService.get_services()
    + │  ├─ AuthStorage.create()
    + │  └─ create_coding_tools()
    + ├─ services.boot()
      ├─ if not options.session_id
         └─ SessionManager.create()
      └─ else
         └─ SessionManager.open(_id)
  "#,
    );
}

#[test]
fn self_method_resolves_to_class_method() {
    expect_callstack_in(
        r#"
      class Runner:
          def start(self):
              self.prepare()
    +         self.validate()
              self.run()

          def prepare(self):
              pass

    +     def validate(self):
    +         pass

          def run(self):
              pass
    "#,
        "Runner.start",
        "runner.py",
        r#"
      Runner.start(self)
      ├─ Runner.prepare(self)
    + ├─ Runner.validate(self)
      └─ Runner.run(self)
  "#,
    );
}

#[test]
fn instantiation_expands_into_init_calls() {
    expect_callstack_in(
        r#"
      class Engine:
          def __init__(self):
              load_config()
    +         validate_config()

      def load_config():
          pass

    + def validate_config():
    +     pass

      def boot():
          Engine()
    "#,
        "boot",
        "engine.py",
        r#"
      boot()
      └─ Engine()
         ├─ load_config()
    +    └─ validate_config()
  "#,
    );
}

#[test]
fn async_def_and_awaited_calls() {
    expect_callstack_in(
        r#"
      async def fetch_all():
          await fetch_one()
    +     await fetch_two()

      async def fetch_one():
          pass

    + async def fetch_two():
    +     pass
    "#,
        "fetch_all",
        "fetch.py",
        r#"
      fetch_all()
      ├─ fetch_one()
    + └─ fetch_two()
  "#,
    );
}

#[test]
fn module_level_lambda_assignment() {
    expect_callstack_in(
        r#"
      handler = lambda event: dispatch(event)

      def dispatch(event):
          log(event)
    +     audit(event)
    "#,
        "handler",
        "handler.py",
        r#"
      handler(event)
      └─ dispatch(event)
         ├─ log()
    +    └─ audit()
  "#,
    );
}

#[test]
fn underscore_prefixed_callables_are_not_inference_roots() {
    let before = build_index(
        extract_functions(
            "svc.py",
            "def run():\n    _helper()\n\ndef _helper():\n    pass",
        )
        .unwrap(),
    );
    let after = build_index(
        extract_functions(
            "svc.py",
            "def run():\n    _helper()\n\ndef _helper():\n    _extra()\n\ndef _extra():\n    pass",
        )
        .unwrap(),
    );
    let entries = infer_entries(&before, &after, &[], 12).unwrap();
    assert_eq!(entries, vec!["run".to_string()]);
}
