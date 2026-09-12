@conformance @must-5.1
Feature: A published object is never modified
  There is no mechanism to modify one. Revision is a new object plus a
  supersedes relation, which is what makes content addressing work at all.

  Scenario: Changing any field yields a different identifier
    Given a well-formed object
    And a second object differing only in its created field
    When I run "pub-cid" on each
    Then the two identifiers differ

  Scenario: A revision is a new object, not an edit
    Given a store where a claim has been revised
    Then both the original and the revision are present
    And the original still verifies against its own identifier
