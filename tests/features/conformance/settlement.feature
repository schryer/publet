@conformance @must-12.1 @must-12.2 @must-12.3 @must-12.5
Feature: Settlement holds money and no authority
  The ledger executes predicates over evaluations anyone can recompute.
  Every predicate is chosen in advance by the party putting up the money,
  and none of it reaches what anyone may read or what anyone believes.

  Scenario: The layer is optional and says so when absent
    Given no ledger is configured
    When a bounty is settled
    Then it fails
    And the reason says settlement is optional

  Scenario: A bounty must name the viewpoint it settles against
    Given a bounty naming no policy
    Then reading it fails
    And the reason says a bounty must name the policy it settles against

  Scenario Outline: The review share has a floor
    Given a bounty with a review share of <share> basis points
    Then reading it <outcome>

    Examples:
      | share | outcome   |
      | 5999  | fails     |
      | 6000  | succeeds  |
      | 10000 | succeeds  |

  Scenario: The prohibitions are enumerated with their reasons
    Given the settlement prohibitions
    Then each names what it forbids and why

  Scenario: The ledger interface cannot express a standing or a read
    Given the ledger trait
    Then it offers no way to report or influence a standing
    And it offers no way to gate a retrieval
