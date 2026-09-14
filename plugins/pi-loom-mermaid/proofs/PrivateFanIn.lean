/-!
Two sources `e` and `t` on distinct rows, each with a private (unmerged) edge
into target `i`. In a left-to-right layered drawing each edge takes its own bus
column `ce ≠ ct`, both left of the target (`ce < g`, `ct < g` where `g` is the
target's left border column), and is drawn as:

  - a horizontal from the source box to its column, on the source's row;
  - a vertical on that column from the source row to row `i`;
  - a horizontal on row `i` from the column to `g`.

A crossing is a vertical of one edge meeting a horizontal of the other:

  - column `cx` lies strictly within the other's source-row horizontal
    (which runs from the box at column `b` (b < both columns) to `cy`), and
    that other's source row lies strictly within `x`'s vertical span; or
  - column `cx` lies strictly within the other's target-row horizontal
    `(cy, g)`, and row `i` lies within `x`'s vertical span — always, since
    the vertical ends at `i` inclusive.

We show a crossing is unavoidable for every choice of rows and columns.
-/

def between (lo x hi : Int) : Prop := (lo < x ∧ x < hi) ∨ (hi < x ∧ x < lo)

/-- Edge from row `s` on column `c` crosses the target-row horizontal of the
edge on column `d`: `c` is strictly between `d` and the target border `g`.
The vertical on `c` reaches row `i`, where that horizontal lives. -/
def crossesTargetRun (c d g : Int) : Prop := d < c ∧ c < g

/-- Edge on column `c` spanning rows `s..i` crosses the source-row
horizontal of the other edge (row `r`, from the box edge `b` to column `d`). -/
def crossesSourceRun (c s i r b d : Int) : Prop :=
  between s r i ∧ between b c d

theorem private_fan_in_crosses
    (e t i ce ct g b : Int)
    (_hrows : e ≠ t) (hcols : ce ≠ ct)
    (hb : b < ce ∧ b < ct) (hg : ce < g ∧ ct < g) :
    crossesTargetRun ce ct g ∨ crossesTargetRun ct ce g ∨
    crossesSourceRun ce e i t b ct ∨ crossesSourceRun ct t i e b ce := by
  unfold crossesTargetRun crossesSourceRun between
  omega
