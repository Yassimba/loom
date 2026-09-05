# Python typing

Read `requires-python`, the type-checker's target version, and runtime annotation consumers before recommending syntax changes. The oldest supported interpreter must parse the result. `typing_extensions` backports typing objects, not grammar: it cannot enable PEP 695 syntax on Python before 3.12.

## Mechanical candidates

Apply these where the supported runtime, checker, and annotation introspection permit them. Existing project conventions take precedence over a cosmetic migration.

| Existing spelling | Candidate | Compatibility |
| --- | --- | --- |
| `typing.List`, `Dict`, `Tuple`, `Set` | `list[...]`, `dict[...]`, `tuple[...]`, `set[...]` | Built-in generics: Python 3.9+ |
| `Optional[X]`, `Union[X, Y]` | `X \| None`, `X \| Y` | Union operator: Python 3.10+ |
| `TypeVar` plus `Generic[T]` | `class Stack[T]:` or `def first[T](...):` | PEP 695: Python 3.12+; check bounds, constraints, scope, and variance |
| Alias assignment or `TypeAlias` | `type Alias = ...` | Python 3.12+; lazy alias evaluation and runtime identity differ |
| A method returning its own dynamic class | `Self` | `typing` on 3.11+, or a compatible `typing_extensions` version |

A type alias or generic migration is mechanical only after its runtime and checker semantics are shown equivalent. Quoted or postponed annotations on older runtimes need separate compatibility analysis; they are not a blanket permission to upgrade syntax.

## Contract tools

Reach for a tool when a specific contract needs it. Check support in the installed checker and runtime, including the chosen `typing_extensions` version if used.

| Contract | Candidate |
| --- | --- |
| A closed set of accepted values | `Literal` or an enum; `StrEnum` requires Python 3.11+ |
| Known keyword fields | `Unpack[TypedDict]` |
| A decorator forwards its input signature | `ParamSpec` |
| A genuinely heterogeneous variadic tuple | `TypeVarTuple` |
| A predicate narrows a value | `TypeIs` or `TypeGuard`, according to their different narrowing rules |
| Consumers depend on structural behavior | `Protocol` |
| A method deliberately overrides inherited behavior | `@override` |
| All members of a closed union must be handled | Exhaustive handling with `assert_never` |

`TypeIs` narrows both branches and requires a compatible narrowed type; `TypeGuard` narrows the positive branch and supports some incompatible input/output types. Select the predicate's truthful contract rather than treating these as interchangeable spellings.

Keep runtime boundary checks alongside static contracts. Type checking verifies the configured static surface; it does not prove unchanged serialization, reflection, or runtime error behavior.
