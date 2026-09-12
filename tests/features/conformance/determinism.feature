@conformance @must-11.2
Feature: Evaluation is reproducible
  R8: the same policy over the same snapshot must yield bit-identical
  results in every conformant implementation. A result that varies between
  runs on one machine cannot possibly hold across implementations.

  Scenario: Repeated evaluation produces identical bytes
    Given every committed evaluation vector
    When I evaluate each one twice
    Then both runs produce byte-identical output

  Scenario: Output carries every component of the standing
    Given the "06-contested" vector
    When I evaluate it
    Then the output names affirm, deny, abstain, active and delta
    And every weight is written to exactly six decimal places
