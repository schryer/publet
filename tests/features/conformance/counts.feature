@conformance @must-7.1
Feature: Annotation counts are never presented as a quality signal
  An interface showing "47 endorsements" without saying whose has
  reintroduced every Sybil problem the design removes. Annotations are
  weighted through a viewpoint before anything is shown.

  Scenario: Standing is reported as weight, not as a count of annotations
    Given a workspace containing the cohort claim
    And fifty trusted keys affirming the claim
    When I ask why
    Then no raw count of annotations appears
    And the affirming weight is shown instead
