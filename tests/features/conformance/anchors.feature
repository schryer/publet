@conformance @must-9.3
Feature: An anchor points inside the lineage it names
  An anchor is a shortcut to a computation every reader can perform
  themselves. One pointing outside its own lineage is not a shortcut; it is
  a different claim wearing the shape of one.

  Scenario: An anchor naming a member of its lineage is accepted
    Given an anchor recommending a member of the lineage it names
    When I load the store
    Then loading the store succeeds

  Scenario: An anchor naming something outside its lineage is refused
    Given an anchor recommending an object outside the lineage it names
    When I load the store
    Then loading the store fails
    And stderr mentions "not in the lineage"
