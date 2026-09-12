@conformance @must-11.6
Feature: A signer counts once per equivalence class
  Otherwise endorsing five paraphrases of a claim would be worth five times
  endorsing one, and duplication would become a way of manufacturing weight.

  Scenario: Repeating a signer does not multiply their weight
    Given an evaluation vector where one key affirms three times
    When I evaluate it
    Then the affirm weight equals that of a single affirmation
