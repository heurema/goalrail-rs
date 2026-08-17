#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile


EXPECTED_HASHES = {
    "case-004/evaluator-packet.json":
        "33442d002fea28255659010796ef25bbf2c3b91755012f4feacb5a877e2143a1",
    "case-004-activation/freeze-receipt.json":
        "00eacd49f0b4c24d040c786f27d317d2977c86243b84ff3d6c0dc9ac9b963833",
    "case-004-result/launch-feasibility.json":
        "0082187552b49b28e83253b557476501dd479d2a050a072cad945e84320e1717",
}

EXPECTED_EXECUTION_RECEIPT = {
    "seed_directories_created": False,
    "worktrees_created": False,
    "writer_tasks_created": False,
    "writers_started": False,
    "model_invocations": 0,
    "repository_work_started": False,
    "writer_results_created": False,
    "tracked_repository_edits": False,
    "git_configuration_changed": False,
    "commit_created": False,
    "push_performed": False,
    "external_write_performed": False,
    "rollback_required": False,
}

CANONICAL_ARTIFACT_SOURCE_FIELDS = {
    "evaluator-packet.json": "evaluator_packet_sha256",
    "writer-contract.json": "writer_contract_sha256",
    "writer-prompt.txt": "writer_prompt_sha256",
    "writer-output.schema.json": "writer_output_schema_sha256",
    "packet-binding.json": "packet_binding_sha256",
    "capability-check.json": "capability_check_sha256",
}


class VerificationError(Exception):
    pass


def require_value(document, path):
    value = document
    for key in path:
        if not isinstance(value, dict) or key not in value:
            raise VerificationError(f"missing JSON field: {'.'.join(path)}")
        value = value[key]
    return value


def json_equal_strict(value, expected):
    if type(value) is not type(expected):
        return False
    if isinstance(expected, dict):
        return value.keys() == expected.keys() and all(
            json_equal_strict(value[key], expected[key]) for key in expected
        )
    if isinstance(expected, list):
        return len(value) == len(expected) and all(
            json_equal_strict(item, expected_item)
            for item, expected_item in zip(value, expected)
        )
    return value == expected


def require_equal(document, path, expected):
    value = require_value(document, path)
    if not json_equal_strict(value, expected):
        raise VerificationError(
            f"unexpected JSON field {'.'.join(path)}: expected {expected!r}, got {value!r}"
        )


def load_regular_file(evidence_root, relative_path):
    path = evidence_root / relative_path
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise VerificationError(f"missing receipt: {relative_path}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise VerificationError(f"receipt is not a regular file: {relative_path}")

    return path.read_bytes()


def parse_json_object(data, label):
    try:
        document = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerificationError(f"invalid JSON object: {label}") from error
    if not isinstance(document, dict):
        raise VerificationError(f"JSON root is not an object: {label}")
    return document


def load_receipt(evidence_root, relative_path, expected_hash):
    data = load_regular_file(evidence_root, relative_path)

    observed_hash = hashlib.sha256(data).hexdigest()
    if observed_hash != expected_hash:
        raise VerificationError(
            f"receipt SHA-256 mismatch for {relative_path}: "
            f"expected {expected_hash}, got {observed_hash}"
        )
    return parse_json_object(data, relative_path)


def canonical_artifact_hashes(document):
    artifacts = require_value(document, ("canonical_artifacts",))
    if not isinstance(artifacts, list):
        raise VerificationError("canonical_artifacts is not an array")

    identities = {}
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise VerificationError("canonical_artifacts contains a non-object")
        filename = artifact.get("filename")
        sha256 = artifact.get("sha256")
        byte_count = artifact.get("bytes")
        if (
            not isinstance(filename, str)
            or not isinstance(sha256, str)
            or not isinstance(byte_count, int)
        ):
            raise VerificationError("canonical artifact identity is incomplete")
        if filename in identities:
            raise VerificationError(f"duplicate canonical artifact: {filename}")
        identities[filename] = (byte_count, sha256)

    expected_filenames = set(CANONICAL_ARTIFACT_SOURCE_FIELDS)
    if set(identities) != expected_filenames:
        raise VerificationError(
            "canonical artifact set mismatch: "
            f"expected {sorted(expected_filenames)}, got {sorted(identities)}"
        )
    return identities


def verify_receipts(evidence_root, expected_hashes):
    packet = load_receipt(
        evidence_root,
        "case-004/evaluator-packet.json",
        expected_hashes["case-004/evaluator-packet.json"],
    )
    activation = load_receipt(
        evidence_root,
        "case-004-activation/freeze-receipt.json",
        expected_hashes["case-004-activation/freeze-receipt.json"],
    )
    terminal = load_receipt(
        evidence_root,
        "case-004-result/launch-feasibility.json",
        expected_hashes["case-004-result/launch-feasibility.json"],
    )

    for document in (packet, activation, terminal):
        require_equal(document, ("case_id",), "case-004")
        require_equal(document, ("protocol_version",), 4)

    require_equal(
        packet,
        ("schema_version",),
        "goalrail.intent-canary.evaluator-packet.v4",
    )
    require_equal(packet, ("status",), "PACKET_FROZEN_LAUNCH_NOT_AUTHORIZED")
    require_equal(packet, ("packet_frozen",), True)
    require_equal(packet, ("launch_authorized",), False)
    require_equal(packet, ("attempt_reserved",), True)
    require_equal(packet, ("attempt_consumed",), False)
    require_equal(packet, ("writers_started",), False)

    require_equal(
        activation,
        ("schema_version",),
        "goalrail.intent-canary.freeze-activation-receipt.v1",
    )
    require_equal(
        activation,
        ("status",),
        "PACKET_FROZEN_LAUNCH_NOT_AUTHORIZED",
    )
    require_equal(
        activation,
        ("independent_acceptance", "verdict"),
        "READY_TO_FREEZE",
    )
    require_equal(
        activation,
        ("owner_authority", "live_launch_authorized"),
        False,
    )
    artifact_identities = canonical_artifact_hashes(activation)
    if artifact_identities["evaluator-packet.json"][1] != expected_hashes[
        "case-004/evaluator-packet.json"
    ]:
        raise VerificationError(
            "canonical evaluator-packet SHA-256 does not match packet bytes"
        )

    for filename, (expected_size, expected_hash) in artifact_identities.items():
        artifact_data = load_regular_file(
            evidence_root, f"case-004/{filename}"
        )
        if len(artifact_data) != expected_size:
            raise VerificationError(f"canonical artifact size mismatch for {filename}")
        if hashlib.sha256(artifact_data).hexdigest() != expected_hash:
            raise VerificationError(f"canonical artifact SHA-256 mismatch for {filename}")

    manifest_hash = require_value(activation, ("candidate_manifest", "sha256"))
    require_equal(
        activation,
        ("candidate_manifest", "path"),
        ".git/goalrail/intent-canary/case-004-freeze-candidate/"
        "freeze-candidate-manifest.json",
    )
    manifest_data = load_regular_file(
        evidence_root,
        "case-004-freeze-candidate/freeze-candidate-manifest.json",
    )
    require_equal(
        activation,
        ("candidate_manifest", "bytes"),
        len(manifest_data),
    )
    if hashlib.sha256(manifest_data).hexdigest() != manifest_hash:
        raise VerificationError("freeze candidate manifest SHA-256 mismatch")
    manifest = parse_json_object(
        manifest_data,
        "case-004-freeze-candidate/freeze-candidate-manifest.json",
    )
    require_equal(
        manifest,
        ("schema_version",),
        "goalrail.intent-canary.freeze-candidate-manifest.v1",
    )
    require_equal(manifest, ("case_id",), "case-004")
    require_equal(manifest, ("protocol_version",), 4)
    require_equal(
        manifest,
        ("status",),
        "INACTIVE_CANDIDATE_AWAITING_INDEPENDENT_REVIEW",
    )
    for field in (
        "launch_authorized",
        "packet_frozen_now",
        "writers_started",
        "attempt_reserved_now",
        "case_id_reserved_now",
        "staging_authoritative",
    ):
        require_equal(manifest, (field,), False)
    require_equal(
        activation,
        ("independent_acceptance", "accepted_manifest_sha256"),
        manifest_hash,
    )

    packet_accounting = require_value(packet, ("attempt_accounting",))
    activation_accounting = require_value(activation, ("attempt_accounting",))
    for field in (
        "total",
        "consumed",
        "reserved",
        "reserved_case",
        "unreserved_remaining",
        "attempt_consumed",
    ):
        require_equal(activation_accounting, (field,), require_value(packet_accounting, (field,)))

    require_equal(
        terminal,
        ("schema_version",),
        "goalrail.intent-canary.launch-feasibility.v2",
    )
    require_equal(terminal, ("verdict",), "INVALID_PRELAUNCH")
    for filename, source_field in CANONICAL_ARTIFACT_SOURCE_FIELDS.items():
        require_equal(
            terminal,
            ("source_of_truth", source_field),
            artifact_identities[filename][1],
        )
    require_equal(
        terminal,
        ("source_of_truth", "freeze_activation_receipt_sha256"),
        expected_hashes["case-004-activation/freeze-receipt.json"],
    )
    require_equal(
        terminal,
        ("source_of_truth", "freeze_candidate_manifest_sha256"),
        manifest_hash,
    )
    for field in ("repository", "commit", "tree", "tracked_status"):
        packet_field = "path" if field == "repository" else field
        require_equal(
            terminal,
            ("source_of_truth", field),
            require_value(packet, ("frozen_repository", packet_field)),
        )

    require_equal(
        packet,
        ("seed_contract", "source_commit"),
        require_value(packet, ("frozen_repository", "commit")),
    )
    require_equal(
        packet,
        ("seed_contract", "source_tree"),
        require_value(packet, ("frozen_repository", "tree")),
    )
    require_equal(
        terminal,
        ("frozen_seed_contract", "baseline_destination"),
        require_value(packet, ("seed_contract", "baseline_destination")),
    )
    require_equal(
        terminal,
        ("frozen_seed_contract", "baseline_destination_state"),
        require_value(packet, ("seed_contract", "baseline_destination_state")),
    )
    require_equal(
        terminal,
        ("frozen_seed_contract", "treatment_files"),
        require_value(packet, ("seed_contract", "expected_complete_manifest_difference")),
    )

    require_equal(terminal, ("execution_receipt",), EXPECTED_EXECUTION_RECEIPT)
    require_equal(packet_accounting, ("total",), 5)
    require_equal(packet_accounting, ("consumed",), 3)
    require_equal(packet_accounting, ("reserved",), 1)
    require_equal(packet_accounting, ("reserved_case",), "case-004")
    require_equal(packet_accounting, ("unreserved_remaining",), 1)
    require_equal(packet_accounting, ("attempt_consumed",), False)
    invalid_accounting = require_value(
        packet_accounting, ("after_invalid_post_freeze",)
    )
    require_equal(invalid_accounting, ("consumed",), 4)
    require_equal(invalid_accounting, ("reserved",), 0)
    require_equal(invalid_accounting, ("unreserved_remaining",), 1)
    require_equal(invalid_accounting, ("protocol_disposition",), "MODIFY")
    require_equal(
        terminal,
        ("attempt_accounting_after", "total"),
        require_value(packet_accounting, ("total",)),
    )
    for field in ("consumed", "reserved", "unreserved_remaining"):
        require_equal(
            terminal,
            ("attempt_accounting_after", field),
            require_value(invalid_accounting, (field,)),
        )
    require_equal(
        terminal,
        ("attempt_accounting_after", "case_004_attempt_consumed"),
        True,
    )
    require_equal(
        terminal,
        ("attempt_accounting_after", "case_004_attempt_result"),
        "INVALID_PRELAUNCH",
    )
    require_equal(
        terminal,
        ("protocol_disposition",),
        require_value(invalid_accounting, ("protocol_disposition",)),
    )


def sanitized_git_environment():
    return {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("GIT_")
    }


def default_evidence_root():
    repo_root = Path(__file__).resolve().parent.parent
    result = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=repo_root,
        env=sanitized_git_environment(),
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    common_dir = Path(result.stdout.strip())
    if not common_dir.is_absolute():
        common_dir = repo_root / common_dir
    return common_dir.resolve() / "goalrail" / "intent-canary"


def write_json(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(document, sort_keys=True, separators=(",", ":")),
        encoding="utf-8",
    )


def fixture_artifact_data(filename):
    return f"fixture canonical artifact: {filename}\n".encode("utf-8")


def fixture_manifest_data(overrides=None):
    manifest = {
        "schema_version": "goalrail.intent-canary.freeze-candidate-manifest.v1",
        "case_id": "case-004",
        "protocol_version": 4,
        "status": "INACTIVE_CANDIDATE_AWAITING_INDEPENDENT_REVIEW",
        "launch_authorized": False,
        "packet_frozen_now": False,
        "writers_started": False,
        "attempt_reserved_now": False,
        "case_id_reserved_now": False,
        "staging_authoritative": False,
    }
    if overrides is not None:
        manifest.update(overrides)
    return json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode(
        "utf-8"
    )


def fixture_documents():
    commit = "1" * 40
    tree = "2" * 40
    manifest_data = fixture_manifest_data()
    manifest_hash = hashlib.sha256(manifest_data).hexdigest()
    artifact_hashes = {
        "evaluator-packet.json": "0" * 64,
        **{
            filename: hashlib.sha256(fixture_artifact_data(filename)).hexdigest()
            for filename in CANONICAL_ARTIFACT_SOURCE_FIELDS
            if filename != "evaluator-packet.json"
        },
    }
    current_accounting = {
        "total": 5,
        "consumed": 3,
        "reserved": 1,
        "reserved_case": "case-004",
        "unreserved_remaining": 1,
        "attempt_consumed": False,
    }
    packet = {
        "case_id": "case-004",
        "protocol_version": 4,
        "schema_version": "goalrail.intent-canary.evaluator-packet.v4",
        "status": "PACKET_FROZEN_LAUNCH_NOT_AUTHORIZED",
        "packet_frozen": True,
        "launch_authorized": False,
        "attempt_reserved": True,
        "attempt_consumed": False,
        "writers_started": False,
        "frozen_repository": {
            "path": "/fixture/repository",
            "commit": commit,
            "tree": tree,
            "tracked_status": "clean",
        },
        "seed_contract": {
            "source_commit": commit,
            "source_tree": tree,
            "baseline_destination": ".agents/skills/goalrail-intent",
            "baseline_destination_state": "ABSENT",
            "expected_complete_manifest_difference": [
                ".agents/skills/goalrail-intent/SKILL.md",
                ".agents/skills/goalrail-intent/agents/openai.yaml",
            ],
        },
        "attempt_accounting": {
            **current_accounting,
            "after_invalid_post_freeze": {
                "consumed": 4,
                "reserved": 0,
                "unreserved_remaining": 1,
                "protocol_disposition": "MODIFY",
            },
        },
    }
    activation = {
        "case_id": "case-004",
        "protocol_version": 4,
        "schema_version": "goalrail.intent-canary.freeze-activation-receipt.v1",
        "status": "PACKET_FROZEN_LAUNCH_NOT_AUTHORIZED",
        "candidate_manifest": {
            "path": ".git/goalrail/intent-canary/case-004-freeze-candidate/"
            "freeze-candidate-manifest.json",
            "bytes": len(manifest_data),
            "sha256": manifest_hash,
        },
        "independent_acceptance": {
            "verdict": "READY_TO_FREEZE",
            "accepted_manifest_sha256": manifest_hash,
        },
        "owner_authority": {"live_launch_authorized": False},
        "canonical_artifacts": [
            {
                "filename": filename,
                "bytes": (
                    0
                    if filename == "evaluator-packet.json"
                    else len(fixture_artifact_data(filename))
                ),
                "sha256": sha256,
            }
            for filename, sha256 in artifact_hashes.items()
        ],
        "attempt_accounting": dict(current_accounting),
    }
    terminal = {
        "case_id": "case-004",
        "protocol_version": 4,
        "schema_version": "goalrail.intent-canary.launch-feasibility.v2",
        "verdict": "INVALID_PRELAUNCH",
        "source_of_truth": {
            "repository": "/fixture/repository",
            "commit": commit,
            "tree": tree,
            "tracked_status": "clean",
            **{
                source_field: artifact_hashes[filename]
                for filename, source_field in CANONICAL_ARTIFACT_SOURCE_FIELDS.items()
            },
            "freeze_candidate_manifest_sha256": manifest_hash,
            "freeze_activation_receipt_sha256": "0" * 64,
        },
        "frozen_seed_contract": {
            "baseline_destination": ".agents/skills/goalrail-intent",
            "baseline_destination_state": "ABSENT",
            "treatment_files": [
                ".agents/skills/goalrail-intent/SKILL.md",
                ".agents/skills/goalrail-intent/agents/openai.yaml",
            ],
        },
        "execution_receipt": dict(EXPECTED_EXECUTION_RECEIPT),
        "attempt_accounting_after": {
            "total": 5,
            "consumed": 4,
            "reserved": 0,
            "unreserved_remaining": 1,
            "case_004_attempt_consumed": True,
            "case_004_attempt_result": "INVALID_PRELAUNCH",
        },
        "protocol_disposition": "MODIFY",
    }
    return packet, activation, terminal


def write_fixture(root, documents, manifest_data=None):
    packet_path = "case-004/evaluator-packet.json"
    activation_path = "case-004-activation/freeze-receipt.json"
    terminal_path = "case-004-result/launch-feasibility.json"

    candidate_path = (
        root / "case-004-freeze-candidate/freeze-candidate-manifest.json"
    )
    candidate_path.parent.mkdir(parents=True, exist_ok=True)
    candidate_path.write_bytes(
        fixture_manifest_data() if manifest_data is None else manifest_data
    )
    for filename in CANONICAL_ARTIFACT_SOURCE_FIELDS:
        if filename == "evaluator-packet.json":
            continue
        artifact_path = root / "case-004" / filename
        artifact_path.parent.mkdir(parents=True, exist_ok=True)
        artifact_path.write_bytes(fixture_artifact_data(filename))

    write_json(root / packet_path, documents[0])
    packet_hash = hashlib.sha256((root / packet_path).read_bytes()).hexdigest()
    for artifact in documents[1]["canonical_artifacts"]:
        if artifact["filename"] == "evaluator-packet.json":
            artifact["bytes"] = (root / packet_path).stat().st_size
            artifact["sha256"] = packet_hash
            break
    documents[2]["source_of_truth"]["evaluator_packet_sha256"] = packet_hash
    write_json(root / activation_path, documents[1])
    activation_hash = hashlib.sha256(
        (root / activation_path).read_bytes()
    ).hexdigest()
    documents[2]["source_of_truth"][
        "freeze_activation_receipt_sha256"
    ] = activation_hash
    write_json(root / terminal_path, documents[2])
    terminal_hash = hashlib.sha256((root / terminal_path).read_bytes()).hexdigest()
    return {
        packet_path: packet_hash,
        activation_path: activation_hash,
        terminal_path: terminal_hash,
    }


def write_manifest_variant_fixture(root, overrides):
    documents = fixture_documents()
    manifest_data = fixture_manifest_data(overrides)
    manifest_hash = hashlib.sha256(manifest_data).hexdigest()
    documents[1]["candidate_manifest"]["bytes"] = len(manifest_data)
    documents[1]["candidate_manifest"]["sha256"] = manifest_hash
    documents[1]["independent_acceptance"][
        "accepted_manifest_sha256"
    ] = manifest_hash
    documents[2]["source_of_truth"][
        "freeze_candidate_manifest_sha256"
    ] = manifest_hash
    return write_fixture(root, documents, manifest_data)


def expect_failure(label, callback):
    try:
        callback()
    except VerificationError:
        return
    raise VerificationError(f"self-test accepted sabotage: {label}")


def run_self_test():
    with tempfile.TemporaryDirectory(prefix="goalrail-intent-case-004.") as directory:
        root = Path(directory)

        expected_evidence_root = default_evidence_root()
        fake_git_dir = root / "redirected.git"
        subprocess.run(
            ["git", "init", "--bare", str(fake_git_dir)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env=sanitized_git_environment(),
        )
        previous_git_dir = os.environ.get("GIT_DIR")
        os.environ["GIT_DIR"] = str(fake_git_dir)
        try:
            if default_evidence_root() != expected_evidence_root:
                raise VerificationError(
                    "default evidence root inherited GIT_DIR"
                )
        finally:
            if previous_git_dir is None:
                os.environ.pop("GIT_DIR", None)
            else:
                os.environ["GIT_DIR"] = previous_git_dir

        documents = fixture_documents()
        hashes = write_fixture(root, documents)
        verify_receipts(root, hashes)

        terminal_path = root / "case-004-result/launch-feasibility.json"
        terminal_bytes = terminal_path.read_bytes()
        terminal_path.write_bytes(terminal_bytes + b"\n")
        expect_failure("hash-mismatch", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        documents[2]["verdict"] = "VALID"
        hashes = write_fixture(root, documents)
        expect_failure("terminal-verdict", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        documents[2]["execution_receipt"]["external_write_performed"] = True
        hashes = write_fixture(root, documents)
        expect_failure("no-launch", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        documents[2]["execution_receipt"]["external_write_performed"] = 0
        hashes = write_fixture(root, documents)
        expect_failure(
            "no-launch-boolean-type",
            lambda: verify_receipts(root, hashes),
        )

        documents = fixture_documents()
        documents[1]["independent_acceptance"]["accepted_manifest_sha256"] = (
            "0" * 64
        )
        hashes = write_fixture(root, documents)
        expect_failure("manifest-cross-link", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        documents[2]["source_of_truth"]["writer_prompt_sha256"] = "0" * 64
        hashes = write_fixture(root, documents)
        expect_failure("artifact-cross-link", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        documents[0]["attempt_accounting"]["after_invalid_post_freeze"][
            "consumed"
        ] = 5
        hashes = write_fixture(root, documents)
        expect_failure("accounting-cross-link", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        hashes = write_fixture(root, documents)
        artifact_path = root / "case-004/writer-prompt.txt"
        artifact_path.write_bytes(artifact_path.read_bytes() + b"sabotage\n")
        expect_failure("canonical-artifact-bytes", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        hashes = write_fixture(root, documents)
        manifest_path = (
            root / "case-004-freeze-candidate/freeze-candidate-manifest.json"
        )
        manifest_path.write_bytes(manifest_path.read_bytes() + b"sabotage\n")
        expect_failure("candidate-manifest-bytes", lambda: verify_receipts(root, hashes))

        manifest_sabotages = [
            (
                "candidate-manifest-schema",
                {"schema_version": "invalid-schema"},
            ),
            ("candidate-manifest-case", {"case_id": "case-005"}),
            ("candidate-manifest-protocol-type", {"protocol_version": 4.0}),
            (
                "candidate-manifest-status",
                {"status": "PACKET_FROZEN_LAUNCH_NOT_AUTHORIZED"},
            ),
        ]
        manifest_sabotages.extend(
            (f"candidate-manifest-{field}-type", {field: 0})
            for field in (
                "launch_authorized",
                "packet_frozen_now",
                "writers_started",
                "attempt_reserved_now",
                "case_id_reserved_now",
                "staging_authoritative",
            )
        )
        for label, overrides in manifest_sabotages:
            hashes = write_manifest_variant_fixture(root, overrides)
            expect_failure(label, lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        hashes = write_fixture(root, documents)
        activation_path = root / "case-004-activation/freeze-receipt.json"
        activation_path.unlink()
        expect_failure("missing-receipt", lambda: verify_receipts(root, hashes))

        documents = fixture_documents()
        hashes = write_fixture(root, documents)
        packet_path = root / "case-004/evaluator-packet.json"
        packet_copy = root / "packet-copy.json"
        packet_copy.write_bytes(packet_path.read_bytes())
        packet_path.unlink()
        os.symlink(packet_copy, packet_path)
        expect_failure("symlink-receipt", lambda: verify_receipts(root, hashes))

    print(
        "GOALRAIL_INTENT_CASE_004_RECEIPTS_SELF_TEST_OK "
        "scenarios=valid,hash-mismatch,terminal-verdict,no-launch,"
        "no-launch-boolean-type,manifest-cross-link,artifact-cross-link,"
        "accounting-cross-link,canonical-artifact-bytes,candidate-manifest-bytes,"
        "candidate-manifest-strict-identity,missing-receipt,symlink-receipt,"
        "git-dir-sanitization"
    )


def main():
    parser = argparse.ArgumentParser(
        description="Verify immutable local Goalrail intent case-004 receipts."
    )
    parser.add_argument(
        "--evidence-root",
        type=Path,
        help="Override the intent-canary evidence root for local worktree discovery.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run portable positive and sabotage checks without local trial evidence.",
    )
    arguments = parser.parse_args()

    if arguments.self_test:
        run_self_test()
        return

    evidence_root = (
        arguments.evidence_root.resolve()
        if arguments.evidence_root is not None
        else default_evidence_root()
    )
    verify_receipts(evidence_root, EXPECTED_HASHES)
    print(
        "GOALRAIL_INTENT_CASE_004_RECEIPTS_OK "
        "case=case-004 protocol=4 verdict=INVALID_PRELAUNCH no_launch=verified"
    )


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.SubprocessError, VerificationError) as error:
        print(f"goalrail-intent-case-004-receipts: {error}", file=sys.stderr)
        raise SystemExit(1)
