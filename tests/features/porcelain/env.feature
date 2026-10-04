@porcelain @must-8
Feature: A pipeline runs only inside the environment it names
  An environment publet says how to provision a pinned toolchain into a
  local directory named by the publet's identifier, and which host
  programs it needs to do so. Rendering a publet whose pipeline cites one
  runs only what that environment provisioned.

  Background:
    Given a corpus whose pipeline runs a tool its environment provisions

  Scenario: Rendering before provisioning is refused
    When I build and render the document
    Then it fails
    And stderr mentions "pub env build"

  Scenario: A provisioned environment is the only place the tool runs from
    When I build and provision the environment
    Then it succeeds
    And the tool is not on the host's PATH
    When I render the document
    Then it succeeds
    And the document's output names the document

  Scenario: A missing host requirement is refused and nothing is provisioned
    When the host lacks "stubfetch"
    And I build and provision the environment
    Then it fails
    And stderr mentions "stubfetch"
    And the environment is not provisioned

  Scenario: A host requirement outside its version is refused
    When the host's "stubfetch" reports version "2.0.0"
    And I build and provision the environment
    Then it fails
    And stderr mentions "requires `stubfetch` 1.0"
    And the environment is not provisioned

  Scenario: A fetched file that does not match its pin stops provisioning
    When the environment pins the fetched file to other bytes
    And I build and provision the environment
    Then it fails
    And stderr mentions "not the"
    And the environment is not provisioned

  Scenario: Provisioning twice does nothing the second time
    When I build and provision the environment
    And I provision the environment again
    Then it reports "already provisioned"

  Scenario: A runtime host requirement that has changed stops rendering
    When I build and provision the environment
    And the host's "stubnode" reports version "2.0.0"
    And I render the document
    Then it fails
    And stderr mentions "stubnode"

  Scenario: The environment's directory is named by its identifier
    When I build and ask for the environment's path
    Then the path ends with the environment's identifier
