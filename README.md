# rNEAR

## Platform integration

The LST accepts wNEAR `ft_transfer_call` stakes from registered platform users,
delivers rNEAR shares, and stores platform-scoped unstake requests. Matured
requests can be claimed by any account through `withdraw_request`. Existing
deployments must run `migrate_state` after the code upgrade. Migration leaves
`wnear_id` unset and starts the emergency storage reserve at zero. The owner must
call `set_wnear_id` once with one yoctoNEAR before wNEAR traffic starts; the
address cannot subsequently be changed. New deployments provide `wnear_id` to
`new`. Anyone can irreversibly fund the reserve with
`deposit_emergency_storage_reserve`; new wNEAR stakes are rejected until its
cumulative deposits reach 5 NEAR. The LST account must be registered with the
configured wNEAR contract; governance must add each platform user and fund its
request-storage credit before traffic starts.

Every stake message must include both `msg` and `refund_message`. A successful
stake always calls the platform user's `ft_on_transfer`; omitted fields are
rejected before wNEAR is unwrapped. Unstake and refund routes currently support
only `WithdrawTokens.Wnear` with a non-empty `msg`; withdrawal delivery always
uses wNEAR `ft_transfer_call`. `WithdrawTokens.Native` is not supported.

Accepted wNEAR stakes create a per-attempt lost-found record before the unwrap
Promise is scheduled and return `0` to the outer wNEAR resolver. This prevents
an unwrap callback failure from refunding out of the contract's shared wNEAR.
The outer `ft_transfer_call` succeeding means only that the stake was accepted;
integrators must wait for `stake_completed` or inspect the attempt record.
The record is removed atomically with successful rNEAR minting; an unsuccessful
or missing callback leaves it in `Wnear`, `Near`, or `Processing` status. No
failed stake is retried or automatically refunded. `get_stake_lostfound` and
`get_stake_lostfound_entries` expose pending records, and
`get_stake_lostfound_near_reserved` reports possible NEAR liabilities excluded
from ordinary spendable balance.

After checking the transaction's final receipts and verifying that the contract
still holds the indicated asset, the owner calls `recover_stake_lostfound` with
the attempt ID, asset (`Near` or `Wnear`), and evidence hash. The call directly
wraps NEAR if needed and sends wNEAR to the immutable refund route; no
`withdraw_request` or special withdrawal tranche is created. A known `Near` or
`Wnear` status cannot be recovered as the opposite asset; `Processing` requires
operator judgment. A failed wrap remains retryable, and an unused transfer
amount remains in the attempt record. If a recovery callback cannot establish
the outcome, the record stays locked in `Wrapping` or `Sending` until the owner
checks the receipts and calls `reconcile_stake_wrapping` or
`reconcile_stake_transfer` with evidence. Operator verification is essential:
the contract cannot prove from a failed callback alone which asset it holds or
whether a transfer was consumed. This integration still requires an independent
security review before deployment.
