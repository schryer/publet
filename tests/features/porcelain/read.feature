@porcelain @must-11.3
Feature: An assertion is shown with the scope it was made under
  A publet read without its validity conditions is a different assertion.

  Background:
    Given a workspace containing the cohort claim

  Scenario: The claim and its scope are shown together
    When I read the claim
    Then the content is shown
    And the scope is shown

  Scenario: Reading from the replica discloses nothing
    When I read the claim
    Then the mode is reported as "local"
    And the output says nothing was disclosed

  Scenario: A relation is read as what it asserts, not as an empty header
    Given a workspace with two claims and a relation between them
    When I read the relation
    Then its kind, endpoints, and aspect are shown

  Scenario: An annotation is read as what it asserts
    Given a workspace with a claim and a usage annotation on it
    When I read the annotation
    Then its kind, target, and value fields are shown

  Scenario: An object file is read without any workspace, under the identifier its bytes hash to
    Given an object file holding the cohort claim, named for its identifier
    When I read the object file from a directory with no workspace
    Then it succeeds
    And the content is shown
    And the identifier shown is the one its bytes hash to
    And no mode is reported

  Scenario: An object file named for another identifier is read, and the mismatch reported
    Given an object file holding the cohort claim, named for a different identifier
    When I read the object file from a directory with no workspace
    Then it succeeds
    And the identifier shown is the one its bytes hash to
    And it warns that the file was altered or misnamed

  Scenario: The whole object can be read as JSON
    When I read the claim as JSON
    Then it succeeds
    And the JSON names the claim's identifier and the mode "local"
    And the JSON object holds every header field and the claim's scope
