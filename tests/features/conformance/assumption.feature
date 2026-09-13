@conformance @must-10.7 @must-7.4
Feature: Vouching and triage bear on the graph without authority in it
  An assumer stakes standing on another key's output while disclosing
  nothing about who holds it. A triage annotation orders attention and
  decides nothing.

  Scenario: An assumption is not authorship
    Given a key assuming accountability for another
    Then the assumed key remains the author of its own publets

  Scenario: Assumptions are counted, because fronting is visible only in aggregate
    Given a key assuming accountability for three others
    Then the count of assumptions it holds is 3

  Scenario: No linkage between assumer and assumed holder is recorded
    Given a key assuming accountability for another
    Then the assumption records no identity for the assumed key

  Scenario: A triage annotation changes no standing
    Given a claim with a standing
    And a triage annotation calling it redundant
    Then the standing is unchanged

  Scenario: A triage annotation without its disclosure block is not read
    Given a triage annotation with no engine declared
    Then it is not treated as a triage finding
