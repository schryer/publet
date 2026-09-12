@plumbing
Feature: Plumbing commands compose over CID lines
  The composition contract is behaviour, not convention, so it is tested.

  Background:
    Given a store containing the Appendix A fixture

  Scenario: Listing by class feeds the lint command
    When I run the pipeline "pub-ls --class=empirical | pub-lint"
    Then every line of stdout is a JSON object with a "cid" field

  Scenario: Edges read CIDs from stdin
    When I pipe P1 into "pub-edges --kind=supersedes --in"
    Then stdout lists P4

  Scenario: Filtering by type yields only that type
    When I run "pub-ls --type=rel"
    Then every line of stdout is a CID
    And each names an object of type "rel"
