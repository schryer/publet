@conformance @must-8
Feature: An undeclared fork is conspicuous
  Forking cannot be prevented; anyone holding the bytes can publish a
  near-identical manifest. What the protocol can do is make an undeclared
  one visible, and give the legitimate case -- disagreement about
  composition -- somewhere to be stated.

  Scenario: Two documents sharing their citations without declaring it
    Given two documents citing the same publets
    When I look for undeclared forks
    Then the pair is reported

  Scenario: A declared derivation is not reported
    Given two documents citing the same publets
    And the second declares it derives from the first
    When I look for undeclared forks
    Then nothing is reported
