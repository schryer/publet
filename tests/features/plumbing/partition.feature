@plumbing
Feature: How several publets divide on a term
  Comparing two publets answers whether they differ. A corpus drawn from
  several sources asks a different question: how do all of them divide on
  this term, and which one stands alone. Six pairwise runs and no aggregate
  is not an answer to that.

  Scenario: Four publets on two readings partition into two groups
    Given four publets presupposing two definitions of one term
    When I ask how they partition
    Then 2 groups are reported for "cohort"
    And the majority group holds 3 publets
    And the outlier group holds 1

  Scenario: Agreement is not reported
    Given four publets presupposing one definition of a term
    When I ask how they partition
    Then nothing is reported
