@plumbing @must-6.1
Feature: Lineage distinguishes revision from proposal

  Background:
    Given a store containing the Appendix A fixture

  Scenario: An author's own revision extends the authoritative lineage
    When I run "pub-lineage --authoritative" on P1
    Then stdout lists P1 and P4 in that order

  Scenario: A third party's proposal is not the author's revision history
    When I run "pub-lineage --authoritative" on P1
    Then stdout does not list P2

  Scenario: The full lineage includes third-party proposals
    When I run "pub-lineage --full" on P1
    Then stdout lists P2

  Scenario: Heads can be requested alone
    When I run "pub-lineage --authoritative --heads" on P1
    Then stdout is exactly P4

  Scenario: An unknown object is not found
    When I run "pub-lineage" on an object that is not in the store
    Then the exit code is 3
