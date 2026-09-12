@plumbing @must-9.2
Feature: Definitional divergence is separated from staleness
  The remedies differ. Divergence calls for two scoped publets, one under
  each definition. Staleness calls for superseding against the head.

  Background:
    Given a store where two claims presuppose terms differently

  Scenario: Two generations of one definition are reported as staleness
    When I run "pub-divergence" on the two claims
    Then a finding of "stale" names the term "heritability"

  Scenario: Two unrelated definitions are reported as divergence
    When I run "pub-divergence" on the two claims
    Then a finding of "divergent" names the term "theory"

  Scenario: The two outcomes are never conflated
    When I run "pub-divergence" on the two claims
    Then no term appears as both "stale" and "divergent"
