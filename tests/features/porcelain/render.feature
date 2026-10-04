@porcelain @must-8
Feature: A publet is rendered only by the pipeline it names, and only once that pipeline checks out
  A publet's Colophon cites a render-pipeline publet. Its steps are
  procedural claims carrying the commands to run, each depending on the
  identity claim of every program it runs. `pub render` runs nothing until
  every object is signed by a trusted key, every program reports the
  version its identity claim states, and every repository file the pipeline
  cites still hashes to the blob its claim carries. A failure names the
  publet that disagrees with the machine.

  Background:
    Given a workspace with a signing key its policy trusts
    And a stub tool reporting version "1.0.0"
    And a publet whose Colophon names a pipeline running the stub tool

  Scenario: A checked pipeline runs and records what it produced
    When I build the sources signed and render the publet
    Then it succeeds
    And the step's declared output exists
    And the render record names the pipeline and the observed tool version

  Scenario: An unsigned pipeline is refused before anything runs
    When I build the sources unsigned and render the publet
    Then it fails
    And stderr mentions "carries no `authored` signature"
    And the step's declared output does not exist

  Scenario: A tool whose installed version differs from its claim is refused by name
    When the stub tool reports version "2.0.0" instead
    And I build the sources signed and render the publet
    Then it fails
    And stderr mentions "tool.stub#identity"
    And stderr mentions "stubtool 2.0.0"
    And the step's declared output does not exist

  Scenario: A repository file changed since it was published is refused
    When I build the sources signed
    And the repository file the pipeline cites is edited
    And I render the publet
    Then it fails
    And stderr mentions "no longer hashes"

  Scenario: The MyST project names every cited publet with its tag and identifier
    When I build the sources signed and write the publet's MyST project
    Then it succeeds
    And the MyST page shows the pipeline as its title, tag, and identifier
    And the MyST page lists the stub tool's version under Components
