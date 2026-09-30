@conformance @must-5.5
Feature: Values say where they were read from
  A `source` evidence entry names the file at a commit, the query, the
  export, or the claim a claim's values came from. A source that is a claim
  is also a dependency, so the chain from a figure back to its inputs is a
  closure anyone can walk.

  Scenario: A file at a commit is an external source, shown as unverified
    Given a workspace
    When I compose a budget claim read from "https://github.com/schryer/business-plan" at revision "abc123" in "bizplan/plan/p0.py"
    Then it succeeds
    And reading it shows "external https://github.com/schryer/business-plan  (unverified)"
    And reading it shows "revision abc123"
    And reading it shows "locator  bizplan/plan/p0.py"

  Scenario: A warehouse table is an external source with its query
    Given a workspace
    When I compose a budget claim read from table "bigquery:sustenaut.finance.bank" by query "SELECT * FROM bank WHERE month = '2026-09'"
    Then it succeeds
    And reading it shows "query    SELECT * FROM bank WHERE month = '2026-09'"

  Scenario: A claim named as a source becomes a dependency
    Given a workspace with a normative claim about grouping
    When I compose a budget claim read from that claim
    Then it succeeds
    And reading it shows "claim    "
    And the claim presupposes that claim

  Scenario: A source identifier held nowhere is refused
    Given a workspace
    When I compose a budget claim read from an identifier nothing here holds
    Then it fails
    And stderr mentions "neither as an object nor"

  Scenario: A source claim missing from depends is refused by the loader
    Given a store holding a claim whose source claim is not in depends
    When I load the store
    Then loading the store fails
    And stderr mentions "must also be listed in `depends`"

  Scenario: A source claim listed in depends is accepted by the loader
    Given a store holding a claim whose source claim is in depends
    When I load the store
    Then loading the store succeeds
