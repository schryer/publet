@conformance @must-4.7
Feature: A file is carried as a blob, verified like an object
  A blob is bytes named by their own CID. It is not an object and states
  nothing about itself; the claim citing it says what it is and how large.
  A claim may not cite bytes nobody here could verify.

  Background:
    Given a workspace

  Scenario: A file named in the data is stored as a blob and cited by size
    Given a file "plan.pdf" containing "%PDF-1.7 plan"
    When I compose a claim whose data is the file "plan.pdf" as "application/pdf"
    Then it succeeds
    And reading it shows "file, application/pdf, 13 bytes"
    And the store holds that blob and no object for it

  Scenario: Citing a blob that is not held is refused
    When I compose a claim whose data cites a blob nothing here holds
    Then it fails
    And stderr mentions "is not held here"

  Scenario: Citing a held blob at the wrong size is refused
    Given a file "plan.pdf" containing "%PDF-1.7 plan"
    And a claim whose data is the file "plan.pdf" as "application/pdf"
    When I compose a claim citing that blob with size 99
    Then it fails
    And stderr mentions "not the 99 the data states"
