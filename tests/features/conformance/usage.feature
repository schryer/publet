@conformance @must-5.2
Feature: A definition is settled by usage, not by verdict
  Section 5.2 permits no verdict on a definitional publet and settles it by
  usage evidence instead. A citation names where a term is used; it need
  not carry the text, which is what keeps a corpus of definitions from
  becoming a corpus of restatements.

  Scenario: A verdict on a definition is refused
    Given a workspace with a composed definition
    When I record a verdict on it
    Then it fails
    And stderr mentions "not truth-apt"

  Scenario: Usage citations are recorded and shown
    Given a workspace with a composed definition
    When I cite two sources using it
    Then it succeeds
    And asking why lists both sources

  Scenario: Two citations of one source are one source
    Given a workspace with a composed definition
    When I cite the same source twice
    Then asking why reports one distinct source

  Scenario: A definition with nothing citing it says so
    Given a workspace with a composed definition
    Then asking why reports no corpus citations

  Scenario: A citation need not reproduce the text
    Given a workspace with a composed definition
    When I cite two sources using it
    Then no citation carries the defined text
