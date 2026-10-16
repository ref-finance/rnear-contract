# rNEAR

## Platform integration (Plan A)

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

Plan A follows the returning Promise chain and has no per-stake recovery record.
Sandbox fault injection confirms that a post-unwrap callback panic or out-of-gas
failure can leave a storage reservation pending and cause the outer wNEAR
resolver to refund from wNEAR already held by this contract. A failed re-wrap
does not use shared wNEAR for a refund: it queues an immediately withdrawable
NEAR-backed claim under the supplied refund route. This still depends on its
callback completing; callback panic or out-of-gas remains a known Plan A risk,
not covered by its normal failure handling. Do not deploy this integration
without an independent security review and an explicit decision to accept these
failure assumptions; use a pending-stake recovery design (Plan B) if they are
unacceptable.
