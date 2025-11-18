Place the program IDL here at runtime:

1. Build the program IDL locally (on a machine with Anchor working):
   anchor build
2. Copy the IDL JSON into this directory:
   cp ../../target/idl/token_2022.json apps/stream-listener-ts/idl/token_2022.json

The listener loads ../idl/token_2022.json at startup for typed event parsing.

