@conformance @must-5.2
Feature: Verdicts respect claim classes
  Voting on a poem, a proof, and a diary entry are three different category
  errors, so the class decides which judgements are admissible.

  Scenario Outline: A verdict on a class that is not truth-apt is refused
    Given a "<class>" publet
    And a verdict annotation targeting it with no aspect
    When I load the store
    Then loading the store fails
    And stderr mentions "not truth-apt"

    Examples:
      | class        |
      | definitional |
      | normative    |
      | expressive   |

  Scenario Outline: A verdict on a provenance-only class must name provenance
    Given a "<class>" publet
    And a verdict annotation targeting it with aspect "effect-size"
    When I load the store
    Then loading the store fails
    And stderr mentions "provenance"

    Examples:
      | class       |
      | attributive |
      | archival    |

  Scenario Outline: Truth-apt classes accept verdicts
    Given a "<class>" publet
    And a verdict annotation targeting it with no aspect
    When I load the store
    Then loading the store succeeds

    Examples:
      | class      |
      | formal     |
      | empirical  |
      | procedural |
