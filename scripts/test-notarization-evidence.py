#!/usr/bin/env python3
"""Regression tests for fail-closed Apple notarization evidence parsing."""

import importlib.util
import json
from pathlib import Path
import tempfile


SCRIPT = Path(__file__).with_name("build-release-archive.py")
SPEC = importlib.util.spec_from_file_location("build_release_archive", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def expect_failure(path, message):
    try:
        MODULE.validate_notarization_result(path)
    except SystemExit as error:
        if str(error) != message:
            raise AssertionError(f"unexpected error: {error}") from error
    else:
        raise AssertionError(f"expected failure: {message}")


def main():
    with tempfile.TemporaryDirectory(prefix="worklouderctl-notary-evidence-") as root:
        root = Path(root)
        accepted = root / "accepted.json"
        accepted.write_text(
            json.dumps(
                {
                    "id": "12345678-1234-4321-8765-123456789abc",
                    "message": "Processing complete",
                    "status": "Accepted",
                }
            )
        )
        assert MODULE.validate_notarization_result(accepted)["status"] == "Accepted"

        pending = root / "pending.json"
        pending.write_text(
            json.dumps(
                {
                    "id": "12345678-1234-4321-8765-123456789abc",
                    "status": "In Progress",
                }
            )
        )
        expect_failure(pending, "notarization result was not accepted")

        malformed = root / "malformed.json"
        malformed.write_text("not json")
        expect_failure(malformed, "notarization result was not valid JSON")

        invalid_id = root / "invalid-id.json"
        invalid_id.write_text(
            json.dumps({"id": "not-a-uuid", "status": "Accepted"})
        )
        expect_failure(invalid_id, "notarization result had an invalid submission id")

        expect_failure(None, "accepted notarization result is required")

    print("notarization_evidence_accepted=verified")
    print("notarization_evidence_pending=refused")
    print("notarization_evidence_malformed=refused")
    print("notarization_evidence_invalid_id=refused")


if __name__ == "__main__":
    main()
