@conformance @must-14.1
Feature: A generation declares its membership change
  Leaving a removal to be discovered reveals it only to a reader who thinks
  to look. Requiring the publisher to declare it, with a justification, as a
  condition of the generation parsing is the difference between auditable
  and audited.

  Scenario: A removal without a justification is refused
    Given a generation removing a member with no accounting reference
    Then the generation is malformed
    And the reason mentions "no `ref` accounting for it"

  Scenario: A removal with a justification is well formed
    Given a generation removing a member with a tombstone reference
    Then the generation is well formed

  Scenario: An undeclared removal makes the generation malformed
    Given a membership that loses a member
    And a generation record declaring no removals
    Then checking it against the membership fails
    And the reason mentions "malformed"
