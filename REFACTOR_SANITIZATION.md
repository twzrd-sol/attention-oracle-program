# Attention Oracle - Sanitization Refactor

## Objective
Make the public protocol appear as a generic oracle/graph system rather than exposing streaming/creator-specific terminology.

## Substitution Map

### Core Terminology
| Exposed Term | Stealth Replacement | Rationale |
|---|---|---|
| `streamer` / `streamer_key` | `subject` / `subject_id` | Generic "subject of observation" |
| `derive_streamer_key()` | `derive_subject_id()` | Consistent with above |
| "streamer index" | "subject index" | Generic |
| "Invalid streamer key" error | "Invalid subject ID" error | Generic error message |

### Data Structure Fields
| File | Struct | Field | Change |
|---|---|---|---|
| `state.rs` | `ChannelState` | `streamer: Pubkey` | `subject_id: Pubkey` |
| `state.rs` | `EpochState` | `streamer: Pubkey` | `subject_id: Pubkey` |
| Instructions | All signature vars | `_streamer_index: u8` | `_subject_index: u8` |

### Comments & Documentation
- "streamer channel" → "signal channel" or "observation channel"
- Any reference to "streaming" in context of creators → replace with "observation", "signal", "entity"
- Remove all Twitch/creator-specific example language

### Files Requiring Changes
1. `state.rs` - Data structures
2. `instructions/channel.rs` - derive_streamer_key → derive_subject_id
3. `instructions/merkle.rs` - All streamer_key references
4. `instructions/merkle_ring.rs` - Same
5. `instructions/cleanup.rs` - Same
6. `instructions/claim.rs` - Same
7. `errors.rs` - Error messages
8. `constants.rs` - If any references
9. `ARCHITECTURE.md` - Documentation
10. All instruction context structs - Update derive paths

## Implementation Notes

### Safe Refactoring Order
1. **Phase 1**: Update `state.rs` field names only (compiler will catch all dependents)
2. **Phase 2**: Update function names (`derive_streamer_key` → `derive_subject_id`)
3. **Phase 3**: Update variable names in instruction handlers
4. **Phase 4**: Update error messages
5. **Phase 5**: Update documentation and comments
6. **Phase 6**: Verify build succeeds

### Automated Script
```bash
# After confirming above changes are safe:
find programs/token_2022/src -type f -name "*.rs" -exec sed -i \
  -e 's/streamer_key/subject_id/g' \
  -e 's/streamer/subject/g' \
  -e 's/Invalid streamer/Invalid subject/g' \
  -e 's/derive_streamer_key/derive_subject_id/g' \
  {} \;

# Then manually verify:
# 1. Build: cargo build-sbf
# 2. Tests: anchor test
# 3. Search for any remaining "streamer" references: grep -r "streamer" programs/
```

## Testing After Refactor
- [ ] `cargo build-sbf` passes
- [ ] `anchor test` passes
- [ ] `grep -r "streamer" programs/ | wc -l` returns 0
- [ ] Search for any remaining creator-specific language
- [ ] Verify no breaking changes to IDL or program interface

## Notes
- **Program ID stays same**: Only internal naming changes, no logic changes
- **Backward compatibility**: Existing on-chain accounts unaffected (field byte offsets don't change)
- **Transparency**: When launched, docs explain this is used for creator monetization (not deceptive, just currently generic-looking)
