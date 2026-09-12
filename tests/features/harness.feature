Feature: The functional harness is wired correctly
  # Phase 0 delivers the harness before the first binary exists, so that no
  # later phase can defer writing its scenarios.

  Scenario: Binaries are resolved from the environment, not a build path
    Given the binary directory is configured
    Then it is the directory named by PUBLET_BIN_DIR
