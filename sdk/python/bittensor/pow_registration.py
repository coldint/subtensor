"""Public challenge mining for fee-free subnet registration."""

from __future__ import annotations

import asyncio
import hashlib
import multiprocessing
import os
import secrets
import time
from concurrent.futures import ProcessPoolExecutor

from eth_utils import keccak

from ._generated import storage as st
from ._transport.codec import ss58_decode

DOMAIN = b"subtensor-pow-register-v1"


def registration_prefix(netuid: int, block_hash: str, hotkey: str, coldkey: str) -> bytes:
    """Match the runtime's challenge bytes, without the trailing nonce."""
    if not 0 < netuid < 65536:
        raise ValueError("PoW registration requires a non-root u16 netuid")
    block = bytes.fromhex(block_hash.removeprefix("0x"))
    hot = bytes.fromhex(ss58_decode(hotkey))
    cold = bytes.fromhex(ss58_decode(coldkey))
    if any(len(value) != 32 for value in (block, hot, cold)):
        raise ValueError("PoW requires a 32-byte block hash and AccountId32 addresses")
    return DOMAIN + netuid.to_bytes(2, "little") + block + hot + cold


def registration_seal(prefix: bytes, nonce: int) -> bytes:
    return keccak(hashlib.sha256(prefix + nonce.to_bytes(8, "little")).digest())


def _mine_chunk(prefix: bytes, difficulty: int, start: int, attempts: int):
    target = ((1 << 256) - 1) // max(1, difficulty)
    for offset in range(attempts):
        nonce = (start + offset) % (1 << 64)
        work = registration_seal(prefix, nonce)
        if int.from_bytes(work, "little") <= target:
            return nonce, work.hex()
    return None


async def mine_registration(
    substrate,
    netuid: int,
    hotkey: str,
    coldkey: str,
    *,
    workers: int = 4,
    max_seconds: float = 300,
):
    """Return a PowRegister intent, refreshing stale challenges automatically.

    Only public addresses and hashes enter workers. Signing remains in the
    client's normal executor. Submit the returned intent promptly; a proof is
    valid only while its challenge is one of the two previous chain blocks.
    """
    from .intents.registration import PowRegister

    if not 1 <= workers <= 32 or not 0 < max_seconds <= 3600:
        raise ValueError("workers must be 1..32 and max_seconds must be 0..3600")
    deadline = time.monotonic() + max_seconds
    loop = asyncio.get_running_loop()
    pool = ProcessPoolExecutor(
        max_workers=min(workers, os.cpu_count() or 1),
        mp_context=multiprocessing.get_context("spawn"),
    )
    try:
        while time.monotonic() < deadline:
            head = await substrate.block_number()
            if head < 1:
                await asyncio.sleep(1)
                continue
            anchor = await substrate.block_hash(head)
            (
                allowed,
                pow_allowed,
                difficulty,
                uid,
                last,
                registrations,
                limit,
            ) = await asyncio.gather(
                substrate.query(
                    *st.SubtensorModule.NetworkRegistrationAllowed, [netuid], block_hash=anchor
                ),
                substrate.query(
                    *st.SubtensorModule.NetworkPowRegistrationAllowed, [netuid], block_hash=anchor
                ),
                substrate.query(*st.SubtensorModule.Difficulty, [netuid], block_hash=anchor),
                substrate.query(*st.SubtensorModule.Uids, [netuid, hotkey], block_hash=anchor),
                substrate.query(
                    *st.SubtensorModule.LastPowRegistrationBlock, [hotkey], block_hash=anchor
                ),
                substrate.query(
                    *st.SubtensorModule.RegistrationsThisBlock, [netuid], block_hash=anchor
                ),
                substrate.query(
                    *st.SubtensorModule.MaxRegistrationsPerBlock, [netuid], block_hash=anchor
                ),
            )
            if not allowed or not pow_allowed:
                raise ValueError("The subnet owner must enable registration and PoW registration")
            if uid is not None:
                raise ValueError("The hotkey is already registered on this subnet")
            if not limit:
                raise ValueError("The subnet's per-block registration limit must be positive")
            if int(registrations or 0) >= int(limit):
                await asyncio.sleep(1)
                continue
            work_block = head - 1
            if last is not None and work_block <= int(last):
                await asyncio.sleep(1)
                continue
            block_hash = await substrate.block_hash(work_block)
            prefix = registration_prefix(netuid, block_hash, hotkey, coldkey)
            while time.monotonic() < deadline:
                start = secrets.randbits(64)
                futures = [
                    loop.run_in_executor(
                        pool,
                        _mine_chunk,
                        prefix,
                        int(difficulty or 1),
                        (start + index * 100_000) % (1 << 64),
                        100_000,
                    )
                    for index in range(min(workers, os.cpu_count() or 1))
                ]
                results = await asyncio.gather(*futures)
                current = await substrate.block_number()
                if (
                    current <= work_block
                    or current - work_block >= 2
                    or await substrate.block_hash(work_block) != block_hash
                ):
                    break
                for result in results:
                    if result is not None:
                        nonce, work_hex = result
                        return PowRegister(
                            netuid=netuid,
                            work_block=work_block,
                            nonce=nonce,
                            work_hex=work_hex,
                            hotkey_ss58=hotkey,
                            mining_workers=workers,
                            mining_timeout=max_seconds,
                        )
                await asyncio.sleep(0.1)
        raise TimeoutError("No fresh registration proof found within the mining time limit")
    finally:
        pool.shutdown(wait=False, cancel_futures=True)
