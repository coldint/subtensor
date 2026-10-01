"""The public CPU miner and registration intent preserve the runtime challenge."""

import hashlib

import pytest
from eth_utils import keccak

from bittensor._transport.codec import ss58_encode
from bittensor.intents import PowRegister
from bittensor.pow_registration import (
    DOMAIN,
    _mine_chunk,
    mine_registration,
    registration_prefix,
    registration_seal,
)
from tests.harness.fake_substrate import FakeSubstrate
from tests.harness.samples import ALICE, ALICE_HOT, dev_wallet


def test_challenge_encoding_binds_both_keys_subnet_block_and_nonce():
    hot = bytes([11]) + bytes(31)
    cold = bytes([22]) + bytes(31)
    block = bytes([1]) * 32
    prefix = registration_prefix(1, block.hex(), ss58_encode(hot.hex()), ss58_encode(cold.hex()))
    expected = DOMAIN + b"\x01\x00" + block + hot + cold
    assert prefix == expected
    assert registration_seal(prefix, 7).hex() == (
        "06eef1fff090c8a0c4fbbbd17093346c9a45b3208fb3ff62524474cf4ba2cbc1"
    )
    assert registration_seal(prefix, 7) == keccak(
        hashlib.sha256(expected + (7).to_bytes(8, "little")).digest()
    )
    assert registration_seal(prefix, 7) != registration_seal(prefix, 8)
    assert (
        registration_prefix(2, block.hex(), ss58_encode(hot.hex()), ss58_encode(cold.hex()))
        != prefix
    )
    assert _mine_chunk(prefix, 1, 7, 1) == (7, registration_seal(prefix, 7).hex())
    assert _mine_chunk(prefix, 1 << 256, 7, 1) is None


@pytest.mark.asyncio
async def test_public_miner_returns_fresh_intent_without_a_signing_key():
    substrate = FakeSubstrate()
    substrate.block = 2
    substrate.seed("SubtensorModule", "NetworkRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "NetworkPowRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "Difficulty", [1], 1)
    substrate.seed("SubtensorModule", "MaxRegistrationsPerBlock", [1], 1)
    substrate.seed("SubtensorModule", "Uids", [1, ALICE_HOT], None)
    intent = await mine_registration(substrate, 1, ALICE_HOT, ALICE, workers=1, max_seconds=10)
    assert isinstance(intent, PowRegister)
    assert intent.work_block == 1
    prefix = registration_prefix(1, await substrate.block_hash(1), ALICE_HOT, ALICE)
    assert bytes.fromhex(intent.work_hex) == registration_seal(prefix, intent.nonce)
    await intent.build(substrate, dev_wallet())


@pytest.mark.asyncio
async def test_miner_refuses_disabled_or_already_registered_subnets():
    substrate = FakeSubstrate()
    substrate.block = 2
    with pytest.raises(ValueError, match="enable"):
        await mine_registration(substrate, 1, ALICE_HOT, ALICE, workers=1)
    substrate.seed("SubtensorModule", "NetworkRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "NetworkPowRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "Uids", [1, ALICE_HOT], 0)
    with pytest.raises(ValueError, match="already registered"):
        await mine_registration(substrate, 1, ALICE_HOT, ALICE, workers=1)


@pytest.mark.asyncio
async def test_intent_rejects_bad_seals_and_root():
    with pytest.raises(ValueError, match="32-byte"):
        await PowRegister(netuid=1, work_block=1, nonce=0, work_hex="ab").build(
            FakeSubstrate(), dev_wallet()
        )
    with pytest.raises(ValueError, match="non-root"):
        await PowRegister(netuid=0, work_block=1, nonce=0, work_hex="ab" * 32).build(
            FakeSubstrate(), dev_wallet()
        )


def test_sync_sdk_exposes_the_same_public_miner():
    from bittensor import SyncClient

    substrate = FakeSubstrate()
    substrate.block = 2
    substrate.seed("SubtensorModule", "NetworkRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "NetworkPowRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "Difficulty", [1], 1)
    substrate.seed("SubtensorModule", "MaxRegistrationsPerBlock", [1], 1)
    substrate.seed("SubtensorModule", "Uids", [1, ALICE_HOT], None)
    with SyncClient("local", substrate=substrate) as client:
        intent = client.mine_pow_registration(1, ALICE_HOT, ALICE, workers=1, max_seconds=10)
    assert isinstance(intent, PowRegister)
    assert intent.work_block == 1


@pytest.mark.asyncio
async def test_miner_waits_for_next_block_when_registration_capacity_is_used(monkeypatch):
    import asyncio

    from bittensor import pow_registration

    substrate = FakeSubstrate()
    substrate.block = 2
    substrate.seed("SubtensorModule", "NetworkRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "NetworkPowRegistrationAllowed", [1], True)
    substrate.seed("SubtensorModule", "Difficulty", [1], 1)
    substrate.seed("SubtensorModule", "Uids", [1, ALICE_HOT], None)
    substrate.seed("SubtensorModule", "MaxRegistrationsPerBlock", [1], 1)
    substrate.seed("SubtensorModule", "RegistrationsThisBlock", [1], 1)
    sleep = asyncio.sleep
    waited = []

    async def next_block(delay):
        waited.append(delay)
        substrate.block += 1
        substrate.seed("SubtensorModule", "RegistrationsThisBlock", [1], 0)
        await sleep(0)

    monkeypatch.setattr(pow_registration.asyncio, "sleep", next_block)
    intent = await mine_registration(substrate, 1, ALICE_HOT, ALICE, workers=1, max_seconds=10)
    assert waited == [1]
    assert intent.work_block == 2
