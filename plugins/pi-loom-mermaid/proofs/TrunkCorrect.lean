/-!
When may edges share a `●` trunk?

A trunk carrying edge set `M` lets a reader trace any of its sources to any
of its targets. It is *correct* when every such trace is a real edge:

    correct M  ⟺  ∀ s ∈ src M, ∀ t ∈ dst M, (s, t) ∈ M

Edges are pairs of node ids. `M` is given as a predicate on pairs; `src`
and `dst` as predicates derived from it.
-/

def Edge := Int × Int

/-- `s` is a source of some edge in `M`. -/
def src (M : Int → Int → Prop) (s : Int) : Prop := ∃ t, M s t
/-- `t` is a target of some edge in `M`. -/
def dst (M : Int → Int → Prop) (t : Int) : Prop := ∃ s, M s t

def correct (M : Int → Int → Prop) : Prop :=
  ∀ s t, src M s → dst M t → M s t

/-- A fan-out: every edge leaves one node. -/
theorem fanOut_correct (M : Int → Int → Prop) (a : Int)
    (h : ∀ s t, M s t → s = a) : correct M := by
  intro s t ⟨t', hs⟩ ⟨s', ht⟩
  have := h s t' hs
  have := h s' t ht
  subst_vars
  exact ht

/-- A fan-in: every edge enters one node. -/
theorem fanIn_correct (M : Int → Int → Prop) (b : Int)
    (h : ∀ s t, M s t → t = b) : correct M := by
  intro s t ⟨t', hs⟩ ⟨s', ht⟩
  have := h s t' hs
  have := h s' t ht
  subst_vars
  exact hs

/-- A complete biclique `S × T`. -/
theorem biclique_correct (S T : Int → Prop) :
    correct (fun s t => S s ∧ T t) := by
  intro s t ⟨_, hs, _⟩ ⟨_, _, ht⟩
  exact ⟨hs, ht⟩

/-- The merge condition. Two correct trunks `A`, `B` may join iff every
cross pair (a source of one, a target of the other) is a real edge in the
union. This is the one line `mergeShared` has to check. -/
theorem merge_iff (A B : Int → Int → Prop) (hA : correct A) (hB : correct B) :
    correct (fun s t => A s t ∨ B s t) ↔
      (∀ s t, src A s → dst B t → A s t ∨ B s t) ∧
      (∀ s t, src B s → dst A t → A s t ∨ B s t) := by
  constructor
  · intro h
    constructor
    · intro s t ⟨t', hs⟩ ⟨s', ht⟩
      exact h s t ⟨t', Or.inl hs⟩ ⟨s', Or.inr ht⟩
    · intro s t ⟨t', hs⟩ ⟨s', ht⟩
      exact h s t ⟨t', Or.inr hs⟩ ⟨s', Or.inl ht⟩
  · rintro ⟨hAB, hBA⟩ s t ⟨t', hs⟩ ⟨s', ht⟩
    rcases hs with hs | hs <;> rcases ht with ht | ht
    · exact Or.inl (hA s t ⟨t', hs⟩ ⟨s', ht⟩)
    · exact hAB s t ⟨t', hs⟩ ⟨s', ht⟩
    · exact hBA s t ⟨t', hs⟩ ⟨s', ht⟩
    · exact Or.inr (hB s t ⟨t', hs⟩ ⟨s', ht⟩)
